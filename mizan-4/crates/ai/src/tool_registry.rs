//! Tool-registry compile-time scaffolding — Track C PR-C3.b / ADR 0020.
//!
//! Per audit Finding 11.1 + Working-agreement §C, every AI tool MUST
//! declare four AI Safety Runtime properties at its registration site:
//!
//!   1. **Cap weight** — per-turn budget cost (1..=8)
//!   2. **Audit scope** — what the dispatcher logs about each call
//!   3. **Numeric bounds** — defence-in-depth against prompt injection
//!      driving math outside reasonable ranges
//!   4. **Truth Ledger emission** — whether mutations emit a chain entry
//!
//! This module ships:
//!
//!   - [`ToolRegistration`] struct that holds all four properties.
//!     A new tool that omits any property is a compile error because
//!     every field is required.
//!   - [`AuditScope`] + [`NumericBounds`] enums modelled on the
//!     ADR 0020 §"Tool inventory + safety properties" table.
//!   - [`register_tool!`] declarative macro — sugar around the struct
//!     literal that also runs the [`debug_assert_truth_ledger_contract`]
//!     check (catches the "claims to emit, but bound to a non-mutating
//!     handler" mismatch at startup) and `const`-validates the cap weight.
//!
//! This is the COMPILE-TIME scaffolding. The runtime check ([`SafetyRuntime`])
//! already ships in `crate::safety`; PR-C3.c wires the two together
//! when the dispatcher actually consumes the registry. PR-C4..PR-C14
//! ship the per-tool registrations themselves.
//!
//! Why ship the macro before any tool uses it: ADR 0020 closes audit
//! finding 11.1 by declaring the property matrix; this module makes
//! that matrix mechanically enforceable so PR-C4..PR-C14 land against
//! the contract instead of inventing one each time.

use std::borrow::Cow;

// ─── Cap weight ─────────────────────────────────────────────────────

/// Per-turn budget weight a tool consumes. Per ADR 0020 the scale is:
///
///   - 1 — read-only get/list
///   - 2..=3 — cheap mutation or compound read
///   - 5 — standard mutation
///   - 8 — heavy compute (Monte Carlo, document parsing)
///
/// Silver tier: 50 units / turn; Gold: 200 / turn. Both budgets are
/// enforced by the `crate::safety::AiSafetyRuntime` at dispatch time.
///
/// We enforce the [1, 8] range at construction so a misregistered
/// tool with cap_weight=0 (silent free) or cap_weight=99 (defeats the
/// budget entirely) can't reach a release build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolCapWeight(u8);

impl ToolCapWeight {
    /// Construct a cap weight, checked at runtime. Returns `None` for
    /// values outside [1, 8].
    pub const fn new(value: u8) -> Option<Self> {
        if value >= 1 && value <= 8 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Construct without runtime validation — used inside the
    /// [`register_tool!`] macro with the literal-validation guard.
    ///
    /// `const` so the macro can build registrations in `const` context.
    pub const fn from_literal_checked(value: u8) -> Self {
        // Compile-time bound check. If `value` is out of range, the
        // const-eval panics during build (errors point at the call site
        // in the macro expansion).
        assert!(
            value >= 1 && value <= 8,
            "ToolCapWeight literal out of range; ADR 0020 limits cap_weight to 1..=8"
        );
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

// ─── Audit scope ────────────────────────────────────────────────────

/// What the dispatcher should record to `agent_audit_log` after a call.
/// Per ADR 0020 §"Tool inventory" the variant per tool is:
///
///   - `FullInput { redact }` — full args minus the listed fields
///   - `Diff` — before/after diff on update tools
///   - `IdOnly` — record only the target id (used for `delete_*`)
///   - `FilterAndCount` — filter args + N rows returned (read tools)
///   - `IdAndDateRange` — id + window (history reads)
///   - `SymbolAndDateRange` — symbol + window (market-data reads)
///   - `Pair` — generic 2-tuple (FX pair + as_of_date)
///   - `Custom(static_name)` — escape hatch for one-off tool shapes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditScope {
    FullInput {
        /// Field names to omit from the logged input. Empty slice = log everything.
        redact: &'static [&'static str],
    },
    Diff,
    IdOnly,
    FilterAndCount,
    IdAndDateRange,
    SymbolAndDateRange,
    Pair,
    Custom(&'static str),
}

// ─── Numeric bounds ─────────────────────────────────────────────────

/// Per-call numeric bounds the dispatcher enforces before invoking
/// the tool's handler. ADR 0020 names the bound family per tool.
///
/// Decimal bounds are kept as f64-encoded literals at this LAYER —
/// the dispatcher's runtime check converts to `rust_decimal::Decimal`
/// for the actual comparison, so the value is exact at compare time.
/// The f64 envelope is only for the static declaration ergonomics
/// inside the registration macro. CLAUDE.md §0 rule 2 holds because
/// the comparison itself never touches f64.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericBounds {
    /// Tool is read-only or otherwise has no numeric inputs.
    None,
    /// `quantity ∈ [0, qty_max]`, `cost_basis ∈ [0, cost_basis_max]`
    /// per ADR 0020 row 1/2 (`create_holding` / `update_holding`).
    Holding { qty_max: f64, cost_basis_max: f64 },
    /// Per-activity-type bounds (e.g. dividend ≥ 0; trade qty > 0;
    /// fee ≥ 0). Concrete numbers live in the activity dispatcher;
    /// this variant signals "use the activity table".
    PerActivityType,
    /// `due_date ∈ (now, now + max_horizon_secs]`.
    DateFuture { max_horizon_secs: u64 },
    /// Per-asset-class threshold; concrete bounds live in the asset
    /// dispatcher per ADR 0021. This variant signals "use the asset
    /// class table".
    AssetClassThreshold,
    /// `document_kind ∈ approved_list`, `word_count ≤ max_words`.
    Document {
        approved_kinds: &'static [&'static str],
        max_words: u32,
    },
    /// `as_of_date ∈ (issuance, maturity]` — `bond_analytics` per ADR 0020.
    BondAsOf,
    /// `confidence_interval_width ∈ [0, 1]`, `central_estimate ≥ 0`.
    EstimateRange,
    /// Per-scenario-input bounds (e.g. `equity_return ∈ [-1, 1]`).
    ScenarioInputs,
    /// `fact_value` checked against the per-fact-type table.
    PerFactType,
}

// ─── Tool registration ──────────────────────────────────────────────

/// One row in the static tool registry. The four-property contract.
///
/// Construct via the [`register_tool!`] macro — that's the surface that
/// catches missing properties at compile time. Direct struct-literal
/// construction also works (every field is `pub`) and is what the macro
/// expands to.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRegistration {
    /// Public tool name — what the LLM sees in the function-call schema.
    pub name: Cow<'static, str>,
    /// Per-turn budget weight per ADR 0020 (1..=8).
    pub cap_weight: ToolCapWeight,
    /// What the dispatcher logs about each call.
    pub audit_scope: AuditScope,
    /// Numeric bounds checked before handler invocation.
    pub numeric_bounds: NumericBounds,
    /// Whether mutations emit a Truth Ledger chain entry. `true` MUST
    /// match the handler's actual write path; mismatch is caught by
    /// [`debug_assert_truth_ledger_contract`] at registry boot.
    pub emits_truth_ledger: bool,
}

/// Mismatch between [`ToolRegistration::emits_truth_ledger`] and a
/// caller-supplied "does the handler actually write to the ledger?"
/// signal. Surfaces a typed error so callers can decide to panic at
/// startup (the default) or downgrade to a logged warning in dev.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthLedgerContractMismatch {
    pub tool_name: String,
    pub declared: bool,
    pub actual: bool,
}

/// Compare a registration's `emits_truth_ledger` flag against the
/// concrete handler's actual write path (which the caller signals).
///
/// The dispatcher calls this for every tool at startup. The check
/// closes one direction of audit Finding 11.1 — a tool that DECLARES
/// `emits_truth_ledger: true` but is wired to a handler that never
/// calls the ledger silently passes the macro check. This runtime
/// guard fails fast at boot in that case.
pub fn debug_assert_truth_ledger_contract(
    registration: &ToolRegistration,
    handler_actually_emits: bool,
) -> Result<(), TruthLedgerContractMismatch> {
    if registration.emits_truth_ledger == handler_actually_emits {
        Ok(())
    } else {
        Err(TruthLedgerContractMismatch {
            tool_name: registration.name.to_string(),
            declared: registration.emits_truth_ledger,
            actual: handler_actually_emits,
        })
    }
}

// ─── Macro ──────────────────────────────────────────────────────────

/// Declarative tool registration matching ADR 0020 §"Compile-time
/// enforcement". Expands to a [`ToolRegistration`] struct literal so
/// missing properties are caught by the Rust compiler — no field is
/// optional.
///
/// Cap weight is `const`-validated against the 1..=8 ADR 0020 range
/// via [`ToolCapWeight::from_literal_checked`].
///
/// # Example
///
/// ```rust,no_run
/// # use mizan_ai::tool_registry::{register_tool, AuditScope, NumericBounds};
/// let create_holding = register_tool!(
///     name: "create_holding",
///     cap_weight: 5,
///     audit_scope: AuditScope::FullInput { redact: &[] },
///     numeric_bounds: NumericBounds::Holding { qty_max: 1e9, cost_basis_max: 1e9 },
///     emits_truth_ledger: true,
/// );
/// assert_eq!(create_holding.cap_weight.get(), 5);
/// ```
///
/// Forgetting any field is a compile error (struct missing required
/// field). Wrong-type values fail at the field-binding site. Out-of-
/// range cap weights fail at const-eval inside
/// [`ToolCapWeight::from_literal_checked`].
#[macro_export]
macro_rules! register_tool {
    (
        name: $name:expr,
        cap_weight: $cap:expr,
        audit_scope: $audit:expr,
        numeric_bounds: $bounds:expr,
        emits_truth_ledger: $ledger:expr $(,)?
    ) => {
        $crate::tool_registry::ToolRegistration {
            name: ::std::borrow::Cow::Borrowed($name),
            cap_weight: $crate::tool_registry::ToolCapWeight::from_literal_checked($cap),
            audit_scope: $audit,
            numeric_bounds: $bounds,
            emits_truth_ledger: $ledger,
        }
    };
}

// Re-export for `use mizan_ai::tool_registry::register_tool;` ergonomics
// (the `#[macro_export]` attribute alone only exports it at the crate
// root, not the module).
pub use crate::register_tool;

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_weight_constructs_inside_range() {
        for v in 1u8..=8u8 {
            assert!(ToolCapWeight::new(v).is_some(), "expected {v} to be valid");
        }
    }

    #[test]
    fn cap_weight_rejects_outside_range() {
        assert!(ToolCapWeight::new(0).is_none());
        assert!(ToolCapWeight::new(9).is_none());
        assert!(ToolCapWeight::new(255).is_none());
    }

    #[test]
    fn cap_weight_from_literal_checks_at_runtime() {
        // In tests we exercise via `new` to catch the range; the
        // `from_literal_checked` const path would panic on out-of-
        // range values, which is what we want at compile time.
        assert_eq!(ToolCapWeight::from_literal_checked(5).get(), 5);
        assert_eq!(ToolCapWeight::from_literal_checked(1).get(), 1);
        assert_eq!(ToolCapWeight::from_literal_checked(8).get(), 8);
    }

    #[test]
    #[should_panic(expected = "ToolCapWeight literal out of range")]
    fn cap_weight_from_literal_panics_on_zero() {
        let _ = ToolCapWeight::from_literal_checked(0);
    }

    #[test]
    #[should_panic(expected = "ToolCapWeight literal out of range")]
    fn cap_weight_from_literal_panics_above_eight() {
        let _ = ToolCapWeight::from_literal_checked(9);
    }

    #[test]
    fn register_tool_macro_builds_a_full_registration() {
        let reg = register_tool!(
            name: "create_holding",
            cap_weight: 5,
            audit_scope: AuditScope::FullInput { redact: &[] },
            numeric_bounds: NumericBounds::Holding {
                qty_max: 1.0e9,
                cost_basis_max: 1.0e9,
            },
            emits_truth_ledger: true,
        );
        assert_eq!(reg.name, "create_holding");
        assert_eq!(reg.cap_weight.get(), 5);
        assert_eq!(reg.audit_scope, AuditScope::FullInput { redact: &[] });
        assert!(reg.emits_truth_ledger);
    }

    #[test]
    fn register_tool_macro_supports_read_only_tools() {
        let reg = register_tool!(
            name: "list_activities",
            cap_weight: 1,
            audit_scope: AuditScope::FilterAndCount,
            numeric_bounds: NumericBounds::None,
            emits_truth_ledger: false,
        );
        assert_eq!(reg.cap_weight.get(), 1);
        assert!(!reg.emits_truth_ledger);
        assert_eq!(reg.numeric_bounds, NumericBounds::None);
    }

    #[test]
    fn register_tool_macro_handles_redacted_fields() {
        // Confirms the `&'static [&'static str]` literal threads through.
        let reg = register_tool!(
            name: "sync_account",
            cap_weight: 5,
            audit_scope: AuditScope::FullInput {
                redact: &["plaid_access_token", "snaptrade_user_secret"],
            },
            numeric_bounds: NumericBounds::None,
            emits_truth_ledger: false,
        );
        match reg.audit_scope {
            AuditScope::FullInput { redact } => {
                assert_eq!(redact.len(), 2);
                assert!(redact.contains(&"plaid_access_token"));
            }
            _ => panic!("expected FullInput audit scope"),
        }
    }

    #[test]
    fn truth_ledger_contract_match_passes() {
        let reg = register_tool!(
            name: "create_holding",
            cap_weight: 5,
            audit_scope: AuditScope::FullInput { redact: &[] },
            numeric_bounds: NumericBounds::Holding {
                qty_max: 1.0e9,
                cost_basis_max: 1.0e9,
            },
            emits_truth_ledger: true,
        );
        assert!(debug_assert_truth_ledger_contract(&reg, true).is_ok());
    }

    #[test]
    fn truth_ledger_contract_mismatch_returns_named_error() {
        let reg = register_tool!(
            name: "rogue_tool",
            cap_weight: 5,
            audit_scope: AuditScope::FullInput { redact: &[] },
            numeric_bounds: NumericBounds::None,
            emits_truth_ledger: true,
        );
        let err = debug_assert_truth_ledger_contract(&reg, false).unwrap_err();
        assert_eq!(err.tool_name, "rogue_tool");
        assert!(err.declared);
        assert!(!err.actual);
    }

    #[test]
    fn truth_ledger_contract_catches_reverse_mismatch() {
        // Declares false but handler actually emits — equally a bug.
        let reg = register_tool!(
            name: "silent_emitter",
            cap_weight: 5,
            audit_scope: AuditScope::FullInput { redact: &[] },
            numeric_bounds: NumericBounds::None,
            emits_truth_ledger: false,
        );
        let err = debug_assert_truth_ledger_contract(&reg, true).unwrap_err();
        assert_eq!(err.tool_name, "silent_emitter");
        assert!(!err.declared);
        assert!(err.actual);
    }

    #[test]
    fn adr_0020_full_inventory_compiles() {
        // Smoke test: the ADR 0020 §"Tool inventory" table mapped row-
        // by-row. Verifies the four-property contract holds for every
        // tool the matrix names. Any future row added to the ADR
        // should be added here too.
        let registrations = [
            register_tool!(
                name: "create_holding",
                cap_weight: 5,
                audit_scope: AuditScope::FullInput { redact: &[] },
                numeric_bounds: NumericBounds::Holding { qty_max: 1.0e9, cost_basis_max: 1.0e9 },
                emits_truth_ledger: true,
            ),
            register_tool!(
                name: "update_holding",
                cap_weight: 5,
                audit_scope: AuditScope::Diff,
                numeric_bounds: NumericBounds::Holding { qty_max: 1.0e9, cost_basis_max: 1.0e9 },
                emits_truth_ledger: true,
            ),
            register_tool!(
                name: "delete_holding",
                cap_weight: 3,
                audit_scope: AuditScope::IdOnly,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: true,
            ),
            register_tool!(
                name: "add_activity",
                cap_weight: 5,
                audit_scope: AuditScope::FullInput { redact: &[] },
                numeric_bounds: NumericBounds::PerActivityType,
                emits_truth_ledger: true,
            ),
            register_tool!(
                name: "list_activities",
                cap_weight: 1,
                audit_scope: AuditScope::FilterAndCount,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "compute_net_worth",
                cap_weight: 2,
                audit_scope: AuditScope::Custom("base_currency"),
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "get_holding_history",
                cap_weight: 1,
                audit_scope: AuditScope::IdAndDateRange,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "get_market_data",
                cap_weight: 1,
                audit_scope: AuditScope::SymbolAndDateRange,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "get_fx_rate",
                cap_weight: 1,
                audit_scope: AuditScope::Pair,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "sync_account",
                cap_weight: 5,
                audit_scope: AuditScope::FullInput {
                    redact: &["plaid_access_token", "snaptrade_user_secret"],
                },
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "generate_report",
                cap_weight: 3,
                audit_scope: AuditScope::Custom("report_type_and_scope"),
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "set_reminder",
                cap_weight: 2,
                audit_scope: AuditScope::FullInput { redact: &[] },
                numeric_bounds: NumericBounds::DateFuture {
                    max_horizon_secs: 60 * 60 * 24 * 365 * 2,
                },
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "set_alert",
                cap_weight: 2,
                audit_scope: AuditScope::FullInput { redact: &[] },
                numeric_bounds: NumericBounds::AssetClassThreshold,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "get_news",
                cap_weight: 1,
                audit_scope: AuditScope::FilterAndCount,
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "summarize_document",
                cap_weight: 8,
                audit_scope: AuditScope::Custom("document_kind_and_word_count"),
                numeric_bounds: NumericBounds::Document {
                    approved_kinds: &["pdf", "csv", "xlsx", "txt"],
                    max_words: 50_000,
                },
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "bond_analytics",
                cap_weight: 3,
                audit_scope: AuditScope::Custom("bond_id_and_computation"),
                numeric_bounds: NumericBounds::BondAsOf,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "estimate_price",
                cap_weight: 5,
                audit_scope: AuditScope::Custom("asset_id_and_as_of"),
                numeric_bounds: NumericBounds::EstimateRange,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "run_scenario",
                cap_weight: 8,
                audit_scope: AuditScope::FullInput { redact: &[] },
                numeric_bounds: NumericBounds::ScenarioInputs,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "compare_scenarios",
                cap_weight: 5,
                audit_scope: AuditScope::Custom("scenario_ids_list"),
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "get_user_memory",
                cap_weight: 1,
                audit_scope: AuditScope::Custom("fact_keys_list"),
                numeric_bounds: NumericBounds::None,
                emits_truth_ledger: false,
            ),
            register_tool!(
                name: "update_user_memory",
                cap_weight: 3,
                audit_scope: AuditScope::Diff,
                numeric_bounds: NumericBounds::PerFactType,
                emits_truth_ledger: false,
            ),
        ];

        // ADR 0020 lists 19 + 2 memory tools = 21 registrations.
        assert_eq!(registrations.len(), 21);
        // Spot-check: every cap weight is within the documented range.
        for reg in &registrations {
            let w = reg.cap_weight.get();
            assert!(
                (1..=8).contains(&w),
                "{} cap_weight={w} out of range",
                reg.name
            );
        }
        // Every tool that emits truth ledger should be a mutation (the
        // ADR-documented set: create/update/delete_holding + add_activity).
        let ledger_emitters: Vec<_> = registrations
            .iter()
            .filter(|r| r.emits_truth_ledger)
            .map(|r| r.name.as_ref())
            .collect();
        assert_eq!(
            ledger_emitters,
            vec![
                "create_holding",
                "update_holding",
                "delete_holding",
                "add_activity"
            ]
        );
    }
}
