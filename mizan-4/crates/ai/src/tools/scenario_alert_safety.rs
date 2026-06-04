//! Scenario + alert + activity tool safety descriptors —
//! Track C PR-C4.e / ADR 0020 rows 4, 5, 12, 13, 14, 18, 19.
//!
//! This PR completes the ADR 0020 inventory. After it lands, every
//! one of the 22 inventory rows + the Goal v3 `find_sharia_status`
//! bridge has a safety descriptor registered through the
//! `register_tool!` macro from PR-C3.b (#102).
//!
//! The seven tools registered here:
//!
//!   - row 4:  `add_activity`         — cap 5, FullInput, PerActivityType,  **emits_truth_ledger=true**
//!   - row 5:  `list_activities`      — cap 1, FilterAndCount, None, false
//!   - row 12: `set_reminder`         — cap 2, FullInput, DateFuture{2y}, false
//!   - row 13: `set_alert`            — cap 2, FullInput, AssetClassThreshold, false
//!   - row 14: `get_news`             — cap 1, FilterAndCount, None, false
//!   - row 18: `run_scenario`         — cap 8, FullInput, ScenarioInputs, false
//!   - row 19: `compare_scenarios`    — cap 5, Custom("scenario_ids_list"), None, false
//!
//! `add_activity` is the **fifth and final** Truth-Ledger-emitting
//! tool in the registry (after create/update/delete_holding from
//! PR-C4.a). Its handler in PR-C5 MUST satisfy
//! `debug_assert_truth_ledger_contract` at boot.

use crate::tool_registry::{register_tool, AuditScope, NumericBounds, ToolRegistration};

/// ADR 0020 row 4: `add_activity`.
///
/// - **cap_weight**: 5 — standard mutation budget.
/// - **audit_scope**: `FullInput { redact: &[] }` — every activity
///   field is non-sensitive (no Plaid tokens or PII).
/// - **numeric_bounds**: `PerActivityType` — dispatcher consults the
///   per-activity-type bound table (dividend ≥ 0; trade qty > 0;
///   fee ≥ 0 per ADR 0020).
/// - **emits_truth_ledger**: `true` — every activity emits the
///   `ActivityRecorded` chain entry; handler must satisfy
///   `debug_assert_truth_ledger_contract` at boot.
pub fn add_activity() -> ToolRegistration {
    register_tool!(
        name: "add_activity",
        cap_weight: 5,
        audit_scope: AuditScope::FullInput { redact: &[] },
        numeric_bounds: NumericBounds::PerActivityType,
        emits_truth_ledger: true,
    )
}

/// ADR 0020 row 5: `list_activities`.
///
/// - **cap_weight**: 1 — cheap read.
/// - **audit_scope**: `FilterAndCount` — log filter args + N rows.
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false` — read-only.
pub fn list_activities() -> ToolRegistration {
    register_tool!(
        name: "list_activities",
        cap_weight: 1,
        audit_scope: AuditScope::FilterAndCount,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 12: `set_reminder`.
///
/// - **cap_weight**: 2 — cheap write to the reminders table.
/// - **audit_scope**: `FullInput { redact: &[] }` — log the reminder
///   text + due date.
/// - **numeric_bounds**: `DateFuture { max_horizon_secs: 2 years }`
///   per ADR 0020 row 12 (`due_date ∈ (now, now + 2y]`). 2y =
///   2 × 365 × 86400 = 63_072_000 seconds.
/// - **emits_truth_ledger**: `false` — reminders have their own
///   audit trail in the reminders table; the Truth Ledger is for
///   financial mutations only per CLAUDE.md §0 rule 1.
pub fn set_reminder() -> ToolRegistration {
    register_tool!(
        name: "set_reminder",
        cap_weight: 2,
        audit_scope: AuditScope::FullInput { redact: &[] },
        numeric_bounds: NumericBounds::DateFuture {
            max_horizon_secs: 60 * 60 * 24 * 365 * 2,
        },
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 13: `set_alert`.
///
/// - **cap_weight**: 2 — cheap write to the alerts table.
/// - **audit_scope**: `FullInput { redact: &[] }`.
/// - **numeric_bounds**: `AssetClassThreshold` — dispatcher consults
///   the per-asset-class threshold table per ADR 0021.
/// - **emits_truth_ledger**: `false` — alerts have their own audit
///   trail; not financial mutations.
pub fn set_alert() -> ToolRegistration {
    register_tool!(
        name: "set_alert",
        cap_weight: 2,
        audit_scope: AuditScope::FullInput { redact: &[] },
        numeric_bounds: NumericBounds::AssetClassThreshold,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 14: `get_news`.
///
/// - **cap_weight**: 1 — cheap read from the `news_items` table
///   (populated by Track D).
/// - **audit_scope**: `FilterAndCount`.
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false` — read-only.
pub fn get_news() -> ToolRegistration {
    register_tool!(
        name: "get_news",
        cap_weight: 1,
        audit_scope: AuditScope::FilterAndCount,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 18: `run_scenario`.
///
/// - **cap_weight**: 8 — heaviest tier (Monte Carlo simulation).
/// - **audit_scope**: `FullInput { redact: &[] }` — log all scenario
///   inputs so the result is reproducible.
/// - **numeric_bounds**: `ScenarioInputs` — dispatcher enforces
///   per-input bounds (e.g. `equity_return ∈ [-1, 1]`).
/// - **emits_truth_ledger**: `false` — projection only, no
///   financial-state mutation.
pub fn run_scenario() -> ToolRegistration {
    register_tool!(
        name: "run_scenario",
        cap_weight: 8,
        audit_scope: AuditScope::FullInput { redact: &[] },
        numeric_bounds: NumericBounds::ScenarioInputs,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 19: `compare_scenarios`.
///
/// - **cap_weight**: 5 — compound read across multiple pre-computed
///   scenarios.
/// - **audit_scope**: `Custom("scenario_ids_list")` — log only the
///   ids; full scenario data already in the `projection_snapshots`
///   table.
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false`.
pub fn compare_scenarios() -> ToolRegistration {
    register_tool!(
        name: "compare_scenarios",
        cap_weight: 5,
        audit_scope: AuditScope::Custom("scenario_ids_list"),
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// Static catalog of the seven scenario/alert/activity descriptors
/// in ADR 0020 declaration order. PR-C3.c consumes this alongside
/// the holdings/computation/lifecycle/memory catalogs.
pub fn all() -> [ToolRegistration; 7] {
    [
        add_activity(),
        list_activities(),
        set_reminder(),
        set_alert(),
        get_news(),
        run_scenario(),
        compare_scenarios(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_activity_emits_truth_ledger_and_per_activity_bounds() {
        let r = add_activity();
        assert_eq!(r.name, "add_activity");
        assert_eq!(r.cap_weight.get(), 5);
        assert!(matches!(
            r.audit_scope,
            AuditScope::FullInput { redact } if redact.is_empty()
        ));
        assert_eq!(r.numeric_bounds, NumericBounds::PerActivityType);
        // Critical: financial mutation per CLAUDE.md §0 rule 1
        assert!(r.emits_truth_ledger);
    }

    #[test]
    fn list_activities_is_cheap_read() {
        let r = list_activities();
        assert_eq!(r.name, "list_activities");
        assert_eq!(r.cap_weight.get(), 1);
        assert_eq!(r.audit_scope, AuditScope::FilterAndCount);
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn set_reminder_enforces_two_year_future_horizon() {
        let r = set_reminder();
        assert_eq!(r.name, "set_reminder");
        match r.numeric_bounds {
            NumericBounds::DateFuture { max_horizon_secs } => {
                // 2 years = 60 * 60 * 24 * 365 * 2
                assert_eq!(max_horizon_secs, 63_072_000);
            }
            other => panic!("set_reminder needs DateFuture bounds; got {other:?}"),
        }
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn set_alert_uses_asset_class_threshold() {
        let r = set_alert();
        assert_eq!(r.name, "set_alert");
        assert_eq!(r.numeric_bounds, NumericBounds::AssetClassThreshold);
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn get_news_is_read_only() {
        let r = get_news();
        assert_eq!(r.name, "get_news");
        assert_eq!(r.cap_weight.get(), 1);
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn run_scenario_is_heaviest_tier() {
        let r = run_scenario();
        assert_eq!(r.name, "run_scenario");
        assert_eq!(r.cap_weight.get(), 8);
        assert_eq!(r.numeric_bounds, NumericBounds::ScenarioInputs);
        // Projection only — no financial mutation
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn compare_scenarios_logs_only_ids() {
        let r = compare_scenarios();
        assert_eq!(r.name, "compare_scenarios");
        assert_eq!(r.audit_scope, AuditScope::Custom("scenario_ids_list"));
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn only_add_activity_emits_truth_ledger() {
        // Critical invariant: in this catalog of 7 tools, ONLY
        // add_activity may emit Truth Ledger. A future PR that
        // flips run_scenario or set_alert to ledger=true is a bug
        // (those don't mutate financial state).
        let registrations = all();
        let ledger_emitters: Vec<&str> = registrations
            .iter()
            .filter(|r| r.emits_truth_ledger)
            .map(|r| r.name.as_ref())
            .collect();
        assert_eq!(ledger_emitters, vec!["add_activity"]);
    }

    #[test]
    fn all_seven_cap_weights_within_adr_0020_range() {
        for reg in all().iter() {
            let w = reg.cap_weight.get();
            assert!(
                (1..=8).contains(&w),
                "{} cap_weight={w} outside ADR 0020 range [1, 8]",
                reg.name
            );
        }
    }

    #[test]
    fn all_returns_seven_distinct_tool_names() {
        let registrations = all();
        let names: Vec<&str> = registrations.iter().map(|r| r.name.as_ref()).collect();
        assert_eq!(
            names,
            vec![
                "add_activity",
                "list_activities",
                "set_reminder",
                "set_alert",
                "get_news",
                "run_scenario",
                "compare_scenarios",
            ]
        );
    }

    #[test]
    fn run_scenario_carries_heaviest_cap_in_this_catalog() {
        // Sanity: run_scenario should be the most expensive in this
        // group because Monte Carlo simulation is non-trivial. A
        // future PR that bumps anything else above 8 here is a bug.
        let max_cap = all().iter().map(|r| r.cap_weight.get()).max().unwrap_or(0);
        assert_eq!(max_cap, 8);
        assert_eq!(run_scenario().cap_weight.get(), max_cap);
    }
}
