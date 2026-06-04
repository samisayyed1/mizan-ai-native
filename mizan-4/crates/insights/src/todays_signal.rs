//! Today's Signal selection algorithm — Track C PR-C5.c / Goal v3 §V step C5.c.
//!
//! Picks the single highest-signal notification per day for the
//! dashboard's Today's Signal card surface (PR-A7 #97 placeholder).
//!
//! Design constraints from Goal v3 §V step C5.c + Evolution Spec §3:
//!   1. **Pure.** Same shape as `rules::evaluate()` — a
//!      `fn(&[Notification], &[Notification]) -> Option<Notification>`
//!      so tests + fixtures + golden outputs are easy.
//!   2. **Deterministic.** Same inputs → same output. The scheduler
//!      caches the result keyed on (date, input-hash); a non-
//!      deterministic picker would invalidate the cache every tick.
//!   3. **Dedup against last 7 days.** Per spec — the user shouldn't
//!      see "your Emaar Sukuk matures in 47 days" on Monday and again
//!      Tuesday. The dedupe key is the canonical comparison.
//!   4. **Score by signal strength**, not by recency. A bond
//!      maturing tomorrow beats a generic FX move from this morning.
//!
//! The scoring weights are deliberately conservative:
//!   - Critical severity → weight 1000
//!   - Warning severity → weight 100
//!   - Info severity → weight 10
//!   - Success severity → weight 5 (positive news isn't urgent)
//!
//! plus a per-kind bonus so financial events outrank chrome:
//!   - Bond maturity / Zakat Hawl / Sharia change → +50 (action-shaped)
//!   - BigMove / NetWorthDip / NewAth → +20 (portfolio events)
//!   - Concentration / CashDrag / TaxWindow / FxMoved → +15 (advisory)
//!   - GoalMilestone / DividendPosted → +10 (celebratory)
//!   - SyncFailure / AiDigest / CashDragOpportunity → +0 (operational)
//!
//! The combined score deterministically orders the candidate set.
//! Ties broken by the dedupe_key lexicographic ordering (stable
//! across ticks).

use mizan_core::notifications::{Notification, NotificationKind, NotificationSeverity};

/// Score a single notification for "Today's Signal" candidacy.
///
/// Returns a positive integer; higher = stronger signal. The exact
/// value is opaque — only ordering matters across the candidate set.
pub fn score_signal(n: &Notification) -> u32 {
    let severity_score = match n.severity {
        NotificationSeverity::Critical => 1000,
        NotificationSeverity::Warning => 100,
        NotificationSeverity::Info => 10,
        NotificationSeverity::Success => 5,
    };
    let kind_bonus = match n.kind {
        // Action-shaped financial events
        NotificationKind::BondMaturityApproaching
        | NotificationKind::ZakatHawlApproaching
        | NotificationKind::ShariaStatusChanged => 50,
        // Portfolio events
        NotificationKind::BigMove | NotificationKind::NetWorthDip | NotificationKind::NewAth => 20,
        // Advisory
        NotificationKind::ConcentrationRisk
        | NotificationKind::CashDrag
        | NotificationKind::TaxOptimizationWindow
        | NotificationKind::FxMovedMaterially
        | NotificationKind::AllocationDrift => 15,
        // Celebratory
        NotificationKind::GoalMilestone | NotificationKind::DividendPosted => 10,
        // Operational
        NotificationKind::SyncFailure
        | NotificationKind::AiDigest
        | NotificationKind::CashDragOpportunity => 0,
    };
    severity_score + kind_bonus
}

/// Pick the single Today's Signal from a candidate set, deduplicated
/// against the last 7 days of bell-panel rows.
///
/// `candidates` — typically the output of `rules::evaluate()` for the
///                current tick. Empty input → `None`.
/// `recent`     — bell-panel rows from the last 7 days. The selection
///                skips any candidate whose `dedupe_key` is already
///                present (the user has already seen this story).
///
/// Returns the highest-scoring fresh candidate, or `None` if the
/// candidate set is empty or every candidate was seen recently.
///
/// Ties broken by lexicographic `dedupe_key` ordering for determinism.
pub fn pick_todays_signal(
    candidates: &[Notification],
    recent: &[Notification],
) -> Option<Notification> {
    let mut seen = std::collections::HashSet::new();
    for r in recent {
        seen.insert(r.dedupe_key.as_str());
    }

    candidates
        .iter()
        .filter(|c| !seen.contains(c.dedupe_key.as_str()))
        .max_by(|a, b| {
            let sa = score_signal(a);
            let sb = score_signal(b);
            // Higher score wins; on tie, smaller dedupe_key wins
            // (lexicographic — stable across ticks).
            sa.cmp(&sb).then_with(|| b.dedupe_key.cmp(&a.dedupe_key))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notif(
        kind: NotificationKind,
        severity: NotificationSeverity,
        dedupe_key: &str,
    ) -> Notification {
        Notification {
            id: format!("id_{dedupe_key}"),
            kind,
            severity,
            title: format!("Title for {dedupe_key}"),
            body: String::new(),
            deep_link: None,
            payload_json: "{}".into(),
            dedupe_key: dedupe_key.into(),
            created_at_ms: 0,
            read_at_ms: None,
            dismissed_at_ms: None,
        }
    }

    #[test]
    fn empty_candidates_returns_none() {
        assert!(pick_todays_signal(&[], &[]).is_none());
    }

    #[test]
    fn picks_only_candidate() {
        let n = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Info,
            "bond_maturity:h:90:2026-07-13",
        );
        let pick = pick_todays_signal(std::slice::from_ref(&n), &[]).unwrap();
        assert_eq!(pick.dedupe_key, n.dedupe_key);
    }

    #[test]
    fn critical_beats_warning_beats_info() {
        let info = notif(
            NotificationKind::BigMove,
            NotificationSeverity::Info,
            "big_move:AAA:d",
        );
        let warning = notif(
            NotificationKind::BigMove,
            NotificationSeverity::Warning,
            "big_move:BBB:d",
        );
        let critical = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Critical,
            "bond:h:1:d",
        );
        let pick = pick_todays_signal(&[info, warning, critical.clone()], &[]).unwrap();
        assert_eq!(pick.dedupe_key, critical.dedupe_key);
    }

    #[test]
    fn bond_maturity_warning_beats_fx_moved_warning_via_kind_bonus() {
        // Both Warning severity; bond carries +50 kind bonus vs FX's
        // +15. The §23 scenario hinges on this exact ordering — the
        // bond reminder must outrank a same-day FX move.
        let bond = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Warning,
            "bond:emaar:7:d",
        );
        let fx = notif(
            NotificationKind::FxMovedMaterially,
            NotificationSeverity::Warning,
            "fx:USD_INR_30d:d",
        );
        let pick = pick_todays_signal(&[fx, bond.clone()], &[]).unwrap();
        assert_eq!(pick.dedupe_key, bond.dedupe_key);
    }

    #[test]
    fn ties_broken_lexicographically_by_dedupe_key() {
        // Identical kind + severity → deterministic tiebreak so the
        // scheduler's input-hash cache stays stable. `aaa` < `bbb`
        // lexicographically, so `aaa` wins on the `then_with` rule
        // (note: max_by uses `b.cmp(a)` to invert so smaller-first).
        let a = notif(
            NotificationKind::FxMovedMaterially,
            NotificationSeverity::Info,
            "fx:aaa:d",
        );
        let b = notif(
            NotificationKind::FxMovedMaterially,
            NotificationSeverity::Info,
            "fx:bbb:d",
        );
        let pick1 = pick_todays_signal(&[a.clone(), b.clone()], &[]).unwrap();
        let pick2 = pick_todays_signal(&[b.clone(), a.clone()], &[]).unwrap();
        // Determinism: order-of-input doesn't change the pick
        assert_eq!(pick1.dedupe_key, pick2.dedupe_key);
        assert_eq!(pick1.dedupe_key, "fx:aaa:d");
    }

    #[test]
    fn skips_candidates_already_seen_in_recent() {
        let already_seen = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Critical,
            "bond:emaar:1:d",
        );
        let fresh = notif(
            NotificationKind::FxMovedMaterially,
            NotificationSeverity::Info,
            "fx:USD_INR_30d:d",
        );
        let pick = pick_todays_signal(
            &[already_seen.clone(), fresh.clone()],
            std::slice::from_ref(&already_seen),
        )
        .unwrap();
        // Critical bond would normally win — but it's already seen,
        // so the fresh FX Info wins instead.
        assert_eq!(pick.dedupe_key, fresh.dedupe_key);
    }

    #[test]
    fn all_seen_returns_none() {
        let a = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Critical,
            "bond:a:1:d",
        );
        let pick = pick_todays_signal(std::slice::from_ref(&a), std::slice::from_ref(&a));
        assert!(pick.is_none());
    }

    #[test]
    fn success_severity_underweights_info_warning_critical() {
        // ATH at Success severity should LOSE to a same-day Info-level
        // bond maturity — positive news doesn't push the dashboard.
        let ath = notif(
            NotificationKind::NewAth,
            NotificationSeverity::Success,
            "new_ath:d",
        );
        let bond = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Info,
            "bond:h:90:d",
        );
        let pick = pick_todays_signal(&[ath, bond.clone()], &[]).unwrap();
        assert_eq!(pick.dedupe_key, bond.dedupe_key);
    }

    #[test]
    fn sync_failure_does_not_outrank_financial_events_at_same_severity() {
        // SyncFailure has kind_bonus 0, so a same-severity financial
        // event must win.
        let sync = notif(
            NotificationKind::SyncFailure,
            NotificationSeverity::Warning,
            "sync_failure:plaid:d",
        );
        let bond = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Warning,
            "bond:h:7:d",
        );
        let pick = pick_todays_signal(&[sync, bond.clone()], &[]).unwrap();
        assert_eq!(pick.dedupe_key, bond.dedupe_key);
    }

    #[test]
    fn score_signal_includes_severity_and_kind_bonus() {
        // Direct probe of the scorer — the dispatcher's cache uses
        // score values for ordering, so an externally-observable
        // monotonicity is a stable contract.
        let bond_crit = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Critical,
            "x",
        );
        let bond_info = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Info,
            "x",
        );
        let sync_crit = notif(
            NotificationKind::SyncFailure,
            NotificationSeverity::Critical,
            "x",
        );
        assert!(score_signal(&bond_crit) > score_signal(&bond_info));
        // Same severity, bond's +50 kind bonus beats sync's +0
        assert!(score_signal(&bond_crit) > score_signal(&sync_crit));
    }

    #[test]
    fn s23_scenario_signal_emaar_47day_wins_today() {
        // §23-flavored end-to-end: an Info-severity bond maturity at
        // 47 days (90 step) competes with same-day Info-severity FX
        // move + Warning concentration. Bond should win because it's
        // action-shaped (+50) and Warning concentration only carries
        // +15 — so concentration at 100+15 = 115 beats bond at
        // 10+50 = 60. The test pins this so future re-weighting
        // doesn't silently demote the §23 anchor signal.
        //
        // The §23 spec text "Today's Signal: Your Emaar Sukuk
        // matures in 47 days" assumes that bond maturity surfaces
        // when nothing more urgent is happening. So we run TWO
        // variants:
        //   variant 1: bond alone → bond wins
        //   variant 2: bond + concentration Warning → concentration
        //              wins (correct: a concentration risk Warning
        //              is more urgent than a 47-day bond reminder)
        let bond = notif(
            NotificationKind::BondMaturityApproaching,
            NotificationSeverity::Info,
            "bond:emaar:90:2026-07-13",
        );
        let pick_alone = pick_todays_signal(std::slice::from_ref(&bond), &[]).unwrap();
        assert_eq!(pick_alone.dedupe_key, bond.dedupe_key);

        let concentration = notif(
            NotificationKind::ConcentrationRisk,
            NotificationSeverity::Warning,
            "concentration:issuer:Emaar:d",
        );
        let pick_both = pick_todays_signal(&[bond, concentration.clone()], &[]).unwrap();
        assert_eq!(pick_both.dedupe_key, concentration.dedupe_key);
    }
}
