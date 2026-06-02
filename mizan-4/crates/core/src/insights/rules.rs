//! The deterministic rule set.
//!
//! Each `eval_*` function returns `Option<Notification>` (or `Vec` for
//! multi-emit rules like goal milestones). `evaluate()` collects them in
//! a stable order so the bell-panel UI doesn't shuffle between ticks.
//!
//! Dedupe-key format: `<rule>:<scope>:<date>` — see migration header for
//! why this matters. The scheduler relies on the UNIQUE index for
//! idempotency, but the engine produces the keys so the storage layer
//! never has to know about insight semantics.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;

use super::input::{DividendEvent, GoalProgress, InsightsInput, SyncFailureInput};
use crate::notifications::{Notification, NotificationKind, NotificationSeverity};

/// Threshold for the BigMove rule: any holding moving > this in absolute
/// daily change emits. 5% picks up real movers without spamming the user
/// on a normal market day (where everything moves ±1%).
const BIG_MOVE_THRESHOLD_PCT: f64 = 0.05;

/// Minimum position size to qualify for BigMove. Stops "you're up 12%!"
/// on a $14 holding. 250 base-currency units is the smallest position
/// most users would care to act on.
const BIG_MOVE_MIN_VALUE_BASE: f64 = 250.0;

/// Milestones in the goal-progress space. The engine emits at most one
/// per goal per evaluation — the highest milestone the goal has newly
/// crossed since `previous_progress`. So a goal jumping from 24% → 78%
/// fires a single "75% reached" notification, not three.
const GOAL_MILESTONES: &[(f64, &str)] = &[
    (0.25, "25%"),
    (0.50, "50%"),
    (0.75, "75%"),
    (0.90, "90%"),
    (1.00, "100%"),
];

/// Net-worth-dip rule fires when this-week vs last-week drops by more
/// than this fraction.
const NW_DIP_THRESHOLD_PCT: f64 = 0.03;

/// Cash-drag rule: cash > 10% of net worth for 30+ days emits a
/// "consider deploying" warning.
const CASH_DRAG_PCT_THRESHOLD: f64 = 0.10;
const CASH_DRAG_DAYS_THRESHOLD: u32 = 30;

fn dec_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

fn today_str(input: &InsightsInput) -> String {
    input
        .today
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "no-date".to_string())
}

fn fresh(
    kind: NotificationKind,
    severity: NotificationSeverity,
    title: String,
    body: String,
    deep_link: Option<String>,
    payload_json: String,
    dedupe_key: String,
) -> Notification {
    Notification {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        severity,
        title,
        body,
        deep_link,
        payload_json,
        dedupe_key,
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        read_at_ms: None,
        dismissed_at_ms: None,
    }
}

/// Format a percentage with a leading sign for the +/- BigMove copy.
fn fmt_pct_signed(p: Decimal) -> String {
    let f = dec_to_f64(p);
    let pct = f * 100.0;
    if pct >= 0.0 {
        format!("+{pct:.1}%")
    } else {
        format!("{pct:.1}%")
    }
}

fn eval_big_move(input: &InsightsInput) -> Option<Notification> {
    // Find the holding with the largest |change_pct| above threshold &
    // minimum position size. We emit one notification per day for the
    // single biggest mover — flooding the user with five-mover
    // notifications dilutes signal.
    let mover = input
        .holding_moves
        .iter()
        .filter(|h| dec_to_f64(h.change_pct).abs() >= BIG_MOVE_THRESHOLD_PCT)
        .filter(|h| dec_to_f64(h.current_value_base) >= BIG_MOVE_MIN_VALUE_BASE)
        .max_by(|a, b| {
            dec_to_f64(a.change_pct)
                .abs()
                .partial_cmp(&dec_to_f64(b.change_pct).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;

    let dir_word = if dec_to_f64(mover.change_pct) >= 0.0 {
        "up"
    } else {
        "down"
    };
    let pct = fmt_pct_signed(mover.change_pct);
    let title = format!("{} {} today", mover.symbol, pct);
    let body = format!(
        "Your {} position moved {} {} today, now worth {} {:.0}.",
        mover.symbol,
        dir_word,
        pct,
        input.base_currency,
        dec_to_f64(mover.current_value_base),
    );
    let payload = json!({
        "symbol": mover.symbol,
        "assetName": mover.asset_name,
        "changePct": mover.change_pct,
        "currentValueBase": mover.current_value_base,
        "prevPriceBase": mover.prev_price_base,
        "currPriceBase": mover.curr_price_base,
    });
    let deep_link = mover
        .asset_id
        .as_ref()
        .map(|aid| format!("mizan://holding/{aid}"));
    let dedupe = format!("big_move:{}:{}", mover.symbol, today_str(input));

    // Severity: big-down is Warning (user might want to act); big-up is
    // Info (positive news doesn't warrant an OS push, just an in-app row).
    let severity = if dec_to_f64(mover.change_pct) < 0.0 {
        NotificationSeverity::Warning
    } else {
        NotificationSeverity::Success
    };
    Some(fresh(
        NotificationKind::BigMove,
        severity,
        title,
        body,
        deep_link,
        payload.to_string(),
        dedupe,
    ))
}

fn eval_goal_milestones(input: &InsightsInput) -> Vec<Notification> {
    let mut out = Vec::new();
    for g in &input.goal_progress {
        if let Some(n) = eval_single_goal(g, input) {
            out.push(n);
        }
    }
    out
}

fn eval_single_goal(g: &GoalProgress, input: &InsightsInput) -> Option<Notification> {
    let prev = dec_to_f64(g.previous_progress);
    let curr = dec_to_f64(g.current_progress);
    if curr <= prev {
        return None;
    }
    // Highest milestone newly crossed.
    let crossed = GOAL_MILESTONES
        .iter()
        .rev()
        .find(|(threshold, _)| curr >= *threshold && prev < *threshold);
    let (threshold, label) = crossed?;
    let title = if *threshold >= 1.0 {
        format!("{} — goal reached!", g.title)
    } else {
        format!("{} — {} of target", g.title, label)
    };
    let body = if *threshold >= 1.0 {
        format!(
            "You hit your target for {}. Time to plan what's next.",
            g.title
        )
    } else {
        format!(
            "You're now at {} of your {} target. Keep going.",
            label, g.title
        )
    };
    let severity = if *threshold >= 1.0 {
        NotificationSeverity::Success
    } else {
        NotificationSeverity::Info
    };
    let payload = json!({
        "goalId": g.goal_id,
        "title": g.title,
        "previousProgress": g.previous_progress,
        "currentProgress": g.current_progress,
        "currentValueBase": g.current_value_base,
        "targetValueBase": g.target_value_base,
        "milestone": label,
    });
    let dedupe = format!(
        "goal_milestone:{}:{}:{}",
        g.goal_id,
        label,
        today_str(input)
    );
    Some(fresh(
        NotificationKind::GoalMilestone,
        severity,
        title,
        body,
        Some(format!("mizan://goal/{}", g.goal_id)),
        payload.to_string(),
        dedupe,
    ))
}

fn eval_net_worth_dip_or_ath(input: &InsightsInput) -> Vec<Notification> {
    let mut out = Vec::new();
    let last = input.net_worth_history.last();
    let Some(last) = last else { return out };

    // ATH — only fires if we know the prior ATH (no false alarm on
    // first-ever snapshot).
    if let Some(prev_ath) = input.previous_ath {
        if last.net_worth_base > prev_ath {
            let payload = json!({
                "netWorthBase": last.net_worth_base,
                "previousAth": prev_ath,
                "date": last.date.to_string(),
            });
            out.push(fresh(
                NotificationKind::NewAth,
                NotificationSeverity::Success,
                "New net-worth high".to_string(),
                format!(
                    "Your net worth just hit a fresh high at {} {:.0}.",
                    input.base_currency,
                    dec_to_f64(last.net_worth_base)
                ),
                Some("mizan://dashboard".to_string()),
                payload.to_string(),
                format!("new_ath:{}", today_str(input)),
            ));
        }
    }

    // Week-over-week dip. Find the most-recent point that's ≥ 7 days
    // older than today. Iterating in reverse (newest → oldest) and
    // taking the first that matches gives us the closest "a week ago"
    // anchor when the history is sparse.
    let week_ago = input
        .net_worth_history
        .iter()
        .rev()
        .find(|p| (last.date - p.date).num_days() >= 7);
    if let Some(week_ago) = week_ago {
        let prev = dec_to_f64(week_ago.net_worth_base);
        let curr = dec_to_f64(last.net_worth_base);
        if prev > 0.0 {
            let change = (curr - prev) / prev;
            if change <= -NW_DIP_THRESHOLD_PCT {
                let payload = json!({
                    "netWorthBase": last.net_worth_base,
                    "weekAgoNetWorthBase": week_ago.net_worth_base,
                    "changePct": change,
                });
                out.push(fresh(
                    NotificationKind::NetWorthDip,
                    NotificationSeverity::Warning,
                    "Net worth dipped this week".to_string(),
                    format!(
                        "Your net worth is down {:.1}% vs last week. Often this is just market noise — but worth a glance.",
                        (-change) * 100.0
                    ),
                    Some("mizan://dashboard".to_string()),
                    payload.to_string(),
                    format!("net_worth_dip:{}", today_str(input)),
                ));
            }
        }
    }

    out
}

fn eval_cash_drag(input: &InsightsInput) -> Option<Notification> {
    let pct = dec_to_f64(input.cash_pct_of_net_worth?);
    let days = input.cash_high_for_days?;
    if pct < CASH_DRAG_PCT_THRESHOLD || days < CASH_DRAG_DAYS_THRESHOLD {
        return None;
    }
    let payload = json!({
        "cashPct": pct,
        "daysAbove": days,
    });
    Some(fresh(
        NotificationKind::CashDrag,
        NotificationSeverity::Info,
        "Cash has been sitting idle".to_string(),
        format!(
            "Cash is {:.0}% of your net worth and has been above the {:.0}% threshold for {} days. Consider deploying or laddering.",
            pct * 100.0,
            CASH_DRAG_PCT_THRESHOLD * 100.0,
            days,
        ),
        Some("mizan://dashboard".to_string()),
        payload.to_string(),
        format!("cash_drag:{}", today_str(input)),
    ))
}

fn eval_sync_failures(input: &InsightsInput) -> Vec<Notification> {
    input
        .sync_failures
        .iter()
        .map(|f| sync_failure_notification(f, input))
        .collect()
}

fn eval_dividend_events(input: &InsightsInput) -> Vec<Notification> {
    input
        .dividend_events
        .iter()
        .map(|e| dividend_notification(e, input))
        .collect()
}

fn dividend_notification(e: &DividendEvent, input: &InsightsInput) -> Notification {
    // Title is short — "Dividend posted • PLTR".
    // Body carries the amount in base currency.
    let kind_label = match e.kind.as_str() {
        "INTEREST" => "Interest",
        _ => "Dividend",
    };
    let title = format!("{kind_label} posted • {}", e.symbol);
    let body = format!(
        "{} of {} {:.2} from {} on {}.",
        kind_label,
        input.base_currency,
        dec_to_f64(e.amount_base),
        e.symbol,
        e.posted_on,
    );
    let payload = json!({
        "activityId": e.activity_id,
        "kind": e.kind,
        "symbol": e.symbol,
        "amountBase": e.amount_base,
        "postedOn": e.posted_on.to_string(),
    });
    // Dedupe on the activity id — re-running the engine for the same
    // payment is a no-op (the UNIQUE on the storage layer enforces it).
    let dedupe = format!("dividend:{}", e.activity_id);
    fresh(
        NotificationKind::DividendPosted,
        NotificationSeverity::Success,
        title,
        body,
        Some(format!("mizan://holding/{}", e.symbol)),
        payload.to_string(),
        dedupe,
    )
}

fn sync_failure_notification(f: &SyncFailureInput, input: &InsightsInput) -> Notification {
    let payload = json!({
        "provider": f.provider,
        "reason": f.reason,
        "lastSuccessAtMs": f.last_success_at_ms,
    });
    fresh(
        NotificationKind::SyncFailure,
        NotificationSeverity::Warning,
        format!("{} sync needs attention", f.provider),
        format!(
            "{} hasn't been able to refresh: {}. Your numbers may be stale.",
            f.provider, f.reason
        ),
        Some(format!("mizan://settings/sync/{}", f.provider)),
        payload.to_string(),
        format!("sync_failure:{}:{}", f.provider, today_str(input)),
    )
}

/// Run every rule against the input and return the union, in a stable
/// order (BigMove → GoalMilestone → ATH/Dip → CashDrag → DividendPosted
/// → SyncFailure). Order matters because the bell panel renders the
/// result list as-is for batches emitted in the same tick.
pub fn evaluate(input: &InsightsInput) -> Vec<Notification> {
    let mut out = Vec::new();
    if let Some(n) = eval_big_move(input) {
        out.push(n);
    }
    out.extend(eval_goal_milestones(input));
    out.extend(eval_net_worth_dip_or_ath(input));
    if let Some(n) = eval_cash_drag(input) {
        out.push(n);
    }
    out.extend(eval_dividend_events(input));
    out.extend(eval_sync_failures(input));
    out
}
