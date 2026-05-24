//! Daily Brief trait + in-memory implementation.

use std::sync::RwLock;

use async_trait::async_trait;
use chrono::NaiveDate;

use super::model::DailyBrief;
use crate::Result;

#[async_trait]
pub trait DailyBriefService: Send + Sync {
    /// Persist a freshly-computed brief. Re-running on the same date
    /// replaces the row (idempotent same-day recompute, e.g. user
    /// opened the app twice).
    async fn upsert(&self, brief: DailyBrief) -> Result<()>;

    /// Most recent brief, if any.
    async fn latest(&self) -> Result<Option<DailyBrief>>;

    /// Brief for a specific date.
    async fn get(&self, date: NaiveDate) -> Result<Option<DailyBrief>>;

    /// Recent N briefs, newest first, capped at `limit`. Used by the
    /// Settings → Notifications panel.
    async fn recent(&self, limit: usize) -> Result<Vec<DailyBrief>>;

    /// Mark a brief read.
    async fn mark_read(&self, date: NaiveDate) -> Result<()>;
}

pub struct InMemoryDailyBriefService {
    briefs: RwLock<Vec<DailyBrief>>,
}

impl InMemoryDailyBriefService {
    pub fn new() -> Self {
        Self {
            briefs: RwLock::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.briefs.read().expect("briefs poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemoryDailyBriefService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DailyBriefService for InMemoryDailyBriefService {
    async fn upsert(&self, brief: DailyBrief) -> Result<()> {
        let mut store = self.briefs.write().expect("briefs poisoned");
        if let Some(slot) = store.iter_mut().find(|b| b.brief_date == brief.brief_date) {
            *slot = brief;
        } else {
            store.push(brief);
            store.sort_by_key(|b| b.brief_date);
        }
        Ok(())
    }

    async fn latest(&self) -> Result<Option<DailyBrief>> {
        let store = self.briefs.read().expect("briefs poisoned");
        Ok(store.last().cloned())
    }

    async fn get(&self, date: NaiveDate) -> Result<Option<DailyBrief>> {
        let store = self.briefs.read().expect("briefs poisoned");
        Ok(store.iter().find(|b| b.brief_date == date).cloned())
    }

    async fn recent(&self, limit: usize) -> Result<Vec<DailyBrief>> {
        let store = self.briefs.read().expect("briefs poisoned");
        Ok(store.iter().rev().take(limit).cloned().collect())
    }

    async fn mark_read(&self, date: NaiveDate) -> Result<()> {
        let mut store = self.briefs.write().expect("briefs poisoned");
        if let Some(slot) = store.iter_mut().find(|b| b.brief_date == date) {
            slot.read = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daily_brief::model::{HoldingMover, NetWorthDelta};
    use rust_decimal_macros::dec;

    fn sample_brief(date: NaiveDate) -> DailyBrief {
        let nw = NetWorthDelta::new(dec!(100_000), dec!(101_500));
        DailyBrief::new(date, "USD", nw).with_movers(vec![HoldingMover {
            asset_id: "SEC:AAPL:XNAS".into(),
            display_name: "Apple".into(),
            previous_value: dec!(15_000),
            current_value: dec!(15_500),
            delta: dec!(500),
            percent: Some(dec!(0.033)),
            currency: "USD".into(),
        }])
    }

    #[tokio::test]
    async fn upsert_inserts_then_replaces_same_day() {
        let svc = InMemoryDailyBriefService::new();
        let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        svc.upsert(sample_brief(date)).await.unwrap();
        assert_eq!(svc.len(), 1);

        let mut b = sample_brief(date);
        b.read = true;
        svc.upsert(b).await.unwrap();
        assert_eq!(svc.len(), 1);
        assert!(svc.get(date).await.unwrap().unwrap().read);
    }

    #[tokio::test]
    async fn latest_returns_newest() {
        let svc = InMemoryDailyBriefService::new();
        for day in [10, 11, 12] {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample_brief(d)).await.unwrap();
        }
        let latest = svc.latest().await.unwrap().unwrap();
        use chrono::Datelike;
        assert_eq!(latest.brief_date.day(), 12);
    }

    #[tokio::test]
    async fn recent_caps_at_limit() {
        let svc = InMemoryDailyBriefService::new();
        for day in 1..=10 {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample_brief(d)).await.unwrap();
        }
        let recent = svc.recent(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        use chrono::Datelike;
        assert_eq!(recent[0].brief_date.day(), 10);
        assert_eq!(recent[2].brief_date.day(), 8);
    }

    #[tokio::test]
    async fn mark_read_flips_flag() {
        let svc = InMemoryDailyBriefService::new();
        let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        svc.upsert(sample_brief(date)).await.unwrap();
        assert!(!svc.get(date).await.unwrap().unwrap().read);
        svc.mark_read(date).await.unwrap();
        assert!(svc.get(date).await.unwrap().unwrap().read);
    }

    #[tokio::test]
    async fn is_actionable_filters_no_op_days() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let nw = NetWorthDelta::new(dec!(100_000), dec!(100_000));
        let empty = DailyBrief::new(date, "USD", nw);
        assert!(!empty.is_actionable());

        let nw_moved = NetWorthDelta::new(dec!(100_000), dec!(100_500));
        let real = DailyBrief::new(date, "USD", nw_moved);
        assert!(real.is_actionable());
    }

    #[test]
    fn net_worth_delta_handles_zero_baseline() {
        let nw = NetWorthDelta::new(dec!(0), dec!(1_000));
        assert_eq!(nw.absolute, dec!(1_000));
        assert!(nw.percent.is_none());
    }
}
