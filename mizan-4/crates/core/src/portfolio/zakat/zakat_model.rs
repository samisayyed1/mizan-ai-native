//! Zakat domain types.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Inputs to a Zakat assessment, expressed in the user's base currency.
///
/// The service converts each component into base-currency before passing it
/// here; consumers don't need to think about FX. All fields are non-negative
/// totals at the assessment date.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZakatInputs {
    /// Cash + bank balances + cash-equivalents (money-market funds, etc.).
    pub liquid_cash: Decimal,
    /// Spot value of physical gold + silver + other precious metals.
    pub precious_metals: Decimal,
    /// Marketable securities held with intent to trade (stocks, ETFs, sukuk
    /// held for resale). Long-term-investment positions where the user holds
    /// for income are debated in jurisprudence — see `notes` on the result.
    pub tradable_assets: Decimal,
    /// Debts the user owes that fall due within the lunar year (per Hanafi
    /// practice — other schools count differently; see `notes`).
    pub short_term_debts: Decimal,
    /// The Nisab threshold *in the same base currency* — typically the
    /// equivalent of 85 g gold or 595 g silver at today's spot.
    pub nisab: Decimal,
    /// Optional: the spot-currency code these totals are denominated in. Used
    /// only for the result's display string; the math is unit-free.
    #[serde(default)]
    pub currency: Option<String>,
}

/// Output of a Zakat assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZakatReport {
    /// Total assessable assets (cash + metals + tradable).
    pub total_assessable_assets: Decimal,
    /// Short-term debts subtracted from the assessable pool.
    pub deductible_debts: Decimal,
    /// `total_assessable_assets - deductible_debts` (can go negative if the
    /// user is net-indebted; in that case Zakat is zero regardless).
    pub net_zakat_base: Decimal,
    /// The Nisab threshold the assessment was compared against.
    pub nisab_threshold: Decimal,
    /// Whether the net base exceeds Nisab. False ⇒ no Zakat due.
    pub is_above_nisab: bool,
    /// 2.5% of `net_zakat_base` when `is_above_nisab`, else zero.
    pub zakat_due: Decimal,
    /// Currency code of every monetary field, copied from the inputs.
    #[serde(default)]
    pub currency: Option<String>,
    /// Disclaimers + jurisprudence notes for the UI to render below the
    /// number. Always at least one entry; the UI is required to show it.
    pub notes: Vec<String>,
}
