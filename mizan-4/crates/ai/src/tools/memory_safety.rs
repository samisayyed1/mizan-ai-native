//! Memory tool safety descriptors —
//! Track C PR-C4.d / ADR 0020 rows 20-21.
//!
//! User-memory tools are the agent's persistent-context surface. They
//! NEVER emit Truth Ledger: memory edits aren't financial mutations,
//! and per ADR 0020 §"Memory + tooling boundaries" rationale the
//! user-memory layer has its own GDPR-rectifiable surface that
//! doubles as its audit trail.
//!
//! Per ADR 0020 §"Memory + tooling boundaries":
//!
//!   - row 20: `get_user_memory` — cap 1 (cheap fact-keys read),
//!     audit `Custom("fact_keys_list")`, bounds `None`, ledger
//!     `false`
//!   - row 21: `update_user_memory` — cap 3 (write a fact diff),
//!     audit `Diff` (only the changed fields), bounds `PerFactType`
//!     (e.g. `risk_tolerance ∈ [0, 100]`), ledger `false`
//!
//! Critical contract: per ADR 0020 + Working-agreement §C, **only
//! these two tools may write to `user_memory`.** A future tool that
//! writes memory as a side effect bypasses the memory-writer
//! subroutine and is a bug. The runtime guard in PR-C3.c will
//! enforce by checking handler signatures against a memory-writer
//! capability marker.

use crate::tool_registry::{register_tool, AuditScope, NumericBounds, ToolRegistration};

/// ADR 0020 row 20: `get_user_memory`.
///
/// - **cap_weight**: 1 — cheap key-set read against the memory store.
/// - **audit_scope**: `Custom("fact_keys_list")` — log which keys
///   the agent asked for, not the values (PII reduction).
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false` — read-only.
pub fn get_user_memory() -> ToolRegistration {
    register_tool!(
        name: "get_user_memory",
        cap_weight: 1,
        audit_scope: AuditScope::Custom("fact_keys_list"),
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 21: `update_user_memory`.
///
/// - **cap_weight**: 3 — write to memory store; same weight as
///   `delete_holding` per ADR 0020 ("cheap mutation or compound read").
/// - **audit_scope**: `Diff` — log the before/after on the changed
///   facts only (audit-log size grows linearly with edits, not
///   with full memory state).
/// - **numeric_bounds**: `PerFactType` — dispatcher consults the
///   per-fact-type bound table (e.g. `risk_tolerance ∈ [0, 100]`).
/// - **emits_truth_ledger**: `false` — per ADR 0020 §"Memory +
///   tooling boundaries" memory edits are GDPR-rectifiable through
///   the editor UI, which doubles as the audit trail; the Truth
///   Ledger is reserved for financial mutations.
pub fn update_user_memory() -> ToolRegistration {
    register_tool!(
        name: "update_user_memory",
        cap_weight: 3,
        audit_scope: AuditScope::Diff,
        numeric_bounds: NumericBounds::PerFactType,
        emits_truth_ledger: false,
    )
}

/// Static catalog of the two memory descriptors in declaration order.
/// PR-C3.c consumes this alongside the holdings/computation/lifecycle
/// catalogs in the dispatcher boot path.
pub fn all() -> [ToolRegistration; 2] {
    [get_user_memory(), update_user_memory()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_user_memory_logs_keys_only_not_values() {
        let r = get_user_memory();
        assert_eq!(r.name, "get_user_memory");
        assert_eq!(r.cap_weight.get(), 1);
        assert_eq!(r.audit_scope, AuditScope::Custom("fact_keys_list"));
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn update_user_memory_uses_diff_audit_and_per_fact_bounds() {
        let r = update_user_memory();
        assert_eq!(r.name, "update_user_memory");
        assert_eq!(r.cap_weight.get(), 3);
        assert_eq!(r.audit_scope, AuditScope::Diff);
        assert_eq!(r.numeric_bounds, NumericBounds::PerFactType);
        // Critical invariant per ADR 0020 §"Memory + tooling
        // boundaries": memory edits are NOT Truth Ledger material.
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn neither_memory_tool_emits_truth_ledger() {
        // The Truth Ledger is reserved for financial mutations
        // (CLAUDE.md §0 rule 1). Memory edits go through the
        // GDPR-rectifiable editor UI; promoting either of these to
        // ledger=true requires an ADR amendment + a fundamentally
        // different audit shape.
        for reg in all().iter() {
            assert!(
                !reg.emits_truth_ledger,
                "{} must not emit Truth Ledger per ADR 0020 memory boundary",
                reg.name
            );
        }
    }

    #[test]
    fn all_two_cap_weights_within_adr_0020_range() {
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
    fn all_returns_two_distinct_tool_names() {
        let registrations = all();
        let names: Vec<&str> = registrations.iter().map(|r| r.name.as_ref()).collect();
        assert_eq!(names, vec!["get_user_memory", "update_user_memory"]);
    }

    #[test]
    fn write_tool_has_higher_cap_weight_than_read() {
        // Sanity: writes cost more than reads. If a future PR flips
        // the relative ordering, this test fires and the reviewer
        // should ask whether the change is intentional vs typo.
        assert!(update_user_memory().cap_weight.get() > get_user_memory().cap_weight.get());
    }
}
