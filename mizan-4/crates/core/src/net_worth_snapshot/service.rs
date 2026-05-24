//! Net Worth Snapshot trait + in-memory implementation.

use std::sync::RwLock;

use async_trait::async_trait;
use chrono::NaiveDate;

use super::model::{NetWorthSnapshot, NetWorthSnapshotInput};
use crate::Result;

/// Write-once-per-day net-worth snapshot store. Re-running on the same
/// date overwrites the existing row (so a same-day re-open doesn't
/// double-count).
#[async_trait]
pub trait NetWorthSnapshotService: Send + Sync {
    /// Persist a snapshot for `input.snapshot_date`. If a row already
    /// exists for that date, it's replaced (same-day re-computation
    /// is idempotent). Returns the persisted snapshot.
    async fn upsert(&self, input: NetWorthSnapshotInput) -> Result<NetWorthSnapshot>;

    /// All snapshots between `from` and `to` (inclusive), sorted by
    /// snapshot_date ascending. Empty when no rows in the range.
    async fn range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<NetWorthSnapshot>>;

    /// Most recent snapshot, if any.
    async fn latest(&self) -> Result<Option<NetWorthSnapshot>>;

    /// Snapshot for a specific date, if any.
    async fn get(&self, date: NaiveDate) -> Result<Option<NetWorthSnapshot>>;
}

pub struct InMemoryNetWorthSnapshotService {
    snapshots: RwLock<Vec<NetWorthSnapshot>>,
}

impl InMemoryNetWorthSnapshotService {
    pub fn new() -> Self {
        Self {
            snapshots: RwLock::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.snapshots.read().expect("snapshots poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemoryNetWorthSnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetWorthSnapshotService for InMemoryNetWorthSnapshotService {
    async fn upsert(&self, input: NetWorthSnapshotInput) -> Result<NetWorthSnapshot> {
        let snapshot = input.into_snapshot();
        let mut store = self.snapshots.write().expect("snapshots poisoned");
        // Same-date replace; sort by date so range() can binary-search.
        if let Some(slot) = store
            .iter_mut()
            .find(|s| s.snapshot_date == snapshot.snapshot_date)
        {
            *slot = snapshot.clone();
        } else {
            store.push(snapshot.clone());
            store.sort_by_key(|s| s.snapshot_date);
        }
        Ok(snapshot)
    }

    async fn range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<NetWorthSnapshot>> {
        let store = self.snapshots.read().expect("snapshots poisoned");
        Ok(store
            .iter()
            .filter(|s| s.snapshot_date >= from && s.snapshot_date <= to)
            .cloned()
            .collect())
    }

    async fn latest(&self) -> Result<Option<NetWorthSnapshot>> {
        let store = self.snapshots.read().expect("snapshots poisoned");
        Ok(store.last().cloned())
    }

    async fn get(&self, date: NaiveDate) -> Result<Option<NetWorthSnapshot>> {
        let store = self.snapshots.read().expect("snapshots poisoned");
        Ok(store.iter().find(|s| s.snapshot_date == date).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_worth_snapshot::model::{
        NetWorthBreakdownEntry, NetWorthSnapshotInput, SnapshotSource,
    };
    use rust_decimal_macros::dec;

    fn sample(date: NaiveDate, assets: i64, liab: i64) -> NetWorthSnapshotInput {
        NetWorthSnapshotInput {
            snapshot_date: date,
            base_currency: "USD".to_string(),
            total_assets: dec!(1) * Decimal::from(assets),
            total_liabilities: dec!(1) * Decimal::from(liab),
            breakdown: vec![
                NetWorthBreakdownEntry {
                    key: "SECURITIES".to_string(),
                    value: Decimal::from(assets),
                },
                NetWorthBreakdownEntry {
                    key: "LIABILITY".to_string(),
                    value: Decimal::from(liab),
                },
            ],
            source: SnapshotSource::Scheduler,
        }
    }

    use rust_decimal::Decimal;

    #[tokio::test]
    async fn upsert_inserts_then_replaces_same_day() {
        let svc = InMemoryNetWorthSnapshotService::new();
        let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();

        let first = svc.upsert(sample(date, 100_000, 20_000)).await.unwrap();
        assert_eq!(first.net_worth, dec!(80_000));
        assert_eq!(svc.len(), 1);

        // Re-upsert same date — replaces, doesn't append.
        let second = svc.upsert(sample(date, 110_000, 20_000)).await.unwrap();
        assert_eq!(second.net_worth, dec!(90_000));
        assert_eq!(svc.len(), 1, "same-date upsert should replace, not append");
    }

    #[tokio::test]
    async fn range_returns_inclusive_window_sorted() {
        let svc = InMemoryNetWorthSnapshotService::new();
        for day in [10, 11, 12, 13, 14] {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample(d, day as i64 * 1_000, 0)).await.unwrap();
        }
        let got = svc
            .range(
                NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
                NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].snapshot_date.day(), 11);
        assert_eq!(got[2].snapshot_date.day(), 13);
    }

    #[tokio::test]
    async fn latest_returns_most_recent() {
        let svc = InMemoryNetWorthSnapshotService::new();
        for day in [10, 11, 12] {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample(d, 1_000, 0)).await.unwrap();
        }
        let latest = svc.latest().await.unwrap().unwrap();
        assert_eq!(latest.snapshot_date.day(), 12);
    }

    #[tokio::test]
    async fn get_returns_exact_date() {
        let svc = InMemoryNetWorthSnapshotService::new();
        let d = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        svc.upsert(sample(d, 50_000, 10_000)).await.unwrap();

        let got = svc.get(d).await.unwrap().unwrap();
        assert_eq!(got.net_worth, dec!(40_000));

        let missing = svc
            .get(NaiveDate::from_ymd_opt(2026, 5, 25).unwrap())
            .await
            .unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn net_worth_derives_from_inputs() {
        let svc = InMemoryNetWorthSnapshotService::new();
        let d = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let s = svc.upsert(sample(d, 250_000, 100_000)).await.unwrap();
        assert_eq!(s.total_assets, dec!(250_000));
        assert_eq!(s.total_liabilities, dec!(100_000));
        assert_eq!(s.net_worth, dec!(150_000));
    }

    use chrono::Datelike;
}
