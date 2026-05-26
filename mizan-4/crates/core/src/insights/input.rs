//! Typed input the deterministic insights engine consumes.
//!
//! Why a dedicated input struct rather than passing repositories?
//! Two reasons:
//!   - **Pure.** The engine stays a `fn(&Input) -> Vec<Notification>`,
//!     which means unit tests + fixtures + golden outputs without
//!     standing up a DB.
//!   - **Versionable.** When we add a new rule, we extend `InsightsInput`
//!     with the field that rule needs. Older callers compile but skip
//!     the rule because the field is `None`/empty. No coupling between
//!     scheduler upgrades and rule additions.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A single position's day-over-day move (in base currency). The scheduler
/// computes this from the latest two `holdings_snapshots` rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoldingDayMove {
    /// Display symbol (e.g. "PLTR", "VWRA.L"). Used in notification copy.
    pub symbol: String,
    /// Optional human asset name; the engine prefers `symbol` when copy
    /// space is tight but uses this in deep-link payloads.
    pub asset_name: Option<String>,
    pub asset_id: Option<String>,
    /// Closing price yesterday, in base currency.
    pub prev_price_base: Decimal,
    /// Closing price today, in base currency.
    pub curr_price_base: Decimal,
    /// `(curr_price_base - prev_price_base) / prev_price_base`. Pre-computed
    /// by the caller so the engine doesn't have to guard against
    /// divide-by-zero or sub-cent denominators.
    pub change_pct: Decimal,
    /// Current market value of the user's holding (in base). Used by the
    /// engine to suppress big-mover notifications on tiny positions —
    /// nobody cares that their $14 holding moved 8%.
    pub current_value_base: Decimal,
}

/// One per goal the user is tracking. Progress is the fraction in
/// `[0.0, 1.0+]`; >1.0 means the goal is overfunded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal_id: String,
    pub title: String,
    /// Previous evaluation's progress fraction (from the prior insights
    /// run, snapshotted in `payload_json` of the last GoalMilestone
    /// notification — or 0.0 if the engine has never seen this goal).
    /// Used to suppress repeat milestones across multiple runs in a day.
    pub previous_progress: Decimal,
    pub current_progress: Decimal,
    pub current_value_base: Decimal,
    pub target_value_base: Option<Decimal>,
}

/// A single net worth history point, base currency. Most recent at the
/// end of the vec. The engine reads the last point + a 7-days-ago point
/// for the NetWorthDip + NewAth rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetWorthHistoryPoint {
    pub date: NaiveDate,
    pub net_worth_base: Decimal,
}

/// A sync that failed and is degrading data quality the user can see.
/// Source includes the provider slug for the deep-link target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncFailureInput {
    /// E.g. "plaid", "yahoo", "tradingview", "fx".
    pub provider: String,
    /// One-line failure reason, already redacted of secrets by the
    /// caller. Surfaced verbatim in the notification body.
    pub reason: String,
    pub last_success_at_ms: Option<i64>,
}

/// Bundle the scheduler passes to `evaluate()`. All fields optional /
/// vec-empty so callers can hydrate progressively as more data sources
/// come online.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InsightsInput {
    /// Evaluation date in the user's local timezone, used for dedupe
    /// keys (so the engine fires at most once per logical day per rule
    /// even across multiple ticks).
    pub today: Option<NaiveDate>,
    /// Base currency for copy formatting.
    pub base_currency: String,
    /// Per-holding day moves, sorted however the caller likes (the
    /// engine re-sorts internally by abs(change_pct) when it picks
    /// the top mover).
    pub holding_moves: Vec<HoldingDayMove>,
    pub goal_progress: Vec<GoalProgress>,
    /// Net worth points, ASCENDING by date. The engine looks at
    /// `last()` for ATH and `last() vs. 7-days-ago` for the dip rule.
    pub net_worth_history: Vec<NetWorthHistoryPoint>,
    /// Net worth all-time high known to the caller (so the ATH rule
    /// doesn't fire every time the engine doesn't have full history).
    /// `None` = unknown, suppress the ATH rule.
    pub previous_ath: Option<Decimal>,
    /// Cash percentage of net worth, as a `[0.0, 1.0]` fraction. The
    /// CashDrag rule fires when this is > 0.10 AND a `cash_high_for_days`
    /// counter (computed by the caller) crosses 30.
    pub cash_pct_of_net_worth: Option<Decimal>,
    pub cash_high_for_days: Option<u32>,
    /// Any active sync failures the user hasn't yet seen.
    pub sync_failures: Vec<SyncFailureInput>,
}
