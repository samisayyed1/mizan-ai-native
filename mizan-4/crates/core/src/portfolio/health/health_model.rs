//! Portfolio health domain types.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// One position summary used in scoring. The caller pre-converts to base
/// currency; we don't do FX here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthPosition {
    /// Display label for the position (e.g. "AAPL", "Wells Fargo Checking").
    pub label: String,
    /// Asset class tag (e.g. "EQUITY", "BOND", "CASH", "CRYPTO"). The
    /// caller follows whatever taxonomy the allocation service emits.
    pub asset_class: String,
    /// Position value in the user's base currency. Non-negative.
    pub value_base: Decimal,
    /// True when the underlying instrument trades in a currency other
    /// than the user's base.
    pub is_foreign_currency: bool,
    /// True when this position is a cash / bank-account balance. Cash
    /// holdings always count toward the "cash drag" driver regardless of
    /// asset class taxonomy.
    pub is_cash: bool,
}

/// Target allocation for the portfolio. Map of asset class → target weight
/// as a fraction of 1 (e.g. 0.6 for 60%).
pub type TargetAllocation = std::collections::BTreeMap<String, Decimal>;

/// Inputs to a portfolio health assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthInputs {
    /// All positions (post-FX-conversion). Empty input ⇒ score = None.
    pub positions: Vec<HealthPosition>,
    /// Target allocation per asset class. May be empty — in that case the
    /// drift driver scores `100` (no drift to measure against).
    #[serde(default)]
    pub target_allocation: TargetAllocation,
    /// User's base currency code (e.g. "USD"). Used only for display in
    /// the report; FX classification is the caller's responsibility via
    /// `HealthPosition::is_foreign_currency`.
    #[serde(default)]
    pub base_currency: Option<String>,
}

/// Individual driver score with the underlying metric for transparency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDriver {
    /// Short identifier ("concentration", "fxExposure", "cashDrag",
    /// "allocationDrift").
    pub id: String,
    /// Display label.
    pub label: String,
    /// 0–100 score, higher = healthier.
    pub score: Decimal,
    /// Underlying metric value as a fraction of 1 (e.g. 0.45 for "45% in
    /// the top holding"). Lets the UI render a tooltip.
    pub metric: Decimal,
    /// Human-readable summary line for the report.
    pub note: String,
}

/// Output of a health assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    /// Composite 0–100 health score. `None` when the portfolio is empty.
    pub score: Option<Decimal>,
    /// Per-driver breakdown.
    pub drivers: Vec<HealthDriver>,
    /// The driver with the lowest score — surfaced as the report's
    /// callout. `None` when score is None.
    pub worst_driver: Option<HealthDriver>,
    /// Base currency copied from the inputs.
    #[serde(default)]
    pub base_currency: Option<String>,
    /// Disclaimers for the UI.
    pub notes: Vec<String>,
}
