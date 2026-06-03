//! Holdings tool safety descriptors — Track C PR-C4.a / ADR 0020.
//!
//! Per ADR 0020 §"Tool inventory + safety properties" rows 1–3, the
//! three holdings-mutation tools land with these AI Safety Runtime
//! property bundles. This module is the FIRST consumer of the
//! `register_tool!` macro shipped in PR-C3.b (#102) — it locks the
//! safety contract for the holdings handlers per ADR 0020, before
//! PR-C4.a.1/.2/.3 land the full `rig::tool::Tool` implementations.
//!
//! Why ship the descriptors before the handlers:
//!
//!   1. Audit Finding 11.1 (issue #51) closes only when the
//!      compliance matrix is **mechanically enforced**. The macro
//!      enforces it at registration time; this module exercises that
//!      machinery for the three highest-risk tools the agent needs
//!      to mutate financial state.
//!   2. PR-C3.c (the dispatcher consumer of the registry) needs a
//!      stable catalog to plumb through. Shipping the descriptors
//!      first means C3.c lands without needing to coordinate with
//!      C4.a.1/.2/.3.
//!   3. Per Goal v3 §III step 5 "minimum viable change" — descriptors
//!      are the smallest unit of §23 progress that's independently
//!      testable + shippable.
//!
//! Handler implementations follow per ADR 0020 row in:
//!
//!   - PR-C4.a.1: `create_holding.rs` — produces a draft for user
//!     confirmation per the `create_account` pattern (tool never
//!     writes directly; service writes on confirm via the existing
//!     holdings pipeline)
//!   - PR-C4.a.2: `update_holding.rs` — diff-based update draft
//!   - PR-C4.a.3: `delete_holding.rs` — confirmation prompt
//!
//! Each handler PR exercises `debug_assert_truth_ledger_contract`
//! against its concrete handler — the runtime guard catches the
//! "declared `emits_truth_ledger: true` but handler is a no-op"
//! mismatch at registry boot (PR-C3.c).

use crate::tool_registry::{register_tool, AuditScope, NumericBounds, ToolRegistration};

/// ADR 0020 row 1: `create_holding`.
///
/// **cap_weight**: 5 — standard mutation budget per ADR 0020.
/// **audit_scope**: `FullInput { redact: [] }` — every arg is
///   non-sensitive (no Plaid tokens or PII pass through this tool).
/// **numeric_bounds**: `Holding { qty_max: 1e9, cost_basis_max: 1e9 }`
///   — defence-in-depth against prompt-injection driving
///   `quantity = 10^15`. Dispatcher converts to Decimal at compare.
/// **emits_truth_ledger**: `true` — every new holding emits the
///   `HoldingCreated` chain entry; handler in PR-C4.a.1 must satisfy
///   `debug_assert_truth_ledger_contract`.
pub fn create_holding() -> ToolRegistration {
    register_tool!(
        name: "create_holding",
        cap_weight: 5,
        audit_scope: AuditScope::FullInput { redact: &[] },
        numeric_bounds: NumericBounds::Holding {
            qty_max: 1.0e9,
            cost_basis_max: 1.0e9,
        },
        emits_truth_ledger: true,
    )
}

/// ADR 0020 row 2: `update_holding`.
///
/// **cap_weight**: 5 — same standard mutation budget as create.
/// **audit_scope**: `Diff` — dispatcher logs the before/after diff
///   only, not the full holding state, to keep audit log size
///   bounded under repeated updates.
/// **numeric_bounds**: same Holding {1e9, 1e9}.
/// **emits_truth_ledger**: `true` — every update emits the
///   `HoldingUpdated` chain entry.
pub fn update_holding() -> ToolRegistration {
    register_tool!(
        name: "update_holding",
        cap_weight: 5,
        audit_scope: AuditScope::Diff,
        numeric_bounds: NumericBounds::Holding {
            qty_max: 1.0e9,
            cost_basis_max: 1.0e9,
        },
        emits_truth_ledger: true,
    )
}

/// ADR 0020 row 3: `delete_holding`.
///
/// **cap_weight**: 3 — cheaper than create/update (no numeric
///   inputs to validate; just an id lookup).
/// **audit_scope**: `IdOnly` — log just the holding id.
/// **numeric_bounds**: `None` — no numeric inputs.
/// **emits_truth_ledger**: `true` — deletion emits the
///   `HoldingDeleted` chain entry so the audit trail can prove the
///   holding existed before deletion.
pub fn delete_holding() -> ToolRegistration {
    register_tool!(
        name: "delete_holding",
        cap_weight: 3,
        audit_scope: AuditScope::IdOnly,
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: true,
    )
}

/// Static catalog of the three holdings safety descriptors in ADR
/// 0020 declaration order. PR-C3.c consumes this in the dispatcher
/// boot path.
pub fn all() -> [ToolRegistration; 3] {
    [create_holding(), update_holding(), delete_holding()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_holding_matches_adr_0020_row_1() {
        let r = create_holding();
        assert_eq!(r.name, "create_holding");
        assert_eq!(r.cap_weight.get(), 5);
        assert!(matches!(r.audit_scope, AuditScope::FullInput { redact } if redact.is_empty()));
        assert!(matches!(
            r.numeric_bounds,
            NumericBounds::Holding {
                qty_max: q,
                cost_basis_max: c,
            } if (q - 1.0e9).abs() < f64::EPSILON && (c - 1.0e9).abs() < f64::EPSILON
        ));
        assert!(r.emits_truth_ledger);
    }

    #[test]
    fn update_holding_uses_diff_audit_scope() {
        // Diff is the right shape for updates — full state would
        // bloat audit log size on repeated edits per ADR 0020 §"Tool
        // inventory" row 2 rationale.
        let r = update_holding();
        assert_eq!(r.name, "update_holding");
        assert_eq!(r.cap_weight.get(), 5);
        assert_eq!(r.audit_scope, AuditScope::Diff);
        assert!(r.emits_truth_ledger);
    }

    #[test]
    fn delete_holding_has_cap_three_and_no_bounds() {
        let r = delete_holding();
        assert_eq!(r.name, "delete_holding");
        assert_eq!(r.cap_weight.get(), 3);
        assert_eq!(r.audit_scope, AuditScope::IdOnly);
        assert_eq!(r.numeric_bounds, NumericBounds::None);
        assert!(r.emits_truth_ledger);
    }

    #[test]
    fn all_three_descriptors_emit_truth_ledger() {
        // All three holdings-mutation tools must emit Truth Ledger
        // entries per CLAUDE.md §0 rule 1 — the Truth Ledger is the
        // immutable record of financial mutations. A future "create-
        // but-don't-ledger" attempt fails here.
        for reg in all().iter() {
            assert!(
                reg.emits_truth_ledger,
                "{} must emit Truth Ledger per CLAUDE.md §0 rule 1",
                reg.name
            );
        }
    }

    #[test]
    fn all_three_cap_weights_within_adr_0020_range() {
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
    fn all_returns_three_distinct_tool_names() {
        let registrations = all();
        let names: Vec<&str> = registrations.iter().map(|r| r.name.as_ref()).collect();
        assert_eq!(
            names,
            vec!["create_holding", "update_holding", "delete_holding"]
        );
    }

    #[test]
    fn create_and_update_share_holding_numeric_bounds_envelope() {
        // ADR 0020 rows 1+2 share the same Holding{1e9, 1e9} envelope;
        // a future bump to one without the other is a likely bug —
        // this test pins them together.
        let c = create_holding();
        let u = update_holding();
        assert_eq!(c.numeric_bounds, u.numeric_bounds);
    }
}
