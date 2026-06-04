//! Hanbali debt-deduction + locked-retirement routing — Track F PR-F2.c.
//!
//! Per ADR 0016 (Hanbali school Zakat rules, merged in PR #54), the
//! Hanbali school diverges from Hanafi/Shafi'i/Maliki on two surfaces:
//!
//! 1. **Debt deduction is broader** — long-term mortgage principal
//!    qualifies under the "delayed-debt doctrine" (`tāʾjīl al-dayn`).
//!    Hanafi only counts short-term debts (due within the lunar
//!    year); Hanbali counts the full outstanding principal of
//!    long-term secured debts the user is committed to paying.
//!
//! 2. **Locked retirement is partially Zakatable** — the proportionate
//!    annual share of locked retirement balances accrues to tradable.
//!    Hanafi/Shafi'i/Maliki treat locked retirement (CPF SA/RA, 401k
//!    pre-59½, NPS Tier 1, locked Superannuation) as inaccessible and
//!    exempt until withdrawn. Hanbali apportions: `balance × (1 / years_to_unlock)`
//!    enters tradable each year.
//!
//! # Scope
//!
//! This PR ships the **routing helpers**. Call-site integration into
//! `assess_portfolio` lands as PR-F2.c.1 once liabilities + provident-
//! fund metadata are threaded through HoldingsServiceTrait. The
//! routing tables here are reviewed in isolation per the autonomous-
//! loop directive.
//!
//! # Conservative routing
//!
//! When `years_to_unlock` is unknown for a locked retirement holding,
//! we fall back to 1.0 — meaning the full balance enters tradable
//! that year. This is the conservative Hanbali inclusion (Zakat
//! over-statement preferred to under-statement). The user can set
//! a more accurate years_to_unlock in Settings → Assets → Retirement
//! to refine the apportionment.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::model::School;

/// Classification for a debt liability. Read from
/// `metadata.liability.kind` on a Holding's instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DebtKind {
    /// Falls due within the current lunar year (credit card balance,
    /// short-term personal loan, current portion of long-term debts).
    /// Deductible under ALL four schools.
    #[default]
    ShortTerm,
    /// Outstanding principal on a long-term secured mortgage. Hanbali
    /// allows full principal in `deductible_debts` per ADR 0016; the
    /// other three schools count only the within-year portion (already
    /// captured under `ShortTerm`).
    LongTermMortgage,
    /// Other long-term debt (student loan, business term loan, etc.).
    /// Not deductible under any school's current rules — the borrower
    /// should account for it as a personal obligation, not a Zakat
    /// deduction. Reserved here so callers don't lose the type info.
    Other,
}

impl DebtKind {
    /// Parse a free-text debt-kind string. Returns ShortTerm for any
    /// unrecognised value (the most permissive deduction — conservative
    /// at the OPPOSITE end from PropertyIntent::Unknown: there we
    /// over-state Zakat, here we'd over-deduct. Defaulting to ShortTerm
    /// matches what most callers will mean by "I owe this and need
    /// to pay it soon".)
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().replace([' ', '_'], "-").as_str() {
            "long-term-mortgage" | "mortgage" | "home-loan" | "housing-loan" => {
                Self::LongTermMortgage
            }
            "other" | "long-term" | "student-loan" | "business-loan" | "term-loan" => Self::Other,
            _ => Self::ShortTerm,
        }
    }
}

/// Classification for a retirement-account balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RetirementKind {
    /// User can withdraw the balance today without penalty. Counts in
    /// tradable_assets at full value under all four schools.
    #[default]
    Accessible,
    /// Locked until a future date (CPF SA/RA, 401k pre-59½, NPS Tier 1).
    /// Hanafi/Shafi'i/Maliki treat as inaccessible (exempt from
    /// tradable_assets until withdrawn). Hanbali counts the
    /// proportionate annual share — `balance / years_to_unlock`.
    Locked,
}

/// Compute how much of a long-term mortgage principal enters the
/// `deductible_debts` bucket under the given school.
///
/// - **Hanbali**: full principal (delayed-debt doctrine).
/// - **Hanafi / Shafi'i / Maliki**: zero (only within-year portion
///   counts, which the caller already passed in via `short_term_debts`).
pub fn long_term_mortgage_deductible(school: School, principal: Decimal) -> Decimal {
    match school {
        School::Hanbali => principal,
        _ => Decimal::ZERO,
    }
}

/// Compute the tradable contribution of a locked retirement balance
/// under the given school. `years_to_unlock` is the time horizon
/// until the funds become accessible (e.g. CPF SA: years until 65 –
/// current_age). Defaults sensibly when zero/negative.
///
/// - **Hanbali**: `balance / max(years_to_unlock, 1)` per ADR 0016
///   §"locked retirement proportionate annual share".
/// - **Hanafi / Shafi'i / Maliki**: zero (locked = inaccessible =
///   exempt until withdrawn).
pub fn locked_retirement_tradable(
    school: School,
    balance: Decimal,
    years_to_unlock: Decimal,
) -> Decimal {
    if !matches!(school, School::Hanbali) {
        return Decimal::ZERO;
    }
    // Conservative: when years_to_unlock is unknown or non-positive,
    // treat as 1 year remaining (max apportionment).
    let years = if years_to_unlock > dec!(1) {
        years_to_unlock
    } else {
        dec!(1)
    };
    balance / years
}

/// Compute the tradable contribution of an *accessible* retirement
/// balance. Same answer under every school: full balance counts in
/// tradable_assets.
pub fn accessible_retirement_tradable(balance: Decimal) -> Decimal {
    balance
}

/// Convenience: sum a slice of (DebtKind, principal) into the school-
/// aware deductible_debts contribution from long-term mortgages.
/// The caller adds this on top of the short-term debt total they
/// already aggregated.
pub fn sum_long_term_mortgage_deductible(school: School, items: &[(DebtKind, Decimal)]) -> Decimal {
    items
        .iter()
        .filter_map(|(kind, principal)| match kind {
            DebtKind::LongTermMortgage => Some(long_term_mortgage_deductible(school, *principal)),
            _ => None,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── DebtKind ──────────────────────────────────────────────

    #[test]
    fn debt_kind_default_is_short_term() {
        assert_eq!(DebtKind::default(), DebtKind::ShortTerm);
    }

    #[test]
    fn debt_kind_parses_aliases() {
        assert_eq!(
            DebtKind::parse("long-term-mortgage"),
            DebtKind::LongTermMortgage
        );
        assert_eq!(DebtKind::parse("MORTGAGE"), DebtKind::LongTermMortgage);
        assert_eq!(DebtKind::parse("home loan"), DebtKind::LongTermMortgage);
        assert_eq!(DebtKind::parse("housing_loan"), DebtKind::LongTermMortgage);
        assert_eq!(DebtKind::parse("student-loan"), DebtKind::Other);
        assert_eq!(DebtKind::parse("business-loan"), DebtKind::Other);
        assert_eq!(DebtKind::parse("term-loan"), DebtKind::Other);
        assert_eq!(DebtKind::parse("long-term"), DebtKind::Other);
        // Unrecognised → ShortTerm (conservative: most callers mean
        // "I owe this and intend to pay it soon")
        assert_eq!(DebtKind::parse("credit-card"), DebtKind::ShortTerm);
        assert_eq!(DebtKind::parse(""), DebtKind::ShortTerm);
        assert_eq!(DebtKind::parse("personal-loan"), DebtKind::ShortTerm);
    }

    #[test]
    fn debt_kind_serde_kebab_case() {
        let json = serde_json::to_string(&DebtKind::LongTermMortgage).expect("ok");
        assert_eq!(json, "\"long-term-mortgage\"");
        let parsed: DebtKind = serde_json::from_str("\"short-term\"").expect("ok");
        assert_eq!(parsed, DebtKind::ShortTerm);
    }

    // ─── long_term_mortgage_deductible ─────────────────────────

    #[test]
    fn long_term_mortgage_hanbali_takes_full_principal() {
        assert_eq!(
            long_term_mortgage_deductible(School::Hanbali, dec!(450_000)),
            dec!(450_000)
        );
    }

    #[test]
    fn long_term_mortgage_other_schools_take_zero() {
        for school in [School::Hanafi, School::Shafii, School::Maliki] {
            assert_eq!(
                long_term_mortgage_deductible(school, dec!(450_000)),
                Decimal::ZERO,
                "{school:?} must not deduct long-term mortgage principal"
            );
        }
    }

    #[test]
    fn long_term_mortgage_zero_principal_zero_deduction() {
        assert_eq!(
            long_term_mortgage_deductible(School::Hanbali, Decimal::ZERO),
            Decimal::ZERO
        );
    }

    // ─── locked_retirement_tradable ────────────────────────────

    #[test]
    fn locked_retirement_hanbali_apportions_by_years() {
        // CPF SA $100K with 10 years to unlock → $10K in tradable
        assert_eq!(
            locked_retirement_tradable(School::Hanbali, dec!(100_000), dec!(10)),
            dec!(10_000)
        );
    }

    #[test]
    fn locked_retirement_hanbali_one_year_full_balance() {
        // Within-one-year unlock window → conservatively counts as
        // accessible (full balance enters tradable)
        assert_eq!(
            locked_retirement_tradable(School::Hanbali, dec!(50_000), dec!(1)),
            dec!(50_000)
        );
    }

    #[test]
    fn locked_retirement_hanbali_unknown_years_full_balance() {
        // years_to_unlock = 0 (unknown) → conservative inclusion
        assert_eq!(
            locked_retirement_tradable(School::Hanbali, dec!(50_000), Decimal::ZERO),
            dec!(50_000)
        );
        // Negative years (data entry error) also defaults to 1
        assert_eq!(
            locked_retirement_tradable(School::Hanbali, dec!(50_000), dec!(-5)),
            dec!(50_000)
        );
    }

    #[test]
    fn locked_retirement_other_schools_zero() {
        for school in [School::Hanafi, School::Shafii, School::Maliki] {
            assert_eq!(
                locked_retirement_tradable(school, dec!(100_000), dec!(10)),
                Decimal::ZERO,
                "{school:?} treats locked retirement as inaccessible"
            );
        }
    }

    // ─── accessible_retirement_tradable ────────────────────────

    #[test]
    fn accessible_retirement_passes_through() {
        assert_eq!(accessible_retirement_tradable(dec!(75_000)), dec!(75_000));
        assert_eq!(accessible_retirement_tradable(Decimal::ZERO), Decimal::ZERO);
    }

    // ─── sum_long_term_mortgage_deductible ─────────────────────

    #[test]
    fn s23_fixture_singapore_mortgage_under_hanbali() {
        // §23 reference user: Bukit Batok HDB mortgage ($450K outstanding) +
        // Hyderabad rentals mortgage ($120K) + credit card balance ($3K).
        // Only the long-term mortgages count under Hanbali's delayed-debt
        // doctrine. The credit card is ShortTerm — already in the
        // user's short_term_debts bucket, so route_property doesn't
        // double-count it.
        let debts = vec![
            (DebtKind::LongTermMortgage, dec!(450_000)),
            (DebtKind::LongTermMortgage, dec!(120_000)),
            (DebtKind::ShortTerm, dec!(3_000)),
            (DebtKind::Other, dec!(15_000)), // student loan — not deductible under any school
        ];
        assert_eq!(
            sum_long_term_mortgage_deductible(School::Hanbali, &debts),
            dec!(570_000)
        );
        // Under the other three schools the long-term mortgage
        // contribution is zero.
        for school in [School::Hanafi, School::Shafii, School::Maliki] {
            assert_eq!(
                sum_long_term_mortgage_deductible(school, &debts),
                Decimal::ZERO
            );
        }
    }

    #[test]
    fn sum_long_term_mortgage_empty_input_zero() {
        assert_eq!(
            sum_long_term_mortgage_deductible(School::Hanbali, &[]),
            Decimal::ZERO
        );
    }

    #[test]
    fn sum_long_term_mortgage_skips_non_mortgage_debts() {
        let debts = vec![
            (DebtKind::ShortTerm, dec!(5_000)),
            (DebtKind::Other, dec!(20_000)),
            (DebtKind::LongTermMortgage, dec!(300_000)),
        ];
        assert_eq!(
            sum_long_term_mortgage_deductible(School::Hanbali, &debts),
            dec!(300_000)
        );
    }

    // ─── RetirementKind ────────────────────────────────────────

    #[test]
    fn retirement_kind_default_is_accessible() {
        assert_eq!(RetirementKind::default(), RetirementKind::Accessible);
    }

    #[test]
    fn retirement_kind_serde_kebab_case() {
        let json = serde_json::to_string(&RetirementKind::Locked).expect("ok");
        assert_eq!(json, "\"locked\"");
        let parsed: RetirementKind = serde_json::from_str("\"accessible\"").expect("ok");
        assert_eq!(parsed, RetirementKind::Accessible);
    }
}
