//! Financial-computation tool safety descriptors —
//! Track C PR-C4.b / ADR 0020 rows 6-9 + 16.
//!
//! These are the agent's READ-ONLY financial computation tools. None
//! emit Truth Ledger (read-only by definition), all have cap weight 1
//! (cheap reads), and most have no numeric inputs (the dispatcher
//! still threads them through the registry so per-turn cap accounting
//! is correct).
//!
//! The five tools registered here are the §23 scene's read surface:
//!
//!   - `compute_net_worth` — answers *"what is my net worth this week"*
//!   - `get_holding_history` — the timeline behind any holding's chip
//!   - `get_market_data` — backs the Sukuks panel bar chart
//!   - `get_fx_rate` — the agent's FX answer surface
//!     (returns `None` rather than inventing a rate per
//!     CLAUDE.md §0 rule 2 + working-agreement §0 rule 2)
//!   - `bond_analytics` — duration/convexity/YTM for the Emaar Sukuk
//!
//! Per Goal v3 §III step 5 "minimum viable change" — descriptors first,
//! PR-C4.b.1..b.5 land the per-tool `rig::tool::Tool` impls.

use crate::tool_registry::{register_tool, AuditScope, NumericBounds, ToolRegistration};

/// ADR 0020 row 6: `compute_net_worth`.
///
/// **cap_weight**: 2 — compound read (sums across all holdings + FX).
/// **audit_scope**: `Custom("base_currency")` — log only the currency.
/// **numeric_bounds**: `None` — no caller-supplied numeric inputs.
/// **emits_truth_ledger**: `false` — read-only computation.
pub fn compute_net_worth() -> ToolRegistration {
    register_tool!(
        name: "compute_net_worth",
        cap_weight: 2,
        audit_scope: AuditScope::Custom("base_currency"),
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 7: `get_holding_history`.
///
/// **cap_weight**: 1 — single-holding history read.
/// **audit_scope**: `IdAndDateRange` — log holding id + window.
/// **numeric_bounds**: `None`.
/// **emits_truth_ledger**: `false` — read-only.
pub fn get_holding_history() -> ToolRegistration {
    register_tool!(
        name: "get_holding_history",
        cap_weight: 1,
        audit_scope: AuditScope::IdAndDateRange,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 8: `get_market_data`.
///
/// **cap_weight**: 1 — symbol + window read against Twelve Data.
/// **audit_scope**: `SymbolAndDateRange`.
/// **numeric_bounds**: `None`.
/// **emits_truth_ledger**: `false`.
pub fn get_market_data() -> ToolRegistration {
    register_tool!(
        name: "get_market_data",
        cap_weight: 1,
        audit_scope: AuditScope::SymbolAndDateRange,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 9: `get_fx_rate`.
///
/// **cap_weight**: 1 — single pair + as-of-date lookup.
/// **audit_scope**: `Pair` — log the FX pair + as-of-date.
/// **numeric_bounds**: `None`.
/// **emits_truth_ledger**: `false`.
///
/// **Critical invariant**: the handler MUST return `None` for missing
/// rates, never a 1.0 fallback. CLAUDE.md §0 rule 2 + working-agreement
/// §0 rule 2 + the CI lint shipped in PR-H8 + the `f64`-in-money-paths
/// lint shipped in PR-H8.b enforce this structurally. The safety
/// descriptor is `false` for `emits_truth_ledger` because the tool
/// reads; the no-fallback rule is enforced inside the handler.
pub fn get_fx_rate() -> ToolRegistration {
    register_tool!(
        name: "get_fx_rate",
        cap_weight: 1,
        audit_scope: AuditScope::Pair,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 16: `bond_analytics`.
///
/// - **cap_weight**: 3 — non-trivial compute (duration / convexity / YTM).
/// - **audit_scope**: `Custom("bond_id_and_computation")` — log the bond
///   + which analytic was requested.
/// - **numeric_bounds**: `BondAsOf` — `as_of_date ∈ (issuance, maturity]`
///   per ADR 0020. Dispatcher enforces.
/// - **emits_truth_ledger**: `false`.
pub fn bond_analytics() -> ToolRegistration {
    register_tool!(
        name: "bond_analytics",
        cap_weight: 3,
        audit_scope: AuditScope::Custom("bond_id_and_computation"),
        numeric_bounds: NumericBounds::BondAsOf,
        emits_truth_ledger: false,
    )
}

/// Static catalog of the five financial-computation descriptors in
/// ADR 0020 declaration order. PR-C3.c consumes this in the dispatcher
/// boot path alongside `tools::holding_safety::all()`.
pub fn all() -> [ToolRegistration; 5] {
    [
        compute_net_worth(),
        get_holding_history(),
        get_market_data(),
        get_fx_rate(),
        bond_analytics(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_net_worth_is_cheap_read_with_currency_logged() {
        let r = compute_net_worth();
        assert_eq!(r.name, "compute_net_worth");
        assert_eq!(r.cap_weight.get(), 2);
        assert_eq!(r.audit_scope, AuditScope::Custom("base_currency"));
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn get_holding_history_logs_id_and_window() {
        let r = get_holding_history();
        assert_eq!(r.name, "get_holding_history");
        assert_eq!(r.cap_weight.get(), 1);
        assert_eq!(r.audit_scope, AuditScope::IdAndDateRange);
        assert_eq!(r.numeric_bounds, NumericBounds::None);
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn get_market_data_logs_symbol_and_window() {
        let r = get_market_data();
        assert_eq!(r.name, "get_market_data");
        assert_eq!(r.audit_scope, AuditScope::SymbolAndDateRange);
    }

    #[test]
    fn get_fx_rate_audit_scope_is_pair() {
        let r = get_fx_rate();
        assert_eq!(r.name, "get_fx_rate");
        assert_eq!(r.audit_scope, AuditScope::Pair);
        // Inline invariant comment for future grep: the handler must
        // return None on missing rate, never a 1.0 fallback.
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn bond_analytics_enforces_bond_as_of_bounds() {
        let r = bond_analytics();
        assert_eq!(r.name, "bond_analytics");
        assert_eq!(r.cap_weight.get(), 3);
        assert_eq!(r.numeric_bounds, NumericBounds::BondAsOf);
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn all_five_are_read_only() {
        // Read-only invariant: none of these tools mutate state. If a
        // future PR flips one of these to `emits_truth_ledger: true`,
        // the test fails AND a code reviewer should ask whether it
        // belongs in computation_safety or holding_safety instead.
        for reg in all().iter() {
            assert!(
                !reg.emits_truth_ledger,
                "{} is read-only per ADR 0020; flipping requires an ADR amendment",
                reg.name
            );
        }
    }

    #[test]
    fn all_five_cap_weights_within_adr_0020_range() {
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
    fn all_returns_five_distinct_tool_names() {
        let registrations = all();
        let names: Vec<&str> = registrations.iter().map(|r| r.name.as_ref()).collect();
        assert_eq!(
            names,
            vec![
                "compute_net_worth",
                "get_holding_history",
                "get_market_data",
                "get_fx_rate",
                "bond_analytics",
            ]
        );
    }
}
