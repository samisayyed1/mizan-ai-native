//! SQLite-backed §A4 SyncRunLedger.
//!
//! Writes go through the dedicated WriteHandle actor (no concurrent writers).
//! Reads use a pooled connection. Idempotent on `run_id` — re-inserts replace
//! the row in-place (same semantics as the in-memory impl).

use async_trait::async_trait;
use diesel::prelude::*;
use std::sync::Arc;

use super::model::SyncRunLedgerDB;
use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::sync_run_ledger::dsl as ledger_dsl;
use mizan_core::errors::Result;
use mizan_core::sync_ledger::{SyncRunEntry, SyncRunLedger, SyncRunProvider};

pub struct SqliteSyncRunLedger {
    pool: Arc<DbPool>,
    writer: WriteHandle,
    /// Soft cap: when the row count grows past this, we trim the oldest
    /// completed rows. Keeps the table bounded so support bundles + the
    /// in-app audit page stay snappy.
    max_rows: i64,
}

impl SqliteSyncRunLedger {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self::with_capacity(pool, writer, 50_000)
    }

    pub fn with_capacity(pool: Arc<DbPool>, writer: WriteHandle, max_rows: i64) -> Self {
        Self {
            pool,
            writer,
            max_rows,
        }
    }
}

#[async_trait]
impl SyncRunLedger for SqliteSyncRunLedger {
    async fn append(&self, entry: SyncRunEntry) -> Result<()> {
        let row: SyncRunLedgerDB = (&entry).into();
        let cap = self.max_rows;

        self.writer
            .exec(move |conn| -> Result<()> {
                // UPSERT on run_id so a single run can transition from
                // started → finished/failed without leaving two rows
                // behind. AsChangeset is derived on the model so the
                // updated_at-style fields naturally roll forward.
                diesel::insert_into(ledger_dsl::sync_run_ledger)
                    .values(&row)
                    .on_conflict(ledger_dsl::run_id)
                    .do_update()
                    .set((
                        ledger_dsl::provider.eq(&row.provider),
                        ledger_dsl::mode.eq(&row.mode),
                        ledger_dsl::account_id.eq(&row.account_id),
                        ledger_dsl::thread_id.eq(&row.thread_id),
                        // started_at preserved from the original insert —
                        // don't overwrite it on the close (else the run
                        // duration would always be 0).
                        ledger_dsl::finished_at.eq(&row.finished_at),
                        ledger_dsl::outcome.eq(&row.outcome),
                        ledger_dsl::summary_json.eq(&row.summary_json),
                        ledger_dsl::error_json.eq(&row.error_json),
                    ))
                    .execute(conn)
                    .map_err(StorageError::from)?;

                // FIFO trim — delete completed rows past the cap.
                // Active (finished_at IS NULL) rows are never pruned so
                // in-flight syncs can always close cleanly.
                diesel::sql_query(
                    "DELETE FROM sync_run_ledger WHERE run_id IN (\
                        SELECT run_id FROM sync_run_ledger \
                        WHERE finished_at IS NOT NULL \
                        ORDER BY started_at DESC \
                        LIMIT -1 OFFSET ?\
                    )",
                )
                .bind::<diesel::sql_types::BigInt, _>(cap)
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(())
            })
            .await
    }

    async fn recent(&self, limit: usize) -> Result<Vec<SyncRunEntry>> {
        let pool = Arc::clone(&self.pool);
        let limit = limit as i64;
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let rows: Vec<SyncRunLedgerDB> = ledger_dsl::sync_run_ledger
                .order(ledger_dsl::started_at.desc())
                .limit(limit)
                .select(SyncRunLedgerDB::as_select())
                .load::<SyncRunLedgerDB>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(rows.into_iter().map(SyncRunEntry::from).collect())
        })
        .await
        .map_err(|e| {
            mizan_core::errors::Error::Unexpected(format!("spawn_blocking join error: {e}"))
        })?
    }

    async fn recent_by_provider(
        &self,
        provider: SyncRunProvider,
        limit: usize,
    ) -> Result<Vec<SyncRunEntry>> {
        let pool = Arc::clone(&self.pool);
        let provider_str = provider.as_str().to_string();
        let limit = limit as i64;
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let rows: Vec<SyncRunLedgerDB> = ledger_dsl::sync_run_ledger
                .filter(ledger_dsl::provider.eq(provider_str))
                .order(ledger_dsl::started_at.desc())
                .limit(limit)
                .select(SyncRunLedgerDB::as_select())
                .load::<SyncRunLedgerDB>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(rows.into_iter().map(SyncRunEntry::from).collect())
        })
        .await
        .map_err(|e| {
            mizan_core::errors::Error::Unexpected(format!("spawn_blocking join error: {e}"))
        })?
    }

    async fn get(&self, run_id: &str) -> Result<Option<SyncRunEntry>> {
        let pool = Arc::clone(&self.pool);
        let run_id = run_id.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let row = ledger_dsl::sync_run_ledger
                .filter(ledger_dsl::run_id.eq(run_id))
                .select(SyncRunLedgerDB::as_select())
                .first::<SyncRunLedgerDB>(&mut conn)
                .optional()
                .map_err(StorageError::from)?;
            Ok(row.map(SyncRunEntry::from))
        })
        .await
        .map_err(|e| {
            mizan_core::errors::Error::Unexpected(format!("spawn_blocking join error: {e}"))
        })?
    }
}
