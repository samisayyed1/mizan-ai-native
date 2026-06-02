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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::write_actor::spawn_writer;
    use crate::db::{create_pool, init, run_migrations};
    use mizan_core::sync_ledger::{
        SyncRunEntry, SyncRunMode, SyncRunOutcome, SyncRunProvider, SyncRunSummary,
    };
    use tempfile::tempdir;

    fn build() -> (SqliteSyncRunLedger, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = init(&dir.path().to_string_lossy()).unwrap();
        run_migrations(&db_path).unwrap();
        let pool = create_pool(&db_path).unwrap();
        let writer = spawn_writer(pool.as_ref().clone()).unwrap();
        (SqliteSyncRunLedger::new(pool, writer), dir)
    }

    #[tokio::test]
    async fn close_preserves_started_at() {
        let (ledger, _dir) = build();
        let started = SyncRunEntry::started("run-1", SyncRunProvider::Plaid, SyncRunMode::Initial);
        let original_started_at = started.started_at;
        ledger.append(started.clone()).await.unwrap();

        // Simulate close-after-work: same run_id, finish() on the held
        // entry instance (this is the pattern the scheduler uses).
        let finished = started.finish(SyncRunSummary {
            inserted: 5,
            ..Default::default()
        });
        ledger.append(finished).await.unwrap();

        let got = ledger.get("run-1").await.unwrap().unwrap();
        // Storage truncates timestamps to millisecond precision; compare
        // at that resolution to assert "no drift to a fresh Utc::now()
        // on close" without depending on sub-milli precision.
        assert_eq!(
            got.started_at.timestamp_millis(),
            original_started_at.timestamp_millis(),
            "started_at preserved on close (must NOT bump to current time)"
        );
        assert!(got.finished_at.is_some());
        assert_eq!(got.summary.inserted, 5);
        assert_eq!(got.outcome, SyncRunOutcome::Applied);
    }

    #[tokio::test]
    async fn recent_orders_newest_first_across_providers() {
        let (ledger, _dir) = build();
        for (id, provider) in [
            ("r1", SyncRunProvider::Yahoo),
            ("r2", SyncRunProvider::MarketData),
            ("r3", SyncRunProvider::CsvImport),
        ] {
            ledger
                .append(
                    SyncRunEntry::started(id, provider, SyncRunMode::Incremental).finish(
                        SyncRunSummary {
                            inserted: 1,
                            ..Default::default()
                        },
                    ),
                )
                .await
                .unwrap();
            // tiny stagger so started_at differs deterministically
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let recent = ledger.recent(3).await.unwrap();
        assert_eq!(recent[0].run_id, "r3");
        assert_eq!(recent[2].run_id, "r1");
    }

    #[tokio::test]
    async fn recent_by_provider_only_returns_matching_rows() {
        let (ledger, _dir) = build();
        ledger
            .append(SyncRunEntry::started(
                "a",
                SyncRunProvider::MarketData,
                SyncRunMode::Incremental,
            ))
            .await
            .unwrap();
        ledger
            .append(SyncRunEntry::started(
                "b",
                SyncRunProvider::Yahoo,
                SyncRunMode::Incremental,
            ))
            .await
            .unwrap();
        ledger
            .append(SyncRunEntry::started(
                "c",
                SyncRunProvider::MarketData,
                SyncRunMode::Backfill,
            ))
            .await
            .unwrap();

        let only_md = ledger
            .recent_by_provider(SyncRunProvider::MarketData, 10)
            .await
            .unwrap();
        assert_eq!(only_md.len(), 2);
        assert!(only_md
            .iter()
            .all(|e| e.provider == SyncRunProvider::MarketData));
    }

    #[tokio::test]
    async fn failure_outcome_round_trips_with_error_envelope() {
        let (ledger, _dir) = build();
        let entry = SyncRunEntry::started("r-fail", SyncRunProvider::Plaid, SyncRunMode::Initial)
            .fail(r#"{"__mizan_error":true,"code":"PLAID_AUTH_EXPIRED"}"#);
        ledger.append(entry).await.unwrap();

        let got = ledger.get("r-fail").await.unwrap().unwrap();
        assert_eq!(got.outcome, SyncRunOutcome::Failed);
        assert!(got.error.unwrap().contains("PLAID_AUTH_EXPIRED"));
    }
}
