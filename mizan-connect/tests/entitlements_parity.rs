//! Entitlements parity guard.
//!
//! The cloud's `mizan_connect::billing::entitlements::entitlements_for`
//! and the desktop's `mizan_connect::entitlements::entitlements_for`
//! ship the SAME matrix to the same users — the desktop reads the
//! cloud's `/v1/me` response into its `Entitlements` struct verbatim.
//!
//! Any drift between the two = users see desktop capabilities that
//! don't match their billing tier (security + correctness CRITICAL).
//!
//! This test pins the matrix as a frozen JSON snapshot. The SAME
//! snapshot bytes live in `mizan-connect/tests/entitlements_parity.rs`.
//! Either side changing the matrix without updating both fails its
//! own CI. Anyone touching one matrix is forced to confront the
//! other.
//!
//! When the matrix legitimately needs to change (e.g. new tier,
//! new capability), regenerate the fixture in BOTH repos:
//!   1. Update the matrix in both projects.
//!   2. Run `cargo test --test entitlements_parity -- --nocapture` —
//!      the diff output prints the new JSON.
//!   3. Paste it into `EXPECTED` in both this file and the cloud
//!      mirror.

use mizan_connect::billing::entitlements_for;
use serde_json::{json, Value};

/// All (tier, status) combinations we ship matrices for.
fn fixtures() -> Vec<(Option<&'static str>, Option<&'static str>, &'static str)> {
    vec![
        // tier, status, label
        (None, None, "anonymous"),
        (Some("free"), Some("active"), "free/active"),
        (Some("free"), None, "free/none"),
        (Some("silver"), Some("active"), "silver/active"),
        (Some("silver"), Some("trialing"), "silver/trialing"),
        (Some("silver"), Some("past_due"), "silver/past_due"),
        (Some("silver"), Some("canceled"), "silver/canceled"),
        (Some("basic"), Some("active"), "basic/active (legacy → silver)"),
        (Some("gold"), Some("active"), "gold/active"),
        (Some("gold"), Some("trialing"), "gold/trialing"),
        (Some("gold"), Some("past_due"), "gold/past_due"),
        (Some("gold"), Some("canceled"), "gold/canceled"),
        (Some("enterprise"), Some("active"), "enterprise/active"),
        (Some("advisor"), Some("active"), "advisor/active"),
        (Some("pro"), Some("active"), "pro/active (legacy → gold)"),
        (Some("plus"), Some("active"), "plus/active (legacy → gold)"),
        (Some("essentials"), Some("active"), "essentials/active (legacy → gold)"),
        (Some("duo"), Some("active"), "duo/active (legacy → gold)"),
    ]
}

/// Frozen expected matrix. Identical bytes live in the cloud test.
fn expected() -> Value {
    json!({
        "anonymous": free(),
        "free/active": free(),
        "free/none": free(),
        "silver/active": silver(),
        "silver/trialing": silver(),
        "silver/past_due": free(),
        "silver/canceled": free(),
        "basic/active (legacy → silver)": silver(),
        "gold/active": gold(),
        "gold/trialing": gold(),
        "gold/past_due": free(),
        "gold/canceled": free(),
        "enterprise/active": enterprise(),
        "advisor/active": advisor(),
        "pro/active (legacy → gold)": legacy_gold(),
        "plus/active (legacy → gold)": legacy_gold(),
        "essentials/active (legacy → gold)": legacy_gold(),
        "duo/active (legacy → gold)": legacy_gold(),
    })
}

fn free() -> Value {
    json!({
        "plan": "free",
        "maxPortfolios": 1,
        "maxHoldings": 20,
        "maxAssetClasses": -1,
        "brokerSync": false,
        "maxBrokerConnections": 0,
        "deviceSync": false,
        "cloudBackup": false,
        "managedAi": false,
        "aiCreditsMonthly": 0,
        "newsDailyLimit": 3,
        "marketRefreshDailyLimit": 0,
        "csvImportsMonthly": 0,
        "advancedReports": false,
        "zakatEngine": false,
        "advisorMode": false,
    })
}

fn silver() -> Value {
    json!({
        "plan": "silver",
        "maxPortfolios": 25,
        "maxHoldings": -1,
        "maxAssetClasses": -1,
        "brokerSync": false,
        "maxBrokerConnections": 0,
        "deviceSync": false,
        "cloudBackup": false,
        "managedAi": true,
        "aiCreditsMonthly": 300,
        "newsDailyLimit": -1,
        "marketRefreshDailyLimit": 0,
        "csvImportsMonthly": -1,
        "advancedReports": false,
        "zakatEngine": false,
        "advisorMode": false,
    })
}

fn gold() -> Value {
    json!({
        "plan": "gold",
        "maxPortfolios": -1,
        "maxHoldings": -1,
        "maxAssetClasses": -1,
        "brokerSync": true,
        "maxBrokerConnections": -1,
        "deviceSync": true,
        "cloudBackup": true,
        "managedAi": true,
        "aiCreditsMonthly": -1,
        "newsDailyLimit": -1,
        "marketRefreshDailyLimit": -1,
        "csvImportsMonthly": -1,
        "advancedReports": true,
        "zakatEngine": true,
        "advisorMode": false,
    })
}

fn enterprise() -> Value {
    let mut v = gold();
    v["advisorMode"] = json!(true);
    v
}

fn advisor() -> Value {
    let mut v = gold();
    v["advisorMode"] = json!(true);
    v
}

fn legacy_gold() -> Value {
    // pro/plus/duo/essentials are legacy paid slugs that map to the
    // Gold capability bundle EXCEPT with the older 25/5/1500 caps
    // — they're grandfathered into a slightly tighter tier so users
    // who paid for "pro" don't suddenly get unlimited everything.
    json!({
        "plan": "gold",
        "maxPortfolios": 25,
        "maxHoldings": -1,
        "maxAssetClasses": -1,
        "brokerSync": true,
        "maxBrokerConnections": 5,
        "deviceSync": true,
        "cloudBackup": true,
        "managedAi": true,
        "aiCreditsMonthly": 1500,
        "newsDailyLimit": -1,
        "marketRefreshDailyLimit": -1,
        "csvImportsMonthly": -1,
        "advancedReports": true,
        "zakatEngine": true,
        "advisorMode": false,
    })
}

#[test]
fn entitlements_matrix_matches_canonical_snapshot() {
    let exp = expected();
    let mut actual = serde_json::Map::new();
    for (tier, status, label) in fixtures() {
        let e = entitlements_for(tier, status);
        actual.insert(label.to_string(), serde_json::to_value(&e).unwrap());
    }
    let actual_value = Value::Object(actual);

    if actual_value != exp {
        // Print the diff in a copy-paste-friendly way so the dev
        // updating the matrix can lift the new JSON into the
        // EXPECTED block without re-typing.
        eprintln!("--- expected ---");
        eprintln!("{}", serde_json::to_string_pretty(&exp).unwrap());
        eprintln!("--- actual ---");
        eprintln!("{}", serde_json::to_string_pretty(&actual_value).unwrap());
        panic!(
            "entitlements matrix drifted from the canonical snapshot. \
             Update BOTH mizan-4/crates/connect/tests/entitlements_parity.rs \
             AND mizan-connect/tests/entitlements_parity.rs in lock-step."
        );
    }
}
