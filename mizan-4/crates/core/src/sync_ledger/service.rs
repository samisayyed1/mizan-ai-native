//! Sync Run Ledger trait + in-memory implementation.
//!
//! The SQLite-backed implementation lives in crates/storage-sqlite once
//! the migration lands. Until then, the in-memory impl is enough for
//! tests + the foundation PRs.

use std::sync::RwLock;

use async_trait::async_trait;

use super::model::{SyncRunEntry, SyncRunProvider};
use crate::Result;

/// Append-only ledger of every sync attempt.
#[async_trait]
pub trait SyncRunLedger: Send + Sync {
    /// Append one entry. Idempotent on `run_id` — re-inserting the same
    /// id is a no-op (since the same run may emit multiple updates as
    /// it progresses).
    async fn append(&self, entry: SyncRunEntry) -> Result<()>;

    /// Recent runs across all providers, newest first.
    async fn recent(&self, limit: usize) -> Result<Vec<SyncRunEntry>>;

    /// Recent runs for a specific provider.
    async fn recent_by_provider(
        &self,
        provider: SyncRunProvider,
        limit: usize,
    ) -> Result<Vec<SyncRunEntry>>;

    /// Latest entry for a given run_id, if any.
    async fn get(&self, run_id: &str) -> Result<Option<SyncRunEntry>>;
}

/// In-memory implementation — ideal for tests and the foundation phase.
/// Bounded by `max_entries` (default 10_000) so a long-running process
/// doesn't grow unbounded; oldest entries are dropped FIFO.
pub struct InMemorySyncRunLedger {
    entries: RwLock<Vec<SyncRunEntry>>,
    max_entries: usize,
}

impl InMemorySyncRunLedger {
    pub fn new() -> Self {
        Self::with_capacity(10_000)
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(Vec::with_capacity(max_entries.min(1024))),
            max_entries,
        }
    }

    /// Total entries currently retained (after FIFO truncation).
    pub fn len(&self) -> usize {
        self.entries.read().expect("ledger poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemorySyncRunLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncRunLedger for InMemorySyncRunLedger {
    async fn append(&self, entry: SyncRunEntry) -> Result<()> {
        let mut entries = self.entries.write().expect("ledger poisoned");

        // Replace existing entry with same run_id (idempotent updates).
        if let Some(slot) = entries.iter_mut().find(|e| e.run_id == entry.run_id) {
            *slot = entry;
            return Ok(());
        }

        entries.push(entry);

        // FIFO drop oldest when over cap.
        if entries.len() > self.max_entries {
            let drop_n = entries.len() - self.max_entries;
            entries.drain(0..drop_n);
        }
        Ok(())
    }

    async fn recent(&self, limit: usize) -> Result<Vec<SyncRunEntry>> {
        let entries = self.entries.read().expect("ledger poisoned");
        Ok(entries.iter().rev().take(limit).cloned().collect())
    }

    async fn recent_by_provider(
        &self,
        provider: SyncRunProvider,
        limit: usize,
    ) -> Result<Vec<SyncRunEntry>> {
        let entries = self.entries.read().expect("ledger poisoned");
        Ok(entries
            .iter()
            .rev()
            .filter(|e| e.provider == provider)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn get(&self, run_id: &str) -> Result<Option<SyncRunEntry>> {
        let entries = self.entries.read().expect("ledger poisoned");
        Ok(entries.iter().find(|e| e.run_id == run_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_ledger::model::{
        SyncRunEntry, SyncRunMode, SyncRunOutcome, SyncRunProvider, SyncRunSummary,
    };

    #[tokio::test]
    async fn append_and_get_round_trips() {
        let ledger = InMemorySyncRunLedger::new();
        let entry = SyncRunEntry::started("run-1", SyncRunProvider::Plaid, SyncRunMode::Initial)
            .with_account("acc-1");
        ledger.append(entry.clone()).await.unwrap();

        let got = ledger.get("run-1").await.unwrap().unwrap();
        assert_eq!(got.run_id, "run-1");
        assert_eq!(got.provider, SyncRunProvider::Plaid);
        assert_eq!(got.account_id.as_deref(), Some("acc-1"));
    }

    #[tokio::test]
    async fn append_with_same_run_id_replaces() {
        let ledger = InMemorySyncRunLedger::new();
        let started =
            SyncRunEntry::started("run-2", SyncRunProvider::Yahoo, SyncRunMode::Incremental);
        ledger.append(started.clone()).await.unwrap();

        let finished = started.finish(SyncRunSummary {
            inserted: 5,
            ..Default::default()
        });
        ledger.append(finished).await.unwrap();

        // Still only one row in the ledger.
        assert_eq!(ledger.len(), 1);
        let got = ledger.get("run-2").await.unwrap().unwrap();
        assert!(got.finished_at.is_some());
        assert_eq!(got.summary.inserted, 5);
        assert_eq!(got.outcome, SyncRunOutcome::Applied);
    }

    #[tokio::test]
    async fn recent_returns_newest_first() {
        let ledger = InMemorySyncRunLedger::new();
        for i in 0..5 {
            ledger
                .append(SyncRunEntry::started(
                    format!("run-{}", i),
                    SyncRunProvider::CsvImport,
                    SyncRunMode::OneOff,
                ))
                .await
                .unwrap();
        }
        let recent = ledger.recent(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].run_id, "run-4");
        assert_eq!(recent[2].run_id, "run-2");
    }

    #[tokio::test]
    async fn recent_by_provider_filters() {
        let ledger = InMemorySyncRunLedger::new();
        ledger
            .append(SyncRunEntry::started(
                "p1",
                SyncRunProvider::Plaid,
                SyncRunMode::Initial,
            ))
            .await
            .unwrap();
        ledger
            .append(SyncRunEntry::started(
                "y1",
                SyncRunProvider::Yahoo,
                SyncRunMode::Incremental,
            ))
            .await
            .unwrap();
        ledger
            .append(SyncRunEntry::started(
                "p2",
                SyncRunProvider::Plaid,
                SyncRunMode::Incremental,
            ))
            .await
            .unwrap();

        let plaids = ledger
            .recent_by_provider(SyncRunProvider::Plaid, 10)
            .await
            .unwrap();
        assert_eq!(plaids.len(), 2);
        assert!(plaids.iter().all(|e| e.provider == SyncRunProvider::Plaid));
    }

    #[tokio::test]
    async fn fifo_truncation_drops_oldest_over_cap() {
        let ledger = InMemorySyncRunLedger::with_capacity(3);
        for i in 0..5 {
            ledger
                .append(SyncRunEntry::started(
                    format!("r-{}", i),
                    SyncRunProvider::Manual,
                    SyncRunMode::OneOff,
                ))
                .await
                .unwrap();
        }
        assert_eq!(ledger.len(), 3);
        // Oldest two (r-0, r-1) dropped.
        assert!(ledger.get("r-0").await.unwrap().is_none());
        assert!(ledger.get("r-1").await.unwrap().is_none());
        assert!(ledger.get("r-4").await.unwrap().is_some());
    }

    #[test]
    fn derive_outcome_classifies_correctly() {
        let s = SyncRunSummary {
            inserted: 0,
            errors: 1,
            ..Default::default()
        };
        assert_eq!(s.derive_outcome(), SyncRunOutcome::Failed);

        let s = SyncRunSummary {
            inserted: 5,
            warnings: 1,
            ..Default::default()
        };
        assert_eq!(s.derive_outcome(), SyncRunOutcome::AppliedWithWarnings);

        let s = SyncRunSummary {
            inserted: 5,
            ..Default::default()
        };
        assert_eq!(s.derive_outcome(), SyncRunOutcome::Applied);

        // Errors with partial success → applied-with-warnings.
        let s = SyncRunSummary {
            inserted: 3,
            errors: 2,
            ..Default::default()
        };
        assert_eq!(s.derive_outcome(), SyncRunOutcome::AppliedWithWarnings);
    }

    #[tokio::test]
    async fn fail_stamps_error_json_and_outcome() {
        let ledger = InMemorySyncRunLedger::new();
        let entry = SyncRunEntry::started("r1", SyncRunProvider::Plaid, SyncRunMode::Initial)
            .fail(r#"{"__mizan_error":true,"code":"PLAID_AUTH_EXPIRED"}"#);
        ledger.append(entry).await.unwrap();

        let got = ledger.get("r1").await.unwrap().unwrap();
        assert_eq!(got.outcome, SyncRunOutcome::Failed);
        assert!(got.error.unwrap().contains("PLAID_AUTH_EXPIRED"));
    }
}
