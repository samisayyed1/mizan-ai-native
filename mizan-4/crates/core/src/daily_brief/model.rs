//! Daily Brief data types.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Net-worth movement vs the previous brief / previous snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthDelta {
    pub previous: Decimal,
    pub current: Decimal,
    pub absolute: Decimal,
    /// Percentage as a Decimal (e.g. 0.0125 = +1.25%). Optional when
    /// previous = 0 (no prior baseline to compute against).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<Decimal>,
}

impl NetWorthDelta {
    pub fn new(previous: Decimal, current: Decimal) -> Self {
        let absolute = current - previous;
        let percent = if previous.is_zero() {
            None
        } else {
            Some(absolute / previous)
        };
        Self {
            previous,
            current,
            absolute,
            percent,
        }
    }
}

/// One holding's contribution to the day's delta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingMover {
    /// Stable asset id (e.g. "SEC:AAPL:XNAS" or "PROP-a1b2c3d4").
    pub asset_id: String,
    pub display_name: String,
    pub previous_value: Decimal,
    pub current_value: Decimal,
    pub delta: Decimal,
    /// Same convention as NetWorthDelta.percent (0.05 = +5%).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<Decimal>,
    pub currency: String,
}

/// Allocation drift entry — one category that's off-target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationDriftEntry {
    pub category_key: String,
    pub target_pct: Decimal,
    pub actual_pct: Decimal,
    /// `actual - target`, signed.
    pub drift_pct: Decimal,
}

/// A signal that's gone stale (FX rate, market quote, broker sync).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleSignal {
    /// Free-form label (e.g. "FX:USD:INR", "QUOTE:AAPL", "PLAID:plaid-conn-abc").
    pub key: String,
    pub age_hours: i64,
}

/// Pending AI draft summary — drafts the user opened but didn't confirm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDraftSummary {
    pub tool_name: String,
    pub count: u32,
}

/// One full daily brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyBrief {
    pub brief_date: NaiveDate,
    pub base_currency: String,
    pub net_worth: NetWorthDelta,
    pub top_movers: Vec<HoldingMover>,
    pub allocation_drift: Vec<AllocationDriftEntry>,
    pub stale_signals: Vec<StaleSignal>,
    pub pending_drafts: Vec<PendingDraftSummary>,
    pub computed_at: DateTime<Utc>,
    /// True once the user has opened / read the brief. The Settings →
    /// Notifications panel renders an unread dot from this.
    pub read: bool,
}

impl DailyBrief {
    /// Builder: start with date + currency + net-worth delta, layer on
    /// the optional sections.
    pub fn new(
        brief_date: NaiveDate,
        base_currency: impl Into<String>,
        net_worth: NetWorthDelta,
    ) -> Self {
        Self {
            brief_date,
            base_currency: base_currency.into(),
            net_worth,
            top_movers: Vec::new(),
            allocation_drift: Vec::new(),
            stale_signals: Vec::new(),
            pending_drafts: Vec::new(),
            computed_at: Utc::now(),
            read: false,
        }
    }

    pub fn with_movers(mut self, movers: Vec<HoldingMover>) -> Self {
        self.top_movers = movers;
        self
    }

    pub fn with_drift(mut self, drift: Vec<AllocationDriftEntry>) -> Self {
        self.allocation_drift = drift;
        self
    }

    pub fn with_stale_signals(mut self, signals: Vec<StaleSignal>) -> Self {
        self.stale_signals = signals;
        self
    }

    pub fn with_pending_drafts(mut self, drafts: Vec<PendingDraftSummary>) -> Self {
        self.pending_drafts = drafts;
        self
    }

    /// True when the brief has at least one signal worth surfacing
    /// (delta > 0, movers, drift, stale signals, pending drafts).
    /// The Settings panel hides "nothing-happened" days.
    pub fn is_actionable(&self) -> bool {
        !self.net_worth.absolute.is_zero()
            || !self.top_movers.is_empty()
            || !self.allocation_drift.is_empty()
            || !self.stale_signals.is_empty()
            || !self.pending_drafts.is_empty()
    }
}
