//! Pure AAOIFI Sharia-screening rules — ADR 0012.
//!
//! Every threshold in this file is annotated with its ADR-0012
//! provenance so a future reviewer can trace each decimal back to
//! the AAOIFI standard.
//!
//! # Decimal discipline
//!
//! Per CLAUDE.md §0 rule 2 + §A19, **all** money + ratio math is
//! [`rust_decimal::Decimal`]; no f64 in any code path that ends in
//! a verdict. The fixture inputs are constructed with `dec!(...)` so
//! tests are bit-stable.
//!
//! # No silent fallbacks
//!
//! The screen returns [`Verdict::Unrated`] when an input is missing.
//! It does NOT default a missing ratio to zero or to "passes". This
//! mirrors CLAUDE.md §0 rule 2 in the screening context.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Qualitative-screen verdict on a company's primary business
/// activity. Caller derives this from the GICS classification.
///
/// `Permissible` — none of the prohibited categories per ADR 0012
/// §"Screen 1 — Business Activity".
///
/// `Prohibited(category)` — primary business falls in the AAOIFI
/// prohibited list; the variant carries the matching category name
/// so the rationale string can name it.
///
/// `Mixed` — multi-line business where SOME but not all activity is
/// in the prohibited categories (e.g. tech-conglomerate-with-banking-
/// subsidiary). Drives [`Verdict::Mixed`] for purification ratios.
///
/// `Unknown` — GICS lookup didn't resolve. Drives [`Verdict::Unrated`]
/// — distinct from "verdict-known-and-compliant".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum BusinessActivity {
    Permissible,
    Prohibited(String),
    Mixed,
    Unknown,
}

/// Quantitative-screen ratios from ADR 0012 §"Screen 2 — Financial
/// Ratios". Caller pulls these from SEC EDGAR equivalents and feeds
/// them in already computed.
///
/// Every field is [`Option<Decimal>`] so a missing input surfaces
/// as `Unrated` rather than being silently treated as zero. None
/// across the board → `Unrated`; partial fill → `Unrated` (per
/// "no silent fallbacks").
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FinancialRatios {
    /// Interest-bearing debt / 12-month trailing market cap.
    pub debt_ratio: Option<Decimal>,
    /// (Accounts receivable + cash) / 12-month trailing market cap.
    pub receivables_ratio: Option<Decimal>,
    /// Interest income + other non-permissible / total revenue.
    pub non_permissible_income_ratio: Option<Decimal>,
}

/// Full input bundle for [`screen`]. Caller hydrates the qualitative
/// + quantitative pieces and hands the bundle over.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningInput {
    /// Issuer symbol (uppercased canonical form).
    pub symbol: String,
    /// Qualitative-screen result.
    pub business_activity: BusinessActivity,
    /// Quantitative ratios.
    pub ratios: FinancialRatios,
}

/// Per ADR 0012 §"Output surface" — the four verdict states surfaced
/// to the Mizan Badge layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Passes both screens.
    Compliant,
    /// Passes business activity but has mixed lines — purification
    /// ratio attached.
    Mixed,
    /// Fails business activity OR any financial ratio.
    NonCompliant,
    /// Data inconclusive; surfaced distinctly so the user knows the
    /// screen was attempted.
    Unrated,
}

/// Why the [`screen`] arrived at its [`Verdict`]. Threaded into the
/// `'halal-screened' / 'mixed-compliance'` modifier popover content
/// shipped in PR-E3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreeningRationale {
    /// Stable, machine-readable reason codes (1 per failing check).
    pub reasons: Vec<String>,
    /// Optional purification ratio, in [0, 1], for `Mixed` verdicts.
    pub purification_ratio: Option<Decimal>,
}

// ──── Thresholds per ADR 0012 §"Screen 2 — Financial Ratios" ───────

/// 33% — debt ratio cap.
const DEBT_RATIO_MAX: Decimal = rust_decimal_macros::dec!(0.33);
/// 33% — receivables-plus-cash ratio cap.
const RECEIVABLES_RATIO_MAX: Decimal = rust_decimal_macros::dec!(0.33);
/// 5% — non-permissible-income ratio cap.
const NON_PERMISSIBLE_INCOME_RATIO_MAX: Decimal = rust_decimal_macros::dec!(0.05);

/// Run both screens against the input.
///
/// Order of operations (verbatim from ADR 0012):
///   1. If business activity is `Unknown` → `Unrated`.
///   2. If business activity is `Prohibited(_)` → `NonCompliant`.
///   3. If ANY ratio input is missing → `Unrated`.
///   4. If business activity is `Mixed` AND all three ratios pass →
///      `Mixed` (purification ratio = non_permissible_income_ratio).
///   5. If any ratio exceeds its cap → `NonCompliant`.
///   6. Else → `Compliant`.
///
/// Returns the verdict alongside its rationale. The rationale's
/// `reasons` list is empty on `Compliant`; non-empty on every other
/// verdict so the badge popover never says "non-compliant" without
/// naming what failed.
pub fn screen(input: &ScreeningInput) -> (Verdict, ScreeningRationale) {
    let mut reasons: Vec<String> = Vec::new();

    // Step 1 — unknown qualitative → Unrated.
    if matches!(input.business_activity, BusinessActivity::Unknown) {
        reasons.push("business_activity_unknown".to_string());
        return (
            Verdict::Unrated,
            ScreeningRationale {
                reasons,
                purification_ratio: None,
            },
        );
    }

    // Step 2 — prohibited qualitative → NonCompliant.
    if let BusinessActivity::Prohibited(ref category) = input.business_activity {
        reasons.push(format!("prohibited_business_activity:{category}"));
        return (
            Verdict::NonCompliant,
            ScreeningRationale {
                reasons,
                purification_ratio: None,
            },
        );
    }

    // Step 3 — any missing ratio → Unrated (no silent fallback).
    let (debt, recv, npi) = match (
        input.ratios.debt_ratio,
        input.ratios.receivables_ratio,
        input.ratios.non_permissible_income_ratio,
    ) {
        (Some(d), Some(r), Some(n)) => (d, r, n),
        _ => {
            reasons.push("financial_ratios_incomplete".to_string());
            return (
                Verdict::Unrated,
                ScreeningRationale {
                    reasons,
                    purification_ratio: None,
                },
            );
        }
    };

    // Step 5 — any ratio over cap → NonCompliant.
    let mut any_exceeded = false;
    if debt > DEBT_RATIO_MAX {
        reasons.push(format!("debt_ratio_exceeds:{debt}"));
        any_exceeded = true;
    }
    if recv > RECEIVABLES_RATIO_MAX {
        reasons.push(format!("receivables_ratio_exceeds:{recv}"));
        any_exceeded = true;
    }
    if npi > NON_PERMISSIBLE_INCOME_RATIO_MAX {
        reasons.push(format!("non_permissible_income_exceeds:{npi}"));
        any_exceeded = true;
    }

    if any_exceeded {
        return (
            Verdict::NonCompliant,
            ScreeningRationale {
                reasons,
                purification_ratio: None,
            },
        );
    }

    // Step 4 — mixed qualitative + all ratios pass → Mixed with purification ratio.
    if matches!(input.business_activity, BusinessActivity::Mixed) {
        reasons.push("mixed_business_lines".to_string());
        return (
            Verdict::Mixed,
            ScreeningRationale {
                reasons,
                purification_ratio: Some(npi),
            },
        );
    }

    // Step 6 — Compliant.
    (
        Verdict::Compliant,
        ScreeningRationale {
            reasons,
            purification_ratio: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn input(
        symbol: &str,
        activity: BusinessActivity,
        debt: Option<Decimal>,
        recv: Option<Decimal>,
        npi: Option<Decimal>,
    ) -> ScreeningInput {
        ScreeningInput {
            symbol: symbol.to_string(),
            business_activity: activity,
            ratios: FinancialRatios {
                debt_ratio: debt,
                receivables_ratio: recv,
                non_permissible_income_ratio: npi,
            },
        }
    }

    #[test]
    fn unknown_business_activity_returns_unrated() {
        let i = input("XYZ", BusinessActivity::Unknown, None, None, None);
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::Unrated);
        assert!(r.reasons.contains(&"business_activity_unknown".to_string()));
        assert!(r.purification_ratio.is_none());
    }

    #[test]
    fn prohibited_business_activity_returns_non_compliant() {
        let i = input(
            "BANK",
            BusinessActivity::Prohibited("conventional_banking".to_string()),
            Some(dec!(0.10)),
            Some(dec!(0.10)),
            Some(dec!(0.01)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::NonCompliant);
        assert!(
            r.reasons.iter().any(|s| s.contains("conventional_banking")),
            "rationale must name the prohibited category"
        );
    }

    #[test]
    fn missing_ratios_with_permissible_activity_returns_unrated() {
        let i = input("AAPL", BusinessActivity::Permissible, None, None, None);
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::Unrated);
        assert!(r.reasons.iter().any(|s| s == "financial_ratios_incomplete"));
    }

    #[test]
    fn partial_ratios_returns_unrated() {
        let i = input(
            "AAPL",
            BusinessActivity::Permissible,
            Some(dec!(0.10)),
            None,
            Some(dec!(0.01)),
        );
        let (v, _) = screen(&i);
        assert_eq!(v, Verdict::Unrated);
    }

    #[test]
    fn permissible_with_all_ratios_below_caps_returns_compliant() {
        let i = input(
            "SPUS",
            BusinessActivity::Permissible,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            Some(dec!(0.02)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::Compliant);
        assert!(r.reasons.is_empty(), "compliant verdict carries no reasons");
        assert!(r.purification_ratio.is_none());
    }

    #[test]
    fn debt_ratio_exactly_at_cap_is_compliant() {
        // ADR 0012 §"Screen 2" uses strict `>` per its threshold text.
        let i = input(
            "EDGE",
            BusinessActivity::Permissible,
            Some(dec!(0.33)),
            Some(dec!(0.20)),
            Some(dec!(0.02)),
        );
        assert_eq!(screen(&i).0, Verdict::Compliant);
    }

    #[test]
    fn debt_ratio_just_over_cap_returns_non_compliant_with_reason() {
        let i = input(
            "OVERLEV",
            BusinessActivity::Permissible,
            Some(dec!(0.34)),
            Some(dec!(0.20)),
            Some(dec!(0.02)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::NonCompliant);
        assert!(r
            .reasons
            .iter()
            .any(|s| s.starts_with("debt_ratio_exceeds")));
    }

    #[test]
    fn receivables_ratio_over_cap_returns_non_compliant() {
        let i = input(
            "AR-HEAVY",
            BusinessActivity::Permissible,
            Some(dec!(0.10)),
            Some(dec!(0.50)),
            Some(dec!(0.02)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::NonCompliant);
        assert!(r
            .reasons
            .iter()
            .any(|s| s.starts_with("receivables_ratio_exceeds")));
    }

    #[test]
    fn non_permissible_income_over_cap_returns_non_compliant() {
        let i = input(
            "INTEREST-HEAVY",
            BusinessActivity::Permissible,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            Some(dec!(0.10)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::NonCompliant);
        assert!(r
            .reasons
            .iter()
            .any(|s| s.starts_with("non_permissible_income_exceeds")));
    }

    #[test]
    fn multiple_ratios_exceeded_lists_all_reasons() {
        let i = input(
            "TRIPLE-FAIL",
            BusinessActivity::Permissible,
            Some(dec!(0.50)),
            Some(dec!(0.40)),
            Some(dec!(0.10)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::NonCompliant);
        // Verify all three reason families surface so the badge popover
        // can name each failing check.
        assert!(r.reasons.iter().any(|s| s.starts_with("debt_ratio")));
        assert!(r.reasons.iter().any(|s| s.starts_with("receivables_ratio")));
        assert!(r
            .reasons
            .iter()
            .any(|s| s.starts_with("non_permissible_income")));
    }

    #[test]
    fn mixed_business_with_passing_ratios_returns_mixed_with_purification() {
        let i = input(
            "MIXED-CONGLO",
            BusinessActivity::Mixed,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            Some(dec!(0.03)),
        );
        let (v, r) = screen(&i);
        assert_eq!(v, Verdict::Mixed);
        // Purification ratio = the non-permissible income ratio. This
        // is what Gold-tier dividend purification multiplies through.
        assert_eq!(r.purification_ratio, Some(dec!(0.03)));
    }

    #[test]
    fn mixed_business_with_a_failing_ratio_falls_through_to_non_compliant() {
        let i = input(
            "MIXED-OVER",
            BusinessActivity::Mixed,
            Some(dec!(0.50)),
            Some(dec!(0.20)),
            Some(dec!(0.02)),
        );
        let (v, r) = screen(&i);
        // Ratio cap is checked BEFORE mixed-line branch per the
        // documented step order.
        assert_eq!(v, Verdict::NonCompliant);
        assert!(r
            .reasons
            .iter()
            .any(|s| s.starts_with("debt_ratio_exceeds")));
        assert!(r.purification_ratio.is_none());
    }
}
