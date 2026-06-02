//! Insights engine — golden-output unit tests.
//!
//! Each test feeds a hand-built `InsightsInput` and asserts which
//! rules fire (or stay silent). When adding a rule, add a test in
//! the corresponding section.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::input::{
    DividendEvent, GoalProgress, HoldingDayMove, InsightsInput, NetWorthHistoryPoint,
    SyncFailureInput,
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
