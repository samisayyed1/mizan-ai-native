//! Insights engine — golden-output unit tests.
//!
//! Each test feeds a hand-built `InsightsInput` and asserts which
//! rules fire (or stay silent). When adding a rule, add a test in
//! the corresponding section.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::input::{
    BondMaturityCandidate, DividendEvent, FxPairMove, GoalProgress, HoldingDayMove, InsightsInput,
    NetWorthHistoryPoint, ShariaStatusChange, SyncFailureInput,
};
use super::rules::evaluate;
use mizan_core::notifications::{NotificationKind, NotificationSeverity};

fn base_input() -> InsightsInput {
    InsightsInput {
        today: Some(NaiveDate::from_ymd_opt(2026, 5, 27).unwrap()),
        base_currency: "USD".to_string(),
        holding_moves: vec![],
        goal_progress: vec![],
        net_worth_history: vec![],
        previous_ath: None,
        cash_pct_of_net_worth: None,
        cash_high_for_days: None,
        sync_failures: vec![],
        dividend_events: vec![],
        bond_maturity_candidates: vec![],
        fx_pair_moves: vec![],
        sharia_status_changes: vec![],
    }
}

// ─────────────────────────────────────────────────────────────────────
// BigMove rule
// ─────────────────────────────────────────────────────────────────────

#[test]
fn big_move_fires_on_minus_8pct_drop() {
    let mut i = base_input();
    i.holding_moves.push(HoldingDayMove {
        symbol: "PLTR".into(),
        asset_name: Some("Palantir".into()),
        asset_id: Some("asset_pltr".into()),
        prev_price_base: dec!(100),
        curr_price_base: dec!(92),
        change_pct: dec!(-0.08),
        current_value_base: dec!(5000),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, NotificationKind::BigMove);
    assert_eq!(out[0].severity, NotificationSeverity::Warning);
    assert!(out[0].title.contains("PLTR"), "title was {}", out[0].title);
    assert!(out[0].title.contains("-8"), "title was {}", out[0].title);
    assert!(out[0]
        .deep_link
        .as_deref()
        .unwrap()
        .starts_with("mizan://holding/asset_pltr"));
    assert_eq!(out[0].dedupe_key, "big_move:PLTR:2026-05-27");
}

#[test]
fn big_move_ignored_for_small_position() {
    let mut i = base_input();
    i.holding_moves.push(HoldingDayMove {
        symbol: "TINY".into(),
        asset_name: None,
        asset_id: None,
        prev_price_base: dec!(1),
        curr_price_base: dec!(1.20),
        change_pct: dec!(0.20),
        current_value_base: dec!(14),
    });
    assert!(evaluate(&i).is_empty(), "$14 position should be filtered");
}

#[test]
fn big_move_picks_largest_abs_change() {
    let mut i = base_input();
    i.holding_moves.push(HoldingDayMove {
        symbol: "AAA".into(),
        asset_name: None,
        asset_id: None,
        prev_price_base: dec!(100),
        curr_price_base: dec!(106),
        change_pct: dec!(0.06),
        current_value_base: dec!(10000),
    });
    i.holding_moves.push(HoldingDayMove {
        symbol: "BBB".into(),
        asset_name: None,
        asset_id: None,
        prev_price_base: dec!(100),
        curr_price_base: dec!(90),
        change_pct: dec!(-0.10),
        current_value_base: dec!(8000),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert!(
        out[0].title.contains("BBB"),
        "should have picked the -10% mover"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Goal milestones
// ─────────────────────────────────────────────────────────────────────

#[test]
fn goal_crossing_75_emits_single_milestone() {
    let mut i = base_input();
    i.goal_progress.push(GoalProgress {
        goal_id: "g_house".into(),
        title: "House".into(),
        previous_progress: dec!(0.24),
        current_progress: dec!(0.78),
        current_value_base: dec!(78000),
        target_value_base: Some(dec!(100000)),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, NotificationKind::GoalMilestone);
    assert!(out[0].title.contains("75%"), "title was {}", out[0].title);
}

#[test]
fn goal_at_100_uses_success_severity() {
    let mut i = base_input();
    i.goal_progress.push(GoalProgress {
        goal_id: "g_house".into(),
        title: "House".into(),
        previous_progress: dec!(0.92),
        current_progress: dec!(1.01),
        current_value_base: dec!(101000),
        target_value_base: Some(dec!(100000)),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].severity, NotificationSeverity::Success);
    assert!(
        out[0].title.contains("reached"),
        "title was {}",
        out[0].title
    );
}

#[test]
fn goal_no_new_milestone_silent() {
    let mut i = base_input();
    i.goal_progress.push(GoalProgress {
        goal_id: "g_house".into(),
        title: "House".into(),
        previous_progress: dec!(0.55),
        current_progress: dec!(0.58),
        current_value_base: dec!(58000),
        target_value_base: Some(dec!(100000)),
    });
    assert!(evaluate(&i).is_empty(), "0.55 → 0.58 crosses nothing");
}

// ─────────────────────────────────────────────────────────────────────
// Net worth dip + ATH
// ─────────────────────────────────────────────────────────────────────

#[test]
fn nw_dip_fires_at_minus_5pct_week_over_week() {
    let mut i = base_input();
    let today = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
    for (offset, val) in [(7i64, 100_000i64), (1, 95_000)].iter() {
        i.net_worth_history.push(NetWorthHistoryPoint {
            date: today - chrono::Duration::days(*offset),
            net_worth_base: Decimal::from(*val),
        });
    }
    i.net_worth_history.push(NetWorthHistoryPoint {
        date: today,
        net_worth_base: dec!(94000),
    });
    let out = evaluate(&i);
    assert!(
        out.iter().any(|n| n.kind == NotificationKind::NetWorthDip),
        "expected dip notification"
    );
}

#[test]
fn new_ath_fires_when_above_previous() {
    let mut i = base_input();
    i.previous_ath = Some(dec!(100_000));
    i.net_worth_history.push(NetWorthHistoryPoint {
        date: NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
        net_worth_base: dec!(105_000),
    });
    let out = evaluate(&i);
    let ath = out.iter().find(|n| n.kind == NotificationKind::NewAth);
    assert!(ath.is_some(), "expected ATH notification");
    assert_eq!(ath.unwrap().severity, NotificationSeverity::Success);
}

#[test]
fn ath_silent_when_history_unknown() {
    let mut i = base_input();
    // No previous_ath — engine has nothing to compare against.
    i.net_worth_history.push(NetWorthHistoryPoint {
        date: NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
        net_worth_base: dec!(105_000),
    });
    let out = evaluate(&i);
    assert!(
        !out.iter().any(|n| n.kind == NotificationKind::NewAth),
        "should not fire ATH without previous_ath baseline"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Cash drag
// ─────────────────────────────────────────────────────────────────────

#[test]
fn cash_drag_fires_when_above_threshold_and_days() {
    let mut i = base_input();
    i.cash_pct_of_net_worth = Some(dec!(0.15));
    i.cash_high_for_days = Some(45);
    let out = evaluate(&i);
    assert!(out.iter().any(|n| n.kind == NotificationKind::CashDrag));
}

#[test]
fn cash_drag_silent_under_days_threshold() {
    let mut i = base_input();
    i.cash_pct_of_net_worth = Some(dec!(0.15));
    i.cash_high_for_days = Some(10);
    assert!(
        !evaluate(&i)
            .iter()
            .any(|n| n.kind == NotificationKind::CashDrag),
        "must wait for 30+ days above threshold"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Sync failure
// ─────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────
// DividendPosted
// ─────────────────────────────────────────────────────────────────────

#[test]
fn dividend_event_emits_per_activity() {
    let mut i = base_input();
    i.dividend_events.push(DividendEvent {
        activity_id: "act_abc".into(),
        kind: "DIVIDEND".into(),
        symbol: "PLTR".into(),
        posted_on: NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
        amount_base: dec!(42.50),
    });
    let out = evaluate(&i);
    let div = out
        .iter()
        .find(|n| n.kind == NotificationKind::DividendPosted);
    assert!(div.is_some(), "expected dividend notification");
    let div = div.unwrap();
    assert_eq!(div.severity, NotificationSeverity::Success);
    assert!(div.title.contains("PLTR"));
    assert!(div.dedupe_key == "dividend:act_abc");
}

#[test]
fn interest_event_uses_interest_label() {
    let mut i = base_input();
    i.dividend_events.push(DividendEvent {
        activity_id: "act_int_1".into(),
        kind: "INTEREST".into(),
        symbol: "AAPL Bond".into(),
        posted_on: NaiveDate::from_ymd_opt(2026, 5, 27).unwrap(),
        amount_base: dec!(12.00),
    });
    let out = evaluate(&i);
    let div = out
        .iter()
        .find(|n| n.kind == NotificationKind::DividendPosted);
    assert!(div.is_some());
    assert!(
        div.unwrap().title.starts_with("Interest"),
        "expected Interest prefix"
    );
}

#[test]
fn sync_failure_emits_per_provider() {
    let mut i = base_input();
    i.sync_failures.push(SyncFailureInput {
        provider: "plaid".into(),
        reason: "ITEM_LOGIN_REQUIRED".into(),
        last_success_at_ms: Some(1_700_000_000_000),
    });
    let out = evaluate(&i);
    assert!(out.iter().any(|n| n.kind == NotificationKind::SyncFailure));
}

// ─────────────────────────────────────────────────────────────────────
// Dedupe-key shape
// ─────────────────────────────────────────────────────────────────────

#[test]
fn dedupe_keys_are_deterministic_for_same_input() {
    let mut i = base_input();
    i.holding_moves.push(HoldingDayMove {
        symbol: "PLTR".into(),
        asset_name: None,
        asset_id: None,
        prev_price_base: dec!(100),
        curr_price_base: dec!(92),
        change_pct: dec!(-0.08),
        current_value_base: dec!(5000),
    });
    let a = evaluate(&i);
    let b = evaluate(&i);
    assert_eq!(a[0].dedupe_key, b[0].dedupe_key);
    // ids differ (uuid each time) — that's intentional, the row gets a
    // fresh primary key but the UNIQUE on dedupe_key catches the dupe.
    assert_ne!(a[0].id, b[0].id);
}

// ─────────────────────────────────────────────────────────────────────
// PR-C5.a — BondMaturityApproaching rule
// ─────────────────────────────────────────────────────────────────────

fn emaar_sukuk_47d() -> BondMaturityCandidate {
    BondMaturityCandidate {
        holding_id: "h_emaar_2026".into(),
        symbol: "EMAAR 6.5 2026".into(),
        maturity_date: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
        days_to_maturity: 47,
        principal_returning_base: dec!(188_000),
    }
}

#[test]
fn bond_maturity_fires_at_30day_threshold_for_47day_remaining() {
    // §23 scenario: Emaar Sukuk 47 days to maturity. 47 ≤ 90 — fires
    // at the 90-day step. (90/30/7/1 are *latest-by* thresholds, so a
    // bond with 47 days has crossed the 90 already but not the 30.)
    let mut i = base_input();
    i.bond_maturity_candidates.push(emaar_sukuk_47d());
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, NotificationKind::BondMaturityApproaching);
    assert_eq!(out[0].severity, NotificationSeverity::Info);
    assert!(out[0].title.contains("EMAAR"));
    assert!(out[0].title.contains("47"));
    assert!(
        out[0].body.contains("188000")
            || out[0].body.contains("188,000")
            || out[0].body.contains("188")
    );
}

#[test]
fn bond_maturity_critical_at_one_day() {
    let mut i = base_input();
    let mut c = emaar_sukuk_47d();
    c.days_to_maturity = 1;
    i.bond_maturity_candidates.push(c);
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].severity, NotificationSeverity::Critical);
    assert!(out[0].title.contains("1 day"));
}

#[test]
fn bond_maturity_warning_at_seven_days() {
    let mut i = base_input();
    let mut c = emaar_sukuk_47d();
    c.days_to_maturity = 5;
    i.bond_maturity_candidates.push(c);
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].severity, NotificationSeverity::Warning);
}

#[test]
fn bond_maturity_suppresses_trivial_principal() {
    let mut i = base_input();
    let mut c = emaar_sukuk_47d();
    c.principal_returning_base = dec!(500); // below 1000 floor
    i.bond_maturity_candidates.push(c);
    let out = evaluate(&i);
    assert!(out.is_empty());
}

#[test]
fn bond_maturity_silent_past_horizon() {
    let mut i = base_input();
    let mut c = emaar_sukuk_47d();
    c.days_to_maturity = 365; // beyond all thresholds
    i.bond_maturity_candidates.push(c);
    let out = evaluate(&i);
    assert!(out.is_empty());
}

#[test]
fn bond_maturity_dedupe_key_includes_threshold_step() {
    // The dedupe key must include the threshold step so a holding
    // that crosses 90 → 30 fires both notifications (different keys).
    let mut i = base_input();
    let mut at_90 = emaar_sukuk_47d();
    at_90.days_to_maturity = 60;
    let mut at_30 = emaar_sukuk_47d();
    at_30.days_to_maturity = 20;
    i.bond_maturity_candidates.push(at_90);
    i.bond_maturity_candidates.push(at_30);
    let out = evaluate(&i);
    assert_eq!(out.len(), 2);
    assert_ne!(out[0].dedupe_key, out[1].dedupe_key);
}

// ─────────────────────────────────────────────────────────────────────
// PR-C5.a — FxMovedMaterially rule
// ─────────────────────────────────────────────────────────────────────

#[test]
fn fx_moved_fires_on_4pct_move_with_material_exposure() {
    let mut i = base_input();
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "INR".into(),
        window_days: 30,
        change_pct: dec!(0.04),
        exposure_base: dec!(50_000),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, NotificationKind::FxMovedMaterially);
    assert_eq!(out[0].severity, NotificationSeverity::Info);
    assert!(out[0].title.contains("USD"));
    assert!(out[0].title.contains("INR"));
}

#[test]
fn fx_moved_silent_below_threshold() {
    let mut i = base_input();
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "EUR".into(),
        window_days: 30,
        change_pct: dec!(0.02), // 2% < 3% threshold
        exposure_base: dec!(50_000),
    });
    assert!(evaluate(&i).is_empty());
}

#[test]
fn fx_moved_silent_with_small_exposure() {
    let mut i = base_input();
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "EUR".into(),
        window_days: 30,
        change_pct: dec!(0.05),     // material move
        exposure_base: dec!(1_000), // but tiny exposure
    });
    assert!(evaluate(&i).is_empty());
}

#[test]
fn fx_moved_handles_negative_change_pct() {
    let mut i = base_input();
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "JPY".into(),
        window_days: 30,
        change_pct: dec!(-0.05),
        exposure_base: dec!(20_000),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert!(out[0].body.contains("weakened"));
}

#[test]
fn fx_moved_dedupe_key_per_pair_per_day() {
    let mut i = base_input();
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "INR".into(),
        window_days: 30,
        change_pct: dec!(0.04),
        exposure_base: dec!(50_000),
    });
    let a = evaluate(&i);
    let b = evaluate(&i);
    assert_eq!(a[0].dedupe_key, b[0].dedupe_key);
}

// ─────────────────────────────────────────────────────────────────────
// PR-C5.a — ShariaStatusChanged rule
// ─────────────────────────────────────────────────────────────────────

#[test]
fn sharia_status_change_fires_on_compliant_to_mixed() {
    let mut i = base_input();
    i.sharia_status_changes.push(ShariaStatusChange {
        holding_id: "h_spus".into(),
        symbol: "SPUS".into(),
        prior_verdict: "compliant".into(),
        new_verdict: "mixed".into(),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, NotificationKind::ShariaStatusChanged);
    assert_eq!(out[0].severity, NotificationSeverity::Warning);
    assert!(out[0].title.contains("SPUS"));
    assert!(out[0].title.contains("compliant"));
    assert!(out[0].title.contains("mixed"));
}

#[test]
fn sharia_status_change_silent_on_no_change() {
    // Caller may pass through a same-verdict entry; engine should
    // not fire on it (defence-in-depth against caller bugs).
    let mut i = base_input();
    i.sharia_status_changes.push(ShariaStatusChange {
        holding_id: "h_spus".into(),
        symbol: "SPUS".into(),
        prior_verdict: "compliant".into(),
        new_verdict: "compliant".into(),
    });
    assert!(evaluate(&i).is_empty());
}

#[test]
fn sharia_status_change_dedupe_key_includes_flip_direction() {
    // A flip back-and-forth (compliant → mixed → compliant) must fire
    // twice with distinct dedupe keys, so the user gets credit for
    // both transitions in the audit timeline.
    let mut i = base_input();
    i.sharia_status_changes.push(ShariaStatusChange {
        holding_id: "h_x".into(),
        symbol: "X".into(),
        prior_verdict: "compliant".into(),
        new_verdict: "mixed".into(),
    });
    i.sharia_status_changes.push(ShariaStatusChange {
        holding_id: "h_x".into(),
        symbol: "X".into(),
        prior_verdict: "mixed".into(),
        new_verdict: "compliant".into(),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 2);
    assert_ne!(out[0].dedupe_key, out[1].dedupe_key);
}

// ─────────────────────────────────────────────────────────────────────
// PR-C5.a — Stable rule ordering (regression guard)
// ─────────────────────────────────────────────────────────────────────

#[test]
fn evaluate_emits_new_rules_after_existing_set() {
    // The bell-panel UI renders rules in `evaluate()` order. A future
    // PR that inserts a new rule earlier in the order silently
    // re-arranges the user's notification feed; this test pins the
    // current sequence post-PR-C5.a.
    let mut i = base_input();
    i.holding_moves.push(HoldingDayMove {
        symbol: "AAPL".into(),
        asset_name: None,
        asset_id: None,
        prev_price_base: dec!(100),
        curr_price_base: dec!(92),
        change_pct: dec!(-0.08),
        current_value_base: dec!(5_000),
    });
    i.bond_maturity_candidates.push(emaar_sukuk_47d());
    i.fx_pair_moves.push(FxPairMove {
        from_currency: "USD".into(),
        to_currency: "INR".into(),
        window_days: 30,
        change_pct: dec!(0.04),
        exposure_base: dec!(50_000),
    });
    i.sharia_status_changes.push(ShariaStatusChange {
        holding_id: "h_spus".into(),
        symbol: "SPUS".into(),
        prior_verdict: "compliant".into(),
        new_verdict: "mixed".into(),
    });
    let out = evaluate(&i);
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].kind, NotificationKind::BigMove);
    assert_eq!(out[1].kind, NotificationKind::BondMaturityApproaching);
    assert_eq!(out[2].kind, NotificationKind::FxMovedMaterially);
    assert_eq!(out[3].kind, NotificationKind::ShariaStatusChanged);
}
