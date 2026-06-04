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

/// One dividend or interest payment posted since the last insights tick.
///
/// Caller responsibilities:
///   - de-duplicate by `activity_id` across runs (the engine emits a
///     dedupe_key keyed on it, so re-feeding the same activity is a
///     no-op at the storage layer too),
///   - convert `amount_base` to the user's base currency *before*
///     passing it in — the engine does no FX (it stays pure).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DividendEvent {
    /// Source `activities.id` — used in the dedupe_key.
    pub activity_id: String,
    /// "DIVIDEND" or "INTEREST".
    pub kind: String,
    /// Display symbol or asset name; falls back to "Cash" for portfolio-
    /// level income with no asset_id.
    pub symbol: String,
    /// Settled-date in the user's local timezone, used in copy.
    pub posted_on: NaiveDate,
    pub amount_base: Decimal,
}

/// One bond / sukuk position whose maturity date is upcoming. Caller
/// computes `days_to_maturity` against the user's local "today" so
/// the engine stays pure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BondMaturityCandidate {
    pub holding_id: String,
    /// Display symbol or ISIN (e.g. "EMAAR 6.5 2026").
    pub symbol: String,
    /// Maturity date — used in copy + dedupe key.
    pub maturity_date: NaiveDate,
    /// Days remaining until maturity from the user's local "today".
    /// Caller computes; engine compares against 90/30/7/1 thresholds.
    pub days_to_maturity: i64,
    /// Principal returning at maturity in base currency (for copy).
    pub principal_returning_base: Decimal,
}

/// One FX pair the user has material exposure to that has moved beyond
/// the materiality threshold over the comparison window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxPairMove {
    /// Base currency of the pair (e.g. "USD").
    pub from_currency: String,
    /// Quote currency of the pair (e.g. "INR").
    pub to_currency: String,
    /// Window over which the move was computed (in days).
    pub window_days: u32,
    /// Pre-computed pct change over the window. Engine compares
    /// against FX_MATERIAL_MOVE_PCT.
    pub change_pct: Decimal,
    /// User's net exposure to this pair in base currency (so the
    /// engine can suppress small-exposure noise).
    pub exposure_base: Decimal,
}

/// A holding whose AAOIFI screening verdict changed since the prior
/// evaluation. Caller computes the flip (either direction) and hands
/// the engine the before/after pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShariaStatusChange {
    pub holding_id: String,
    pub symbol: String,
    /// Prior verdict (e.g. "compliant"). Free-form string so the
    /// engine doesn't have to know the AAOIFI worker's verdict enum.
    pub prior_verdict: String,
    /// New verdict (e.g. "mixed").
    pub new_verdict: String,
}

/// One Zakat-Hawl anchor approaching completion. Sourced from
/// `hawl_anchors` (Track F PR-F1). Caller computes
/// `days_to_completion` against the user's local "today" (lunar-year
/// from `anchor_date`) so the engine stays pure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HawlAnchorCandidate {
    /// Stable id from `hawl_anchors.cohort_id`.
    pub cohort_id: String,
    /// Human label for the cohort (e.g. "Cash + Equities").
    pub cohort_label: String,
    /// Days remaining until lunar-year completion. Engine compares
    /// against 30/7/1 thresholds.
    pub days_to_completion: i64,
    /// Current Zakatable balance for this cohort, base currency.
    /// Used in copy to surface the qualifying amount.
    pub qualifying_amount_base: Decimal,
}

/// A concentration risk finding — caller pre-computes the dimension
/// (issuer / sector / currency / geography) and the exposure percent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcentrationRiskFinding {
    /// Dimension key: "issuer" / "sector" / "currency" / "geography".
    pub dimension: String,
    /// Concrete identifier within the dimension (e.g. "Apple" /
    /// "Technology" / "USD" / "United States").
    pub label: String,
    /// Fraction of net worth in this concentration as a `[0, 1]`
    /// number. Engine compares against CONCENTRATION_THRESHOLD_PCT.
    pub fraction_of_net_worth: Decimal,
    /// Absolute exposure in base currency (for copy).
    pub exposure_base: Decimal,
}

/// A cash-drag opportunity — cash sitting at low yield while a
/// higher-yielding alternative exists. Distinct from CashDrag which
/// fires on duration; this fires on the presence of an alternative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashDragOpportunityCandidate {
    /// Current cash balance in base currency.
    pub cash_amount_base: Decimal,
    /// Current effective yield as a `[0, 1]` fraction.
    pub current_yield_pct: Decimal,
    /// Yield available on the suggested alternative (e.g. a
    /// Sharia-compatible money market fund) as a `[0, 1]` fraction.
    pub alternative_yield_pct: Decimal,
    /// Display label for the alternative (e.g. "Wahed Cash Plus").
    pub alternative_label: String,
}

/// A tax-optimization window opening — caller computes from the
/// user's jurisdiction (CPF SA top-up cutoff, IRA contribution
/// deadline, capital-gains harvesting opportunity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaxOptimizationWindow {
    /// Stable kind slug ("cpf_sa_top_up", "ira_deadline",
    /// "capital_gains_harvest", "401k_deadline", "nps_deadline").
    pub kind: String,
    /// Days until the window closes. Engine compares against
    /// TAX_WINDOW_DAY_THRESHOLDS.
    pub days_remaining: i64,
    /// Display label (e.g. "CPF SA top-up cutoff").
    pub label: String,
    /// Maximum savings the user could realise by acting (base
    /// currency), or `None` if unknown.
    pub potential_savings_base: Option<Decimal>,
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
    /// Dividend / interest payments posted since the last tick. One
    /// notification emitted per event so the user gets credit for each
    /// individual payment in the activity history.
    pub dividend_events: Vec<DividendEvent>,
    /// Bond / sukuk positions approaching maturity. Engine emits at
    /// most one notification per holding per crossed threshold
    /// (90/30/7/1 day) per Goal v3 §V step C5.a / Track C PR-C5.a.
    pub bond_maturity_candidates: Vec<BondMaturityCandidate>,
    /// FX pairs whose move over `window_days` exceeded
    /// FX_MATERIAL_MOVE_PCT against `exposure_base`. Engine emits one
    /// per pair per day.
    pub fx_pair_moves: Vec<FxPairMove>,
    /// Holdings whose AAOIFI screening verdict flipped. Engine emits
    /// one per holding per day.
    pub sharia_status_changes: Vec<ShariaStatusChange>,
    /// Zakat-Hawl anchors approaching completion. Sourced from
    /// `hawl_anchors` (Track F PR-F1). Engine emits at most one
    /// notification per cohort per crossed threshold (30/7/1 day).
    pub hawl_anchors_approaching: Vec<HawlAnchorCandidate>,
    /// Concentration-risk findings — caller pre-computes the
    /// dimension + label + exposure. Engine fires per finding above
    /// CONCENTRATION_THRESHOLD_PCT.
    pub concentration_findings: Vec<ConcentrationRiskFinding>,
    /// Cash-drag opportunities — caller surfaces when a higher-
    /// yielding alternative exists. Engine fires once per day if the
    /// yield-gap and amount thresholds are met.
    pub cash_drag_opportunities: Vec<CashDragOpportunityCandidate>,
    /// Tax-deadline windows — caller computes from the user's
    /// jurisdiction. Engine fires per window per crossed threshold.
    pub tax_optimization_windows: Vec<TaxOptimizationWindow>,
}
