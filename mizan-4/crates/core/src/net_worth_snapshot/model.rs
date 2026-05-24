//! Net Worth Snapshot data types.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// What computed this snapshot. Important for support diagnostics —
/// a "user opened the app" snapshot is more trustworthy than a
/// "background scheduler ran the wrong day" snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    /// Background scheduler fired at UTC midnight (or user local).
    Scheduler,
    /// User opened the app and the dashboard recomputed.
    AppOpen,
    /// User clicked Settings → Sync → Snapshot Now.
    ManualTrigger,
    /// Snapshot was reconstructed from historical data (backfill).
    Backfill,
}

impl SnapshotSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::AppOpen => "app_open",
            Self::ManualTrigger => "manual_trigger",
            Self::Backfill => "backfill",
        }
    }
}

/// One row in the breakdown — by asset class, account type, or whichever
/// grouping the caller used. The frontend renders this as a stacked
/// area chart on the dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthBreakdownEntry {
    /// Stable key (e.g. "SECURITIES", "CASH", "LIABILITY", "PROPERTY").
    pub key: String,
    pub value: Decimal,
}

/// One full snapshot point. Decimals stay as Decimals so cumulative
/// daily totals don't drift into float lossiness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthSnapshot {
    pub snapshot_date: NaiveDate,
    pub base_currency: String,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    /// `total_assets - total_liabilities`.
    pub net_worth: Decimal,
    /// Per-kind breakdown so the chart can stack categories.
    pub breakdown: Vec<NetWorthBreakdownEntry>,
    pub source: SnapshotSource,
    /// When this row was last written (or rewritten — same-day replays
    /// bump this without changing snapshot_date).
    pub recorded_at: DateTime<Utc>,
}

impl NetWorthSnapshot {
    /// Convenience for the "this was reconstructed for a past date"
    /// backfill flow: bypasses the source check.
    pub fn backfilled(input: NetWorthSnapshotInput) -> Self {
        let net_worth = input.total_assets - input.total_liabilities;
        Self {
            snapshot_date: input.snapshot_date,
            base_currency: input.base_currency,
            total_assets: input.total_assets,
            total_liabilities: input.total_liabilities,
            net_worth,
            breakdown: input.breakdown,
            source: SnapshotSource::Backfill,
            recorded_at: Utc::now(),
        }
    }
}

/// The fields a caller computes when emitting a snapshot. The service
/// derives net_worth, recorded_at, etc.
#[derive(Debug, Clone, PartialEq)]
pub struct NetWorthSnapshotInput {
    pub snapshot_date: NaiveDate,
    pub base_currency: String,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub breakdown: Vec<NetWorthBreakdownEntry>,
    pub source: SnapshotSource,
}

impl NetWorthSnapshotInput {
    /// Build the persisted shape from the inputs the call site computed.
    pub fn into_snapshot(self) -> NetWorthSnapshot {
        let net_worth = self.total_assets - self.total_liabilities;
        NetWorthSnapshot {
            snapshot_date: self.snapshot_date,
            base_currency: self.base_currency,
            total_assets: self.total_assets,
            total_liabilities: self.total_liabilities,
            net_worth,
            breakdown: self.breakdown,
            source: self.source,
            recorded_at: Utc::now(),
        }
    }
}
