//! Integration test against the actual Uncle Feroz seed.
//!
//! The fixture is a real production-shape portfolio dropped into
//! `tests/fixtures/` so the demo flow has a reproducible regression
//! suite — if the upstream JSON shape changes, this test catches it
//! at the parser layer before the desktop importer sees it.

use mizan_portfolio_seed_import::operations::{summarise, OperationKind};
use mizan_portfolio_seed_import::{parse_seed, plan, Operation};

const SEED: &str = include_str!("fixtures/uncle_feroz_2026_04_04.json");

#[test]
fn parses_full_uncle_feroz_seed_without_error() {
    let seed = parse_seed(SEED).expect("seed should parse");
    assert_eq!(seed.owner.name, "Feroz Siddiqui");
    assert_eq!(seed.owner.base_currency, "USD");
    assert_eq!(seed.owner.school, "Hanafi");
    assert_eq!(seed.owner.as_of, "2026-04-04");
    assert_eq!(seed.summary_target_total_usd, Some(1_713_968.7));
}

#[test]
fn plan_emits_expected_operation_counts_for_uncle_feroz() {
    let seed = parse_seed(SEED).unwrap();
    let ops = plan(&seed).unwrap();
    let counts = summarise(&ops);

    // The seed's actual content — these numbers come straight from
    // the JSON, and the test breaks loudly if the planner silently
    // drops or duplicates a row.
    assert_eq!(counts.get(&OperationKind::SetOwnerProfile), Some(&1));
    assert_eq!(counts.get(&OperationKind::SetFxSnapshot), Some(&1));
    assert_eq!(counts.get(&OperationKind::CreateSukuk), Some(&4));
    assert_eq!(counts.get(&OperationKind::CreateEquitySubset), Some(&2));
    assert_eq!(counts.get(&OperationKind::CreateEtf), Some(&4));
    assert_eq!(counts.get(&OperationKind::CreatePrivateEquity), Some(&4));
    // Tiger UT (1 holding) + Tiger cash side (1 USD account)
    assert_eq!(counts.get(&OperationKind::CreateUnitTrust), Some(&1));
    assert_eq!(counts.get(&OperationKind::CreateUnitTrustTranche), Some(&4));
    assert_eq!(
        counts.get(&OperationKind::CreateUnitTrustPlaceholder),
        Some(&5)
    );
    assert_eq!(counts.get(&OperationKind::CreateUlipPolicy), Some(&4));
    assert_eq!(
        counts.get(&OperationKind::CreateProvidentFundBalance),
        Some(&1)
    );
    // 12 user-listed cash accounts + 1 emitted for Tiger's USD cash
    // sleeve.
    assert_eq!(counts.get(&OperationKind::CreateCashAccount), Some(&13));
    assert_eq!(
        counts.get(&OperationKind::CreateRealEstateProperty),
        Some(&6)
    );
}

#[test]
fn fx_rates_carry_all_four_pairs_from_seed() {
    let seed = parse_seed(SEED).unwrap();
    let snap = mizan_portfolio_seed_import::fx_snapshot(&seed)
        .unwrap()
        .unwrap();
    assert_eq!(snap.as_of, "2026-04-04");
    for pair in ["USD_SGD", "USD_INR", "USD_SAR", "USD_AED"] {
        assert!(
            snap.rates.contains_key(pair),
            "missing FX pair {pair} in snapshot"
        );
    }
}

#[test]
fn owner_profile_is_first_operation_so_consumer_can_anchor_other_writes() {
    let seed = parse_seed(SEED).unwrap();
    let ops = plan(&seed).unwrap();
    assert!(matches!(ops[0], Operation::SetOwnerProfile { .. }));
    assert!(matches!(ops[1], Operation::SetFxSnapshot(_)));
}

#[test]
fn sukuk_isins_match_seed_exactly() {
    let seed = parse_seed(SEED).unwrap();
    let ops = plan(&seed).unwrap();
    let isins: Vec<&str> = ops
        .iter()
        .filter_map(|o| match o {
            Operation::CreateSukuk { isin, .. } => Some(isin.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        isins,
        vec![
            "XS2052469165",
            "XS2124942595",
            "XS2633136234",
            "XS2753304349"
        ]
    );
}

#[test]
fn aparna_indian_properties_have_no_valuation_to_force_user_input() {
    // The seed deliberately leaves the Indian rental properties'
    // `current_value_usd: null`. The planner must NOT silently
    // backfill them; the dashboard renders an "Add valuation"
    // affordance instead.
    let seed = parse_seed(SEED).unwrap();
    let ops = plan(&seed).unwrap();
    let unvalued_indian: Vec<&str> = ops
        .iter()
        .filter_map(|o| match o {
            Operation::CreateRealEstateProperty {
                country,
                name,
                current_value_usd: None,
                ..
            } if country == "India" => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(unvalued_indian.len(), 4, "expected 4 unvalued Aparna units");
}

#[test]
fn cash_accounts_span_four_countries() {
    let seed = parse_seed(SEED).unwrap();
    let ops = plan(&seed).unwrap();
    let countries: std::collections::BTreeSet<&str> = ops
        .iter()
        .filter_map(|o| match o {
            Operation::CreateCashAccount { country, .. } => Some(country.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(countries.len(), 4);
    for c in ["Singapore", "India", "KSA", "UAE"] {
        assert!(countries.contains(c), "missing cash country {c}");
    }
}
