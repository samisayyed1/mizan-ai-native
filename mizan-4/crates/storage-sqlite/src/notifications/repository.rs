//! SQLite-backed `NotificationService` (Notify-3).
//!
//! Mirrors the daily-brief repository pattern:
//!   - writes go through the single-writer `WriteHandle` actor to
//!     serialise mutations + avoid SQLITE_BUSY,
//!   - reads use `spawn_blocking` against the shared read pool.
//!
//! Idempotency is enforced at the database level via the UNIQUE
//! index on `dedupe_key`. The repo translates the conflict into
//! `Ok(false)` so callers don't have to introspect Diesel error
//! variants.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use std::sync::Arc;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::notifications::dsl as n_dsl;
use mizan_core::errors::{Error, Result};
use mizan_core::notifications::{
    Notification, NotificationKind, NotificationService, NotificationSeverity, NotificationsPage,
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::notifications)]
struct NotificationRow {
    id: String,
    kind: String,
    severity: String,
    title: String,
    body: String,
    deep_link: Option<String>,
    payload_json: String,
    dedupe_key: String,
    created_at: i64,
    read_at: Option<i64>,
    dismissed_at: Option<i64>,
}

impl NotificationRow {
    fn from_domain(n: &Notification) -> Self {
        Self {
            id: n.id.clone(),
            kind: n.kind.as_str().to_string(),
            severity: n.severity.as_str().to_string(),
            title: n.title.clone(),
            body: n.body.clone(),
            deep_link: n.deep_link.clone(),
            payload_json: n.payload_json.clone(),
            dedupe_key: n.dedupe_key.clone(),
            created_at: n.created_at_ms,
            read_at: n.read_at_ms,
            dismissed_at: n.dismissed_at_ms,
        }
    }

    fn into_domain(self) -> Notification {
        Notification {
            id: self.id,
            kind: NotificationKind::from_str_lenient(&self.kind),
            severity: NotificationSeverity::from_str_lenient(&self.severity),
            title: self.title,
            body: self.body,
            deep_link: self.deep_link,
            payload_json: self.payload_json,
            dedupe_key: self.dedupe_key,
            created_at_ms: self.created_at,
            read_at_ms: self.read_at,
            dismissed_at_ms: self.dismissed_at,
        }
    }
}

pub struct SqliteNotificationService {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl SqliteNotificationService {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl NotificationService for SqliteNotificationService {
    async fn emit(&self, notification: Notification) -> Result<bool> {
        let row = NotificationRow::from_domain(&notification);
        self.writer
            .exec(move |conn| -> Result<bool> {
                // INSERT OR IGNORE pattern via Diesel: we treat a
                // UNIQUE-constraint violation on `dedupe_key` as the
                // idempotent no-op the engine guarantees, and bubble
                // anything else up.
                let res = diesel::insert_into(n_dsl::notifications)
                    .values(&row)
                    .execute(conn);
                match res {
                    Ok(_) => Ok(true),
                    Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                        Ok(false)
                    }
                    Err(e) => Err(Error::from(StorageError::from(e))),
                }
            })
            .await
    }

    async fn list_active(&self, limit: usize) -> Result<NotificationsPage> {
        let pool = Arc::clone(&self.pool);
        let limit_i64 = limit as i64;
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            // Active = not dismissed. We sort by created_at DESC so the
            // bell panel shows newest first.
            let rows: Vec<NotificationRow> = n_dsl::notifications
                .filter(n_dsl::dismissed_at.is_null())
                .order(n_dsl::created_at.desc())
                .limit(limit_i64)
                .select(NotificationRow::as_select())
                .load::<NotificationRow>(&mut conn)
                .map_err(StorageError::from)?;

            // Unread count is the total across all not-dismissed rows
            // — NOT capped by `limit`, since the bell badge needs the
            // true count even when the panel shows the top N.
            let unread_count: i64 = n_dsl::notifications
                .filter(n_dsl::dismissed_at.is_null())
                .filter(n_dsl::read_at.is_null())
                .count()
                .get_result(&mut conn)
                .map_err(StorageError::from)?;

            Ok(NotificationsPage {
                items: rows.into_iter().map(NotificationRow::into_domain).collect(),
                unread_count,
            })
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn mark_read(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec(move |conn| -> Result<()> {
                let now = chrono::Utc::now().timestamp_millis();
                diesel::update(
                    n_dsl::notifications
                        .filter(n_dsl::id.eq(&id))
                        .filter(n_dsl::read_at.is_null()),
                )
                .set(n_dsl::read_at.eq(now))
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    async fn dismiss(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec(move |conn| -> Result<()> {
                let now = chrono::Utc::now().timestamp_millis();
                diesel::update(
                    n_dsl::notifications
                        .filter(n_dsl::id.eq(&id))
                        .filter(n_dsl::dismissed_at.is_null()),
                )
                .set(n_dsl::dismissed_at.eq(now))
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    async fn mark_all_read(&self) -> Result<usize> {
        self.writer
            .exec(move |conn| -> Result<usize> {
                let now = chrono::Utc::now().timestamp_millis();
                let n = diesel::update(
                    n_dsl::notifications
                        .filter(n_dsl::read_at.is_null())
                        .filter(n_dsl::dismissed_at.is_null()),
                )
                .set(n_dsl::read_at.eq(now))
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(n)
            })
            .await
    }

    async fn unread_count(&self) -> Result<i64> {
        let pool = Arc::clone(&self.pool);
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let count: i64 = n_dsl::notifications
                .filter(n_dsl::read_at.is_null())
                .filter(n_dsl::dismissed_at.is_null())
                .count()
                .get_result(&mut conn)
                .map_err(StorageError::from)?;
            Ok(count)
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }
}
