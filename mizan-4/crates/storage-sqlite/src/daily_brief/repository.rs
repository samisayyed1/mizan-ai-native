//! SQLite-backed §A22 DailyBriefService.
//!
//! DailyBrief carries multiple nested vecs (top_movers, drift, stale,
//! pending_drafts) which would each need their own table. For a first
//! production cut we serialise the whole DailyBrief as JSON in
//! `payload_json` — it's append-once-per-day and only read by the
//! Settings → Notifications surface, so the JSON-blob trade-off is
//! ergonomic. Schema normalisation can land later without a data
//! migration since the read path is `serde_json::from_str`.

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::prelude::*;
use std::sync::Arc;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::daily_briefs::dsl as briefs_dsl;
use mizan_core::daily_brief::{DailyBrief, DailyBriefService};
use mizan_core::errors::{Error, Result};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::daily_briefs)]
struct DailyBriefDB {
    brief_date: String,
    base_currency: String,
    payload_json: String,
    delivered_at: Option<i64>,
    created_at: i64,
}

fn to_db(b: &DailyBrief) -> DailyBriefDB {
    DailyBriefDB {
        brief_date: b.brief_date.format("%Y-%m-%d").to_string(),
        base_currency: b.base_currency.clone(),
        payload_json: serde_json::to_string(b).unwrap_or_else(|_| "{}".to_string()),
        delivered_at: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    }
}

fn from_db(r: DailyBriefDB) -> Option<DailyBrief> {
    serde_json::from_str::<DailyBrief>(&r.payload_json).ok()
}

pub struct SqliteDailyBriefService {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl SqliteDailyBriefService {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl DailyBriefService for SqliteDailyBriefService {
    async fn upsert(&self, brief: DailyBrief) -> Result<()> {
        let row = to_db(&brief);
        self.writer
            .exec(move |conn| -> Result<()> {
                diesel::insert_into(briefs_dsl::daily_briefs)
                    .values(&row)
                    .on_conflict((briefs_dsl::brief_date, briefs_dsl::base_currency))
                    .do_update()
                    .set((
                        briefs_dsl::payload_json.eq(&row.payload_json),
                        briefs_dsl::created_at.eq(row.created_at),
                    ))
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    async fn latest(&self) -> Result<Option<DailyBrief>> {
        let pool = Arc::clone(&self.pool);
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let row = briefs_dsl::daily_briefs
                .order(briefs_dsl::brief_date.desc())
                .select(DailyBriefDB::as_select())
                .first::<DailyBriefDB>(&mut conn)
                .optional()
                .map_err(StorageError::from)?;
            Ok(row.and_then(from_db))
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn get(&self, date: NaiveDate) -> Result<Option<DailyBrief>> {
        let pool = Arc::clone(&self.pool);
        let date_s = date.format("%Y-%m-%d").to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let row = briefs_dsl::daily_briefs
                .filter(briefs_dsl::brief_date.eq(date_s))
                .select(DailyBriefDB::as_select())
                .first::<DailyBriefDB>(&mut conn)
                .optional()
                .map_err(StorageError::from)?;
            Ok(row.and_then(from_db))
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn recent(&self, limit: usize) -> Result<Vec<DailyBrief>> {
        let pool = Arc::clone(&self.pool);
        let limit = limit as i64;
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let rows: Vec<DailyBriefDB> = briefs_dsl::daily_briefs
                .order(briefs_dsl::brief_date.desc())
                .limit(limit)
                .select(DailyBriefDB::as_select())
                .load::<DailyBriefDB>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(rows.into_iter().filter_map(from_db).collect())
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn mark_read(&self, date: NaiveDate) -> Result<()> {
        // Round-trip the row so the `read: true` change lands in
        // payload_json (which is the single source of truth for the
        // brief body). Two writes (read row → write row) inside a
        // single write actor exec so they're transactional.
        let date_s = date.format("%Y-%m-%d").to_string();
        self.writer
            .exec(move |conn| -> Result<()> {
                let existing: Option<DailyBriefDB> = briefs_dsl::daily_briefs
                    .filter(briefs_dsl::brief_date.eq(&date_s))
                    .select(DailyBriefDB::as_select())
                    .first::<DailyBriefDB>(conn)
                    .optional()
                    .map_err(StorageError::from)?;
                let Some(row) = existing else { return Ok(()); };
                let mut brief = match serde_json::from_str::<DailyBrief>(&row.payload_json) {
                    Ok(b) => b,
                    Err(_) => return Ok(()),
                };
                if brief.read {
                    return Ok(());
                }
                brief.read = true;
                let new_json =
                    serde_json::to_string(&brief).unwrap_or_else(|_| row.payload_json.clone());
                diesel::update(
                    briefs_dsl::daily_briefs
                        .filter(briefs_dsl::brief_date.eq(&date_s))
                        .filter(briefs_dsl::base_currency.eq(&row.base_currency)),
                )
                .set(briefs_dsl::payload_json.eq(new_json))
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }
}
