//! Zakat domain types.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The four classical Sunni schools of jurisprudence (madhāhib). Each
/// has documented variations in how Zakat is calculated; the engine
/// branches on this enum to surface school-specific edge cases.
///
/// - **Hanafi** — the default; uses silver Nisab; PE held >1 lunar year
///   treated as Zakatable on its full value. Working-agreement engine
///   has shipped this rule set as the baseline since v1.
/// - **Shafi'i** — uses gold Nisab; otherwise mirrors Hanafi for the
///   common cases the engine implements today.
/// - **Maliki** — per ADR 0015 (merged 2026-06-04): stricter
///   real-estate intent treatment (Zakat applies only to property
///   held with intent to sell), short-term debts deduction tied to
///   demand of repayment, locked retirement treated as inaccessible
///   wealth (no Zakat until withdrawn). Edge-case enforcement lands
///   in PR-F2.b.
/// - **Hanbali** — per ADR 0016 (merged 2026-06-04): debt deduction
///   broader than Hanafi (allows long-term mortgage principal under
///   the "delayed-debt" doctrine), locked retirement Zakatable on
///   the proportionate annual share. Edge-case enforcement lands in
///   PR-F2.c.
///
/// # PR-F2 scope
///
/// This PR ships the enum + branching plumbing + school-specific note
/// text. The school-specific MATH (Maliki real-estate intent,
/// Hanbali debt deduction) lands in PR-F2.b/c so each ADR's edge
/// cases are reviewed in isolation. All four variants today compute
/// the same arithmetic result; what differs is the `notes` array on
/// the report (the audit trail makes it explicit which school was
/// used so the user's imam can confirm the rule set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum School {
    /// Hanafi — the default school; silver Nisab; broadest Zakat base.
    #[default]
    Hanafi,
    /// Shafi'i — gold Nisab; otherwise close to Hanafi.
    #[serde(rename = "shafii", alias = "shafi'i", alias = "shafi-i")]
    Shafii,
    /// Maliki — strict real-estate intent treatment per ADR 0015.
    Maliki,
    /// Hanbali — broader debt deduction per ADR 0016.
    Hanbali,
}

impl School {
    /// Display label for the report's audit trail.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Hanafi => "Hanafi",
            Self::Shafii => "Shafi'i",
            Self::Maliki => "Maliki",
            Self::Hanbali => "Hanbali",
        }
    }

    /// Parse a free-text school selector (typically from `user_memory`)
    /// against the canonical names. Returns `None` for unrecognised
    /// strings; the caller surfaces `ZakatError::UnknownSchool(...)`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "hanafi" => Some(Self::Hanafi),
            "shafii" | "shafi'i" | "shafi-i" | "shafi" => Some(Self::Shafii),
            "maliki" => Some(Self::Maliki),
            "hanbali" => Some(Self::Hanbali),
            _ => None,
        }
    }

    /// School-specific note text appended to the report's `notes` so
    /// the user's imam can verify the rule set used. Sourced from the
    /// docs/adr/0015 (Maliki) + 0016 (Hanbali) ADRs.
    pub fn school_note(&self) -> &'static str {
        match self {
            Self::Hanafi => {
                "Calculated under Hanafi rules (default). Silver Nisab; \
                 PE held over one lunar year treated as Zakatable at full \
                 value. Cross-check with your imam for edge cases."
            }
            Self::Shafii => {
                "Calculated under Shafi'i rules. Gold Nisab; long-term \
                 investment positions held for income (not resale) may be \
                 exempt — cross-check with your imam."
            }
            Self::Maliki => {
                "Calculated under Maliki rules per ADR 0015. Real estate \
                 is Zakatable only when held with intent to sell; short-term \
                 debt deduction depends on demand of repayment; locked \
                 retirement is exempt until withdrawn. PR-F2.b will enforce \
                 these edge cases in the math; today the audit trail \
                 surfaces the school selection for your imam to verify."
            }
            Self::Hanbali => {
                "Calculated under Hanbali rules per ADR 0016. Broader debt \
                 deduction including long-term mortgage principal under the \
                 delayed-debt doctrine; locked retirement Zakatable on the \
                 proportionate annual share. PR-F2.c will enforce these \
                 edge cases in the math; today the audit trail surfaces the \
                 school selection for your imam to verify."
            }
        }
    }
}

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
    /// Scholarly school to use. Defaults to Hanafi for backward
    /// compatibility with callers that haven't been updated yet.
    /// PR-F2.b/c will read this to branch on Maliki/Hanbali edge cases.
    #[serde(default)]
    pub school: School,
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
    /// School used for the calculation. Surfaced on the report so the
    /// audit trail (and any Truth Ledger entry per CLAUDE.md §0 rule 1)
    /// records exactly which rule set produced the number.
    #[serde(default)]
    pub school: School,
}
