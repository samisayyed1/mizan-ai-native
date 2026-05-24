//! Durable retry queue for ledger appends that fail after the originating
//! row already committed (e.g. transient db error on the ledger insert
//! while the activity row landed). A startup task drains the queue —
//! see `mizan_app::startup` integration.
//!
//! The retry queue persists AppendInput as JSON so the same payload
//! can be replayed unchanged when the failing condition clears.

use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::truth_ledger_retry_queue::dsl as q_dsl;
use async_trait::async_trait;
use mizan_core::errors::{Error, Result};
use mizan_core::truth_engine::{AppendInput, LedgerEntryKind, TruthLedger, TruthLedgerRetryQueue};
use rust_decimal::Decimal;

/// JSON-friendly mirror of AppendInput so it can be serialised to the
/// retry queue without leaking BTreeMap<String, Value> ordering quirks
/// (BTreeMap already serialises deterministically — but pinning it here
/// makes the wire shape explicit).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppendInputWire {
    id: String,
    kind: Option<LedgerEntryKind>,
    account_id: Option<String>,
    asset_id: Option<String>,
    amount: Option<String>, // Decimal as string for stable JSON
    currency: Option<String>,
    metadata: std::collections::BTreeMap<String, serde_json::Value>,
    recorded_at: Option<chrono::DateTime<Utc>>,
}

impl From<&AppendInput> for AppendInputWire {
    fn from(a: &AppendInput) -> Self {
        Self {
            id: a.id.clone(),
            kind: a.kind,
            account_id: a.account_id.clone(),
            asset_id: a.asset_id.clone(),
            amount: a.amount.map(|d| d.to_string()),
            currency: a.currency.clone(),
            metadata: a.metadata.clone(),
            recorded_at: a.recorded_at,
        }
    }
}

impl From<AppendInputWire> for AppendInput {
    fn from(w: AppendInputWire) -> Self {
        Self {
            id: w.id,
            kind: w.kind,
            account_id: w.account_id,
            asset_id: w.asset_id,
            amount: w
                .amount
                .as_deref()
                .and_then(|s| std::str::FromStr::from_str(s).ok())
                .or({
                    // Be tolerant of a Decimal that round-trips through
                    // json! into a number (shouldn't happen, but defensive)
                    None::<Decimal>
                }),
            currency: w.currency,
            metadata: w.metadata,
            recorded_at: w.recorded_at,
        }
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::truth_ledger_retry_queue)]
struct RetryRow {
    id: String,
    payload_json: String,
    queued_at: i64,
    attempts: i32,
    last_error: Option<String>,
}

pub struct SqliteTruthLedgerRetryQueue {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl SqliteTruthLedgerRetryQueue {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }

    /// Queue an AppendInput for later replay. Idempotent on AppendInput.id
    /// (one retry slot per source row — re-queueing bumps `attempts`).
    async fn enqueue_impl(&self, input: &AppendInput, reason: &str) -> Result<()> {
        let wire = AppendInputWire::from(input);
        let payload_json = serde_json::to_string(&wire)
            .map_err(|e| Error::Unexpected(format!("retry queue serialise failed: {e}")))?;
        let id = input.id.clone();
        let reason = reason.to_string();
        let now = Utc::now().timestamp_millis();
        self.writer
            .exec(move |conn| -> Result<()> {
                let existing: Option<RetryRow> = q_dsl::truth_ledger_retry_queue
                    .filter(q_dsl::id.eq(&id))
                    .select(RetryRow::as_select())
                    .first::<RetryRow>(conn)
                    .optional()
                    .map_err(StorageError::from)?;
                match existing {
                    Some(row) => {
                        diesel::update(
                            q_dsl::truth_ledger_retry_queue.filter(q_dsl::id.eq(&id)),
                        )
                        .set((
                            q_dsl::attempts.eq(row.attempts + 1),
                            q_dsl::last_error.eq(Some(reason)),
                        ))
                        .execute(conn)
                        .map_err(StorageError::from)?;
                    }
                    None => {
                        let new_row = RetryRow {
                            id: id.clone(),
                            payload_json,
                            queued_at: now,
                            attempts: 0,
                            last_error: Some(reason),
                        };
                        diesel::insert_into(q_dsl::truth_ledger_retry_queue)
                            .values(&new_row)
                            .execute(conn)
                            .map_err(StorageError::from)?;
                    }
                }
                Ok(())
            })
            .await
    }

    /// Drain the queue: re-attempt each pending append. Rows that
    /// succeed are removed; rows that fail again get `attempts` bumped
    /// and `last_error` updated. Caps attempts at `max_attempts` —
    /// beyond that the row stays in the queue but is skipped (the
    /// support bundle surfaces these for manual triage).
    pub async fn drain(
        &self,
        ledger: Arc<dyn TruthLedger>,
        max_attempts: i32,
    ) -> Result<DrainStats> {
        // Snapshot the rows up-front so we don't hold a write lock for
        // every retry. (Rows the queue acquires after this snapshot wait
        // for the next drain — that's fine.)
        let pool = Arc::clone(&self.pool);
        let rows = tokio::task::spawn_blocking(move || -> Result<Vec<RetryRow>> {
            let mut conn = get_connection(&pool)?;
            let rows = q_dsl::truth_ledger_retry_queue
                .filter(q_dsl::attempts.lt(max_attempts))
                .order(q_dsl::queued_at.asc())
                .select(RetryRow::as_select())
                .load::<RetryRow>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(rows)
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join: {e}")))??;

        let mut stats = DrainStats::default();
        for row in rows {
            let wire: AppendInputWire = match serde_json::from_str(&row.payload_json) {
                Ok(w) => w,
                Err(e) => {
                    stats.failed += 1;
                    self.bump_attempt(&row.id, &format!("decode failed: {e}")).await?;
                    continue;
                }
            };
            let input: AppendInput = wire.into();
            match ledger.append(input).await {
                Ok(_) => {
                    self.dequeue(&row.id).await?;
                    stats.succeeded += 1;
                }
                Err(e) => {
                    self.bump_attempt(&row.id, &e.to_string()).await?;
                    stats.failed += 1;
                }
            }
        }
        Ok(stats)
    }

    async fn dequeue(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec(move |conn| -> Result<()> {
                diesel::delete(
                    q_dsl::truth_ledger_retry_queue.filter(q_dsl::id.eq(&id)),
                )
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    async fn bump_attempt(&self, id: &str, reason: &str) -> Result<()> {
        let id = id.to_string();
        let reason = reason.to_string();
        self.writer
            .exec(move |conn| -> Result<()> {
                diesel::update(q_dsl::truth_ledger_retry_queue.filter(q_dsl::id.eq(&id)))
                    .set((
                        q_dsl::attempts.eq(q_dsl::attempts + 1),
                        q_dsl::last_error.eq(Some(reason)),
                    ))
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    /// Public façade matching the `TruthLedgerRetryQueue` trait. Same
    /// body as the trait impl below, exposed inherently so callers
    /// holding the concrete type can use it without importing the trait.
    pub async fn enqueue(&self, input: &AppendInput, reason: &str) -> Result<()> {
        self.enqueue_impl(input, reason).await
    }

    /// Total queue depth. Surfaced by the support bundle.
    pub async fn len(&self) -> Result<i64> {
        let pool = Arc::clone(&self.pool);
        tokio::task::spawn_blocking(move || -> Result<i64> {
            let mut conn = get_connection(&pool)?;
            let n = q_dsl::truth_ledger_retry_queue
                .count()
                .get_result::<i64>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(n)
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join: {e}")))?
    }
}

#[derive(Debug, Clone, Default)]
pub struct DrainStats {
    pub succeeded: u32,
    pub failed: u32,
}

#[async_trait]
impl TruthLedgerRetryQueue for SqliteTruthLedgerRetryQueue {
    async fn enqueue(&self, input: &AppendInput, reason: &str) -> Result<()> {
        self.enqueue_impl(input, reason).await
    }
}
