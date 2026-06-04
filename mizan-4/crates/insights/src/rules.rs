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

use super::input::{
    BondMaturityCandidate, CashDragOpportunityCandidate, ConcentrationRiskFinding, DividendEvent,
    FxPairMove, GoalProgress, HawlAnchorCandidate, InsightsInput, ShariaStatusChange,
    SyncFailureInput, TaxOptimizationWindow,
};
use mizan_core::notifications::{Notification, NotificationKind, NotificationSeverity};

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

/// Bond/sukuk maturity-approaching thresholds — fire once per crossed
/// step per Goal v3 §V step C5.a. The §23 scenario needs the 47-day
/// reminder; 90 covers that, and we step down 30/7/1 so the user gets a
/// month-out + final-week + final-day nudge as well.
///
/// Ordering matters: declared smallest-first so `iter().find(|t| days <= t)`
/// returns the smallest crossed step (e.g. days=5 → step=7, not step=90).
const BOND_MATURITY_DAY_THRESHOLDS: &[i64] = &[1, 7, 30, 90];

/// Minimum principal returning at maturity to qualify for a notification.
/// Same logic as BigMove's value floor — suppress notifications on
/// trivial principal returns where the user already knows.
const BOND_MATURITY_MIN_PRINCIPAL_BASE: f64 = 1_000.0;

/// FX-moved-materially threshold: an FX pair the user has exposure to
/// must move > this absolute fraction over the comparison window.
/// 3% is the "material to a Singapore millionaire with USD-base CPF"
/// calibration from Goal v3 §23.
const FX_MATERIAL_MOVE_PCT: f64 = 0.03;

/// Minimum exposure (base currency) to qualify for an FX-moved-materially
/// notification. A $50 FX exposure moving 10% isn't actionable.
const FX_MIN_EXPOSURE_BASE: f64 = 5_000.0;

/// Zakat-Hawl approach thresholds per Goal v3 §V step C5.b. Declared
/// smallest-first so `iter().find(|t| days <= t)` returns the smallest
/// crossed step (e.g. days=5 → 7, days=1 → 1).
const HAWL_DAY_THRESHOLDS: &[i64] = &[1, 7, 30];

/// Concentration-risk threshold: a single dimension's fraction of
/// net worth above this triggers a notification. 25% is the
/// "don't blow up on one issuer" calibration that maps to a Sharia-
/// aware millionaire holding three to four core Sukuk issuers.
const CONCENTRATION_THRESHOLD_PCT: f64 = 0.25;

/// Concentration-risk min absolute exposure (base) — a 30% concentration
/// in a $200 hobby account isn't actionable.
const CONCENTRATION_MIN_EXPOSURE_BASE: f64 = 10_000.0;

/// Cash-drag-opportunity yield gap threshold. If an alternative
/// instrument yields > this much MORE than the user's current cash
/// yield, surface the opportunity. 1.5% is the "noticeably better
/// while staying low-risk" calibration.
const CASH_DRAG_OPPORTUNITY_YIELD_GAP_PCT: f64 = 0.015;

/// Cash-drag-opportunity minimum cash amount (base). Below this the
/// yield gap × principal doesn't justify the notification noise.
const CASH_DRAG_OPPORTUNITY_MIN_CASH_BASE: f64 = 5_000.0;

/// Tax-window thresholds per Goal v3 §V step C5.b. Same smallest-first
/// ordering as HAWL/BOND so iter().find returns the smallest crossed step.
const TAX_WINDOW_DAY_THRESHOLDS: &[i64] = &[1, 7, 30, 90];

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

/// Bond / sukuk maturity-approaching rule per Goal v3 §V step C5.a.
///
/// Emits one Notification per holding per crossed threshold (90/30/7/1
/// day). The `days_to_maturity` value selects the smallest threshold
/// that is >= the days remaining — i.e. a bond with 47 days to maturity
/// crosses the 30-day step (since 47 > 30 is false; 47 <= 90 is the
/// crossed step). Caller is expected to evict the candidate from the
/// next tick after each step fires so the engine doesn't re-emit.
///
/// Severity:
///   - 90/30 day → Info (planning horizon)
///   - 7 day → Warning (act this week)
///   - 1 day → Critical (today)
fn eval_bond_maturity(
    candidate: &BondMaturityCandidate,
    input: &InsightsInput,
) -> Option<Notification> {
    if dec_to_f64(candidate.principal_returning_base) < BOND_MATURITY_MIN_PRINCIPAL_BASE {
        return None;
    }
    // Pick the smallest threshold that's still >= days_to_maturity.
    // Iteration order matters: thresholds are declared 90,30,7,1 so we
    // walk descending and stop at the first match.
    let crossed = BOND_MATURITY_DAY_THRESHOLDS
        .iter()
        .find(|t| candidate.days_to_maturity <= **t)?;
    let severity = match *crossed {
        1 => NotificationSeverity::Critical,
        7 => NotificationSeverity::Warning,
        _ => NotificationSeverity::Info,
    };
    let title = format!(
        "{} matures in {} {}",
        candidate.symbol,
        candidate.days_to_maturity,
        if candidate.days_to_maturity == 1 {
            "day"
        } else {
            "days"
        },
    );
    let body = format!(
        "{} {:.0} returns to your account on {} — start shortlisting replacements.",
        input.base_currency,
        dec_to_f64(candidate.principal_returning_base),
        candidate.maturity_date,
    );
    let payload = json!({
        "holdingId": candidate.holding_id,
        "symbol": candidate.symbol,
        "maturityDate": candidate.maturity_date,
        "daysToMaturity": candidate.days_to_maturity,
        "principalReturningBase": candidate.principal_returning_base,
        "thresholdDays": crossed,
    });
    let dedupe = format!(
        "bond_maturity:{}:{}:{}",
        candidate.holding_id, crossed, candidate.maturity_date
    );
    Some(fresh(
        NotificationKind::BondMaturityApproaching,
        severity,
        title,
        body,
        Some(format!("mizan://holding/{}", candidate.holding_id)),
        payload.to_string(),
        dedupe,
    ))
}

/// FX-moved-materially rule per Goal v3 §V step C5.a.
///
/// Emits one Notification per FX pair per day where the move over the
/// configured `window_days` exceeded FX_MATERIAL_MOVE_PCT AND the user's
/// exposure to that pair exceeds the minimum-exposure threshold.
fn eval_fx_moved(pair: &FxPairMove, input: &InsightsInput) -> Option<Notification> {
    if dec_to_f64(pair.change_pct).abs() < FX_MATERIAL_MOVE_PCT {
        return None;
    }
    if dec_to_f64(pair.exposure_base) < FX_MIN_EXPOSURE_BASE {
        return None;
    }
    let dir = if dec_to_f64(pair.change_pct) >= 0.0 {
        "strengthened"
    } else {
        "weakened"
    };
    let pct = fmt_pct_signed(pair.change_pct);
    let title = format!("{}/{} {}", pair.from_currency, pair.to_currency, pct);
    let body = format!(
        "{} has {} {} against {} over {} days — your {:.0} {} exposure is now worth notably more.",
        pair.from_currency,
        dir,
        pct,
        pair.to_currency,
        pair.window_days,
        dec_to_f64(pair.exposure_base),
        input.base_currency,
    );
    let payload = json!({
        "fromCurrency": pair.from_currency,
        "toCurrency": pair.to_currency,
        "windowDays": pair.window_days,
        "changePct": pair.change_pct,
        "exposureBase": pair.exposure_base,
    });
    let dedupe = format!(
        "fx_moved:{}_{}_{}d:{}",
        pair.from_currency,
        pair.to_currency,
        pair.window_days,
        today_str(input)
    );
    Some(fresh(
        NotificationKind::FxMovedMaterially,
        NotificationSeverity::Info,
        title,
        body,
        Some(format!(
            "mizan://fx/{}/{}",
            pair.from_currency, pair.to_currency
        )),
        payload.to_string(),
        dedupe,
    ))
}

/// Sharia-status-changed rule per Goal v3 §V step C5.a.
///
/// Emits one Notification per holding whose AAOIFI screening verdict
/// flipped since the prior evaluation. Severity is `Warning` because
/// the user often needs to act (purify dividends, exit position, etc.).
fn eval_sharia_status_change(
    change: &ShariaStatusChange,
    input: &InsightsInput,
) -> Option<Notification> {
    if change.prior_verdict == change.new_verdict {
        return None;
    }
    let title = format!(
        "{} Sharia status: {} → {}",
        change.symbol, change.prior_verdict, change.new_verdict
    );
    let body = format!(
        "AAOIFI screening for {} flipped from {} to {}. Review whether action is required.",
        change.symbol, change.prior_verdict, change.new_verdict
    );
    let payload = json!({
        "holdingId": change.holding_id,
        "symbol": change.symbol,
        "priorVerdict": change.prior_verdict,
        "newVerdict": change.new_verdict,
    });
    let dedupe = format!(
        "sharia_status:{}:{}->{}:{}",
        change.holding_id,
        change.prior_verdict,
        change.new_verdict,
        today_str(input)
    );
    Some(fresh(
        NotificationKind::ShariaStatusChanged,
        NotificationSeverity::Warning,
        title,
        body,
        Some(format!("mizan://holding/{}", change.holding_id)),
        payload.to_string(),
        dedupe,
    ))
}

/// Zakat-Hawl approaching rule per Goal v3 §V step C5.b.
///
/// Fires once per cohort per crossed threshold (30/7/1 day). Sourced
/// from `hawl_anchors` (Track F PR-F1); caller computes days-to-
/// completion against the user's local "today" so the engine stays
/// pure.
///
/// Severity: 30=Info (plan), 7=Warning (act this week), 1=Critical (today).
fn eval_hawl_approaching(
    candidate: &HawlAnchorCandidate,
    _input: &InsightsInput,
) -> Option<Notification> {
    let crossed = HAWL_DAY_THRESHOLDS
        .iter()
        .find(|t| candidate.days_to_completion <= **t)?;
    let severity = match *crossed {
        1 => NotificationSeverity::Critical,
        7 => NotificationSeverity::Warning,
        _ => NotificationSeverity::Info,
    };
    let title = format!(
        "Zakat Hawl: {} completes in {} {}",
        candidate.cohort_label,
        candidate.days_to_completion,
        if candidate.days_to_completion == 1 {
            "day"
        } else {
            "days"
        },
    );
    let body = format!(
        "Qualifying balance ~{:.0} — preview your Zakat liability before the Hawl closes.",
        dec_to_f64(candidate.qualifying_amount_base),
    );
    let payload = json!({
        "cohortId": candidate.cohort_id,
        "cohortLabel": candidate.cohort_label,
        "daysToCompletion": candidate.days_to_completion,
        "qualifyingAmountBase": candidate.qualifying_amount_base,
        "thresholdDays": crossed,
    });
    let dedupe = format!("zakat_hawl:{}:{}", candidate.cohort_id, crossed);
    Some(fresh(
        NotificationKind::ZakatHawlApproaching,
        severity,
        title,
        body,
        Some(format!("mizan://zakat/cohort/{}", candidate.cohort_id)),
        payload.to_string(),
        dedupe,
    ))
}

/// Concentration-risk rule per Goal v3 §V step C5.b.
///
/// Fires per (dimension, label) where the fraction-of-net-worth exceeds
/// CONCENTRATION_THRESHOLD_PCT AND absolute exposure exceeds the min.
/// Severity is Warning — concentration matters but the user often
/// chose it deliberately (e.g. employer equity).
fn eval_concentration_risk(
    finding: &ConcentrationRiskFinding,
    input: &InsightsInput,
) -> Option<Notification> {
    if dec_to_f64(finding.fraction_of_net_worth) < CONCENTRATION_THRESHOLD_PCT {
        return None;
    }
    if dec_to_f64(finding.exposure_base) < CONCENTRATION_MIN_EXPOSURE_BASE {
        return None;
    }
    let pct_display = (dec_to_f64(finding.fraction_of_net_worth) * 100.0).round();
    let title = format!(
        "Concentration risk: {} {} ({:.0}%)",
        finding.label, finding.dimension, pct_display,
    );
    let body = format!(
        "{} {} represents {:.0}% of net worth ({} {:.0}). Consider whether the concentration matches your intent.",
        finding.label,
        finding.dimension,
        pct_display,
        input.base_currency,
        dec_to_f64(finding.exposure_base),
    );
    let payload = json!({
        "dimension": finding.dimension,
        "label": finding.label,
        "fractionOfNetWorth": finding.fraction_of_net_worth,
        "exposureBase": finding.exposure_base,
    });
    let dedupe = format!(
        "concentration:{}:{}:{}",
        finding.dimension,
        finding.label,
        today_str(input)
    );
    Some(fresh(
        NotificationKind::ConcentrationRisk,
        NotificationSeverity::Warning,
        title,
        body,
        Some(format!(
            "mizan://concentration/{}/{}",
            finding.dimension, finding.label
        )),
        payload.to_string(),
        dedupe,
    ))
}

/// Cash-drag-opportunity rule per Goal v3 §V step C5.b.
///
/// Distinct from `CashDrag`: that rule fires on cash sitting too long
/// (duration). This rule fires on the presence of a higher-yielding
/// alternative AND a material yield gap AND a material cash amount.
fn eval_cash_drag_opportunity(
    candidate: &CashDragOpportunityCandidate,
    input: &InsightsInput,
) -> Option<Notification> {
    if dec_to_f64(candidate.cash_amount_base) < CASH_DRAG_OPPORTUNITY_MIN_CASH_BASE {
        return None;
    }
    let gap = dec_to_f64(candidate.alternative_yield_pct) - dec_to_f64(candidate.current_yield_pct);
    if gap < CASH_DRAG_OPPORTUNITY_YIELD_GAP_PCT {
        return None;
    }
    let cur_pct = dec_to_f64(candidate.current_yield_pct) * 100.0;
    let alt_pct = dec_to_f64(candidate.alternative_yield_pct) * 100.0;
    let title = format!("Cash drag: {:.1}% vs {:.1}% available", cur_pct, alt_pct);
    let body = format!(
        "Your {} {:.0} earning {:.1}% could move to {} earning {:.1}% — a {:.1}% improvement.",
        input.base_currency,
        dec_to_f64(candidate.cash_amount_base),
        cur_pct,
        candidate.alternative_label,
        alt_pct,
        gap * 100.0,
    );
    let payload = json!({
        "cashAmountBase": candidate.cash_amount_base,
        "currentYieldPct": candidate.current_yield_pct,
        "alternativeYieldPct": candidate.alternative_yield_pct,
        "alternativeLabel": candidate.alternative_label,
    });
    let dedupe = format!("cash_drag_opp:{}", today_str(input));
    Some(fresh(
        NotificationKind::CashDragOpportunity,
        NotificationSeverity::Info,
        title,
        body,
        Some("mizan://cash-drag-opportunity".into()),
        payload.to_string(),
        dedupe,
    ))
}

/// Tax-optimization-window rule per Goal v3 §V step C5.b.
///
/// Fires per window per crossed threshold (90/30/7/1 day). Caller
/// computes from the user's jurisdiction; engine compares against
/// the threshold ladder.
///
/// Severity: 90/30=Info, 7=Warning, 1=Critical.
fn eval_tax_window(window: &TaxOptimizationWindow, _input: &InsightsInput) -> Option<Notification> {
    let crossed = TAX_WINDOW_DAY_THRESHOLDS
        .iter()
        .find(|t| window.days_remaining <= **t)?;
    let severity = match *crossed {
        1 => NotificationSeverity::Critical,
        7 => NotificationSeverity::Warning,
        _ => NotificationSeverity::Info,
    };
    let title = format!(
        "{} — {} {} remaining",
        window.label,
        window.days_remaining,
        if window.days_remaining == 1 {
            "day"
        } else {
            "days"
        },
    );
    let body = match window.potential_savings_base {
        Some(savings) => format!(
            "Window closes in {} days. Potential savings {:.0}.",
            window.days_remaining,
            dec_to_f64(savings),
        ),
        None => format!("Window closes in {} days.", window.days_remaining),
    };
    let payload = json!({
        "kind": window.kind,
        "daysRemaining": window.days_remaining,
        "label": window.label,
        "potentialSavingsBase": window.potential_savings_base,
        "thresholdDays": crossed,
    });
    let dedupe = format!("tax_window:{}:{}", window.kind, crossed);
    Some(fresh(
        NotificationKind::TaxOptimizationWindow,
        severity,
        title,
        body,
        Some(format!("mizan://tax/{}", window.kind)),
        payload.to_string(),
        dedupe,
    ))
}

/// Run every rule against the input and return the union, in a stable
/// order (BigMove → GoalMilestone → ATH/Dip → CashDrag → DividendPosted
/// → SyncFailure → BondMaturityApproaching → FxMovedMaterially →
/// ShariaStatusChanged → ZakatHawlApproaching → ConcentrationRisk →
/// CashDragOpportunity → TaxOptimizationWindow). Order matters because
/// the bell panel renders the result list as-is for batches emitted in
/// the same tick.
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
    // Bond / FX / Sharia rules added in Track C PR-C5.a — they're per-
    // item (one notification per holding/pair/change), so we iterate
    // the caller-supplied slice rather than re-querying inside the
    // eval fn. Engine stays pure; caller decides which items qualify.
    for candidate in &input.bond_maturity_candidates {
        if let Some(n) = eval_bond_maturity(candidate, input) {
            out.push(n);
        }
    }
    for pair in &input.fx_pair_moves {
        if let Some(n) = eval_fx_moved(pair, input) {
            out.push(n);
        }
    }
    for change in &input.sharia_status_changes {
        if let Some(n) = eval_sharia_status_change(change, input) {
            out.push(n);
        }
    }
    // Hawl / Concentration / CashDragOpportunity / TaxWindow rules
    // added in Track C PR-C5.b — same per-item pattern as PR-C5.a.
    for candidate in &input.hawl_anchors_approaching {
        if let Some(n) = eval_hawl_approaching(candidate, input) {
            out.push(n);
        }
    }
    for finding in &input.concentration_findings {
        if let Some(n) = eval_concentration_risk(finding, input) {
            out.push(n);
        }
    }
    for candidate in &input.cash_drag_opportunities {
        if let Some(n) = eval_cash_drag_opportunity(candidate, input) {
            out.push(n);
        }
    }
    for window in &input.tax_optimization_windows {
        if let Some(n) = eval_tax_window(window, input) {
            out.push(n);
        }
    }
    out
}
