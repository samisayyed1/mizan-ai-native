//! Maliki real-estate intent routing — Track F PR-F2.b / Goal v3 §V Phase 8.
//!
//! Per ADR 0015 (Maliki school Zakat rules, merged in PR #54), property
//! is Zakatable only when held with intent to sell ("urūḍ al-tijāra").
//! The Hanafi/Shafi'i default — that all property is consumer-use and
//! exempt from market-value Zakat — diverges from Maliki here.
//!
//! # Scope
//!
//! This PR ships the **routing helper** that maps (school, intent,
//! value) → bucket. The actual call-site integration into
//! `assess_portfolio` lands as PR-F2.b.1 once the holdings model
//! exposes `metadata.property.intent` through `HoldingsServiceTrait`.
//! Until then, this module is a typed table reviewed in isolation per
//! the autonomous-loop directive: "encode the school-specific edge
//! cases".
//!
//! # Property intent values
//!
//! - `ForSale` — held with intent to sell at market value
//! - `Rental` — held to collect rental income
//! - `Residence` — primary residence or family home
//! - `Unknown` — intent not declared (treated conservatively per school)
//!
//! # Routing table
//!
//! | School    | Residence | Rental                  | ForSale    | Unknown    |
//! |-----------|-----------|-------------------------|------------|------------|
//! | Hanafi    | Exempt    | Exempt (income separate)| Exempt¹    | Exempt     |
//! | Shafi'i   | Exempt    | Exempt                  | Exempt¹    | Exempt     |
//! | Maliki    | Exempt    | Income-only (TBD)²      | Tradable   | Tradable³  |
//! | Hanbali   | Exempt    | Exempt                  | Exempt¹    | Exempt     |
//!
//! ¹ Pre-PR-F2.b baseline: Hanafi/Shafi'i/Hanbali treat all property
//!   as consumer-use. The current `assess_portfolio` reflects this and
//!   stays unchanged. PR-F2.c (Hanbali) adds debt-deduction divergence,
//!   not property-intent divergence.
//!
//! ² Rental-income tracking lands in PR-F2.b.2 once activities are
//!   threaded. For Maliki rentals, the property market value is exempt
//!   and the rental income (a separate cash flow) accrues to liquid_cash
//!   when received. PR-F2.b's routing returns `Exempt` for now and
//!   flags the rental property in the report's `notes` so the user's
//!   imam can apply the rental-income rule manually.
//!
//! ³ Maliki conservative inclusion: when intent is undeclared, route
//!   to tradable (over-statement risk preferred over under-statement
//!   for safety). PR-F2.b.2 surfaces the "set intent in Settings →
//!   Assets" reminder via the report's notes.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::model::School;

/// User-declared intent for a property holding. Read from
/// `metadata.property.intent` on the Holding's instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PropertyIntent {
    /// Held with intent to sell at market value — full market value
    /// counts as tradable under Maliki rules.
    ForSale,
    /// Held to collect rental income — market value exempt; rental
    /// income accrues to liquid_cash separately.
    Rental,
    /// Primary residence / family home — exempt from Zakat in all
    /// four schools.
    Residence,
    /// Intent not declared — conservative routing per school.
    #[default]
    Unknown,
}

impl PropertyIntent {
    /// Parse a free-text intent string. Returns `Unknown` for any
    /// unrecognised value (never panics on bad data).
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().replace([' ', '_'], "-").as_str() {
            "for-sale" | "forsale" | "for-resale" | "tijara" => Self::ForSale,
            "rental" | "rent" | "lease" | "leased" | "income" => Self::Rental,
            "residence" | "home" | "primary" | "primary-residence" | "family"
            | "owner-occupied" => Self::Residence,
            _ => Self::Unknown,
        }
    }
}

/// Which Zakat bucket the property's value feeds into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyBucket {
    /// Exempt from market-value Zakat. The property does NOT contribute
    /// to `tradable_assets`. Rental income (if any) accrues to
    /// `liquid_cash` separately.
    Exempt,
    /// Counted at market value in `tradable_assets`.
    Tradable,
}

/// Route a property's market value into the right Zakat bucket given
/// the user's school + the declared intent. See the module-level
/// docstring for the full routing table.
///
/// Returns the bucket. The caller is responsible for actually summing
/// the values + populating ZakatInputs.
pub fn route_property(school: School, intent: PropertyIntent) -> PropertyBucket {
    match (school, intent) {
        // Residence is exempt in all four schools.
        (_, PropertyIntent::Residence) => PropertyBucket::Exempt,

        // Maliki diverges on ForSale (Tradable) + Unknown (conservative
        // Tradable). Rental for Maliki is Exempt today; PR-F2.b.2 wires
        // the rental-income tracking.
        (School::Maliki, PropertyIntent::ForSale) => PropertyBucket::Tradable,
        (School::Maliki, PropertyIntent::Unknown) => PropertyBucket::Tradable,
        (School::Maliki, PropertyIntent::Rental) => PropertyBucket::Exempt,

        // Hanafi / Shafi'i / Hanbali: property exempt across the board
        // (consumer-use exclusion). PR-F2.c adds Hanbali debt-deduction
        // divergence, NOT property divergence.
        _ => PropertyBucket::Exempt,
    }
}

/// Convenience: sum a list of (intent, value) pairs into a tradable
/// total under the given school. The caller passes this total to
/// ZakatInputs.tradable_assets.
pub fn sum_property_tradable(school: School, items: &[(PropertyIntent, Decimal)]) -> Decimal {
    items
        .iter()
        .filter(|(intent, _)| matches!(route_property(school, *intent), PropertyBucket::Tradable))
        .map(|(_, value)| *value)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_for_sale_aliases() {
        assert_eq!(PropertyIntent::parse("for-sale"), PropertyIntent::ForSale);
        assert_eq!(PropertyIntent::parse("FOR_SALE"), PropertyIntent::ForSale);
        assert_eq!(PropertyIntent::parse("ForSale"), PropertyIntent::ForSale);
        assert_eq!(PropertyIntent::parse(" for sale "), PropertyIntent::ForSale);
        assert_eq!(PropertyIntent::parse("tijara"), PropertyIntent::ForSale);
        assert_eq!(PropertyIntent::parse("for-resale"), PropertyIntent::ForSale);
    }

    #[test]
    fn parse_rental_aliases() {
        assert_eq!(PropertyIntent::parse("rental"), PropertyIntent::Rental);
        assert_eq!(PropertyIntent::parse("RENT"), PropertyIntent::Rental);
        assert_eq!(PropertyIntent::parse("lease"), PropertyIntent::Rental);
        assert_eq!(PropertyIntent::parse("leased"), PropertyIntent::Rental);
        assert_eq!(PropertyIntent::parse("income"), PropertyIntent::Rental);
    }

    #[test]
    fn parse_residence_aliases() {
        assert_eq!(
            PropertyIntent::parse("residence"),
            PropertyIntent::Residence
        );
        assert_eq!(PropertyIntent::parse("home"), PropertyIntent::Residence);
        assert_eq!(PropertyIntent::parse("primary"), PropertyIntent::Residence);
        assert_eq!(
            PropertyIntent::parse("primary-residence"),
            PropertyIntent::Residence
        );
        assert_eq!(PropertyIntent::parse("family"), PropertyIntent::Residence);
        assert_eq!(
            PropertyIntent::parse("owner-occupied"),
            PropertyIntent::Residence
        );
    }

    #[test]
    fn parse_unknown_falls_through() {
        assert_eq!(PropertyIntent::parse(""), PropertyIntent::Unknown);
        assert_eq!(PropertyIntent::parse("vacation"), PropertyIntent::Unknown);
        assert_eq!(PropertyIntent::parse("commercial"), PropertyIntent::Unknown);
    }

    #[test]
    fn default_is_unknown() {
        assert_eq!(PropertyIntent::default(), PropertyIntent::Unknown);
    }

    #[test]
    fn residence_always_exempt() {
        for school in [
            School::Hanafi,
            School::Shafii,
            School::Maliki,
            School::Hanbali,
        ] {
            assert_eq!(
                route_property(school, PropertyIntent::Residence),
                PropertyBucket::Exempt,
                "residence under {school:?} must be exempt"
            );
        }
    }

    #[test]
    fn hanafi_shafii_hanbali_all_property_exempt() {
        for school in [School::Hanafi, School::Shafii, School::Hanbali] {
            for intent in [
                PropertyIntent::ForSale,
                PropertyIntent::Rental,
                PropertyIntent::Residence,
                PropertyIntent::Unknown,
            ] {
                assert_eq!(
                    route_property(school, intent),
                    PropertyBucket::Exempt,
                    "{school:?} + {intent:?} must be exempt (consumer-use exclusion)"
                );
            }
        }
    }

    #[test]
    fn maliki_for_sale_is_tradable() {
        assert_eq!(
            route_property(School::Maliki, PropertyIntent::ForSale),
            PropertyBucket::Tradable
        );
    }

    #[test]
    fn maliki_rental_is_exempt_today() {
        // PR-F2.b.2 will add the rental-income tracking; today, market
        // value is exempt under Maliki rules per ADR 0015.
        assert_eq!(
            route_property(School::Maliki, PropertyIntent::Rental),
            PropertyBucket::Exempt
        );
    }

    #[test]
    fn maliki_unknown_intent_conservative_tradable() {
        // Conservative routing: undeclared intent under Maliki errs on
        // the side of inclusion (Zakat over-statement preferred to
        // under-statement for safety).
        assert_eq!(
            route_property(School::Maliki, PropertyIntent::Unknown),
            PropertyBucket::Tradable
        );
    }

    #[test]
    fn s23_singapore_fixture_under_maliki() {
        // §23 reference user's properties:
        //   - Bukit Batok residence (Residence) → exempt
        //   - 3 Hyderabad rentals (Rental) → exempt under Maliki today
        //     (rental income tracked separately in PR-F2.b.2)
        //   - 1 Hyderabad held-for-sale (ForSale) → tradable at $300K
        let properties = vec![
            (PropertyIntent::Residence, dec!(800_000)),
            (PropertyIntent::Rental, dec!(250_000)),
            (PropertyIntent::Rental, dec!(225_000)),
            (PropertyIntent::Rental, dec!(225_000)),
            (PropertyIntent::ForSale, dec!(300_000)),
        ];
        let tradable_under_maliki = sum_property_tradable(School::Maliki, &properties);
        assert_eq!(
            tradable_under_maliki,
            dec!(300_000),
            "§23 Maliki: only the held-for-sale unit ($300K) flows to tradable"
        );

        // Same fixture under Hanafi: all $1.8M exempt (consumer-use).
        let tradable_under_hanafi = sum_property_tradable(School::Hanafi, &properties);
        assert_eq!(tradable_under_hanafi, Decimal::ZERO);
    }

    #[test]
    fn s23_fixture_with_unknown_intent_under_maliki() {
        // Worst-case Maliki path: user hasn't declared intent on any
        // property. Conservative inclusion: ALL $1.8M flows into
        // tradable. PR-F2.b.2 surfaces a "declare intent" reminder.
        let properties = vec![
            (PropertyIntent::Unknown, dec!(800_000)),
            (PropertyIntent::Unknown, dec!(250_000)),
            (PropertyIntent::Unknown, dec!(225_000)),
            (PropertyIntent::Unknown, dec!(225_000)),
            (PropertyIntent::Unknown, dec!(300_000)),
        ];
        assert_eq!(
            sum_property_tradable(School::Maliki, &properties),
            dec!(1_800_000)
        );
    }

    #[test]
    fn sum_property_tradable_empty_input_zero() {
        assert_eq!(sum_property_tradable(School::Maliki, &[]), Decimal::ZERO);
        assert_eq!(sum_property_tradable(School::Hanafi, &[]), Decimal::ZERO);
    }

    #[test]
    fn sum_property_tradable_skips_exempt_items() {
        let properties = vec![
            (PropertyIntent::Residence, dec!(1_000_000)),
            (PropertyIntent::ForSale, dec!(500_000)),
        ];
        assert_eq!(
            sum_property_tradable(School::Maliki, &properties),
            dec!(500_000)
        );
    }

    #[test]
    fn property_intent_serde_kebab_case() {
        // PropertyIntent uses kebab-case so JSON matches the
        // `metadata.property.intent` shape declared on the holding.
        let json = serde_json::to_string(&PropertyIntent::ForSale).expect("ok");
        assert_eq!(json, "\"for-sale\"");
        let parsed: PropertyIntent = serde_json::from_str("\"residence\"").expect("ok");
        assert_eq!(parsed, PropertyIntent::Residence);
    }
}
