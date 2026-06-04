//! Lifecycle tool safety descriptors —
//! Track C PR-C4.c / ADR 0020 rows 10-11 + 15 + 17.
//!
//! Lifecycle tools are the "operational" surface — the agent uses
//! them to trigger sync, request reports, fetch Sharia screening,
//! estimate prices for ai-estimated badges, and summarize documents
//! the user uploads. None mutate Truth Ledger directly (sync emits
//! account-level events from its own subsystem; price estimates
//! surface drafts the user explicitly accepts before any write).
//!
//! Per ADR 0020 §"Tool inventory":
//!
//!   - row 10: `sync_account` — cap 5, audit FullInput with token
//!     redaction (Plaid + SnapTrade secrets), bounds None, ledger
//!     false (sync emits at provider layer)
//!   - row 11: `generate_report` — cap 3, audit Custom("report_type_
//!     and_scope"), bounds None, ledger false
//!   - row 15: `summarize_document` — cap 8 (heavy parse), audit
//!     Custom("document_kind_and_word_count"), bounds Document with
//!     approved kinds + max_words, ledger false
//!   - row 17: `estimate_price` — cap 5, audit Custom("asset_id_
//!     and_as_of"), bounds EstimateRange, ledger false (surfaces a
//!     draft; user accepts before write)
//!
//! Plus `find_sharia_status` — the agent's bridge to PR-E4's AAOIFI
//! worker. Not in the original ADR 0020 inventory but added under
//! the same contract per Goal v3 §V step C4.c.

use crate::tool_registry::{register_tool, AuditScope, NumericBounds, ToolRegistration};

/// ADR 0020 row 10: `sync_account`.
///
/// - **cap_weight**: 5 — heavy operation (kicks off provider sync).
/// - **audit_scope**: `FullInput { redact: &["plaid_access_token",
///   "snaptrade_user_secret"] }` — the inputs include provider
///   identifiers; tokens are redacted before write to
///   `agent_audit_log`. CLAUDE.md §0 rule 4 (no plaintext tokens on
///   disk) is enforced both at the cipher layer + here.
/// - **numeric_bounds**: `None` — no money-shaped inputs.
/// - **emits_truth_ledger**: `false` — sync emits an account-level
///   event from its own subsystem (sync run ledger), not the agent
///   dispatcher.
pub fn sync_account() -> ToolRegistration {
    register_tool!(
        name: "sync_account",
        cap_weight: 5,
        audit_scope: AuditScope::FullInput {
            redact: &["plaid_access_token", "snaptrade_user_secret"],
        },
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 11: `generate_report`.
///
/// - **cap_weight**: 3 — moderate compute (compose tabular data).
/// - **audit_scope**: `Custom("report_type_and_scope")` — log the
///   shape of the report + the scope (per-account vs portfolio-wide).
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false` — report contents already
///   audited at generation source.
pub fn generate_report() -> ToolRegistration {
    register_tool!(
        name: "generate_report",
        cap_weight: 3,
        audit_scope: AuditScope::Custom("report_type_and_scope"),
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 15: `summarize_document`.
///
/// - **cap_weight**: 8 — heaviest in the registry; document parsing
///   + summarization is non-trivial.
/// - **audit_scope**: `Custom("document_kind_and_word_count")` — log
///   the kind + length so the audit trail can spot anomalous large
///   uploads.
/// - **numeric_bounds**: `Document { approved_kinds: ["pdf", "csv",
///   "xlsx", "txt"], max_words: 50_000 }` — defence-in-depth against
///   a runaway summarize-the-internet attempt.
/// - **emits_truth_ledger**: `false`.
pub fn summarize_document() -> ToolRegistration {
    register_tool!(
        name: "summarize_document",
        cap_weight: 8,
        audit_scope: AuditScope::Custom("document_kind_and_word_count"),
        numeric_bounds: NumericBounds::Document {
            approved_kinds: &["pdf", "csv", "xlsx", "txt"],
            max_words: 50_000,
        },
        emits_truth_ledger: false,
    )
}

/// ADR 0020 row 17: `estimate_price`.
///
/// - **cap_weight**: 5 — costs a credit per the Spec §7 calibration.
/// - **audit_scope**: `Custom("asset_id_and_as_of")` — log the asset
///   id + the as-of-date.
/// - **numeric_bounds**: `EstimateRange` — `confidence ∈ [0, 1]`,
///   `central_estimate ≥ 0` per ADR 0020.
/// - **emits_truth_ledger**: `false` — the tool **never auto-writes**.
///   It surfaces an `'ai-estimated'`-badged draft (per ADR 0023
///   modifier semantics + ADR 0021 §"AI estimation pipelines") that
///   the user must explicitly accept before any holdings write
///   occurs. The accept path goes through `update_holding` (which
///   DOES emit Truth Ledger).
pub fn estimate_price() -> ToolRegistration {
    register_tool!(
        name: "estimate_price",
        cap_weight: 5,
        audit_scope: AuditScope::Custom("asset_id_and_as_of"),
        numeric_bounds: NumericBounds::EstimateRange,
        emits_truth_ledger: false,
    )
}

/// Bridge tool to PR-E4's AAOIFI screening worker.
///
/// Not in the original ADR 0020 inventory; added under the same
/// safety contract per Goal v3 §V step C4.c so the agent can ask
/// `"is this Sukuk halal-screened?"` via the existing endpoint
/// without inventing a verdict locally.
///
/// - **cap_weight**: 1 — single-symbol lookup against the cloud
///   AAOIFI worker.
/// - **audit_scope**: `Custom("symbol")` — log just the symbol.
/// - **numeric_bounds**: `None`.
/// - **emits_truth_ledger**: `false` — read-only against the
///   screening worker.
pub fn find_sharia_status() -> ToolRegistration {
    register_tool!(
        name: "find_sharia_status",
        cap_weight: 1,
        audit_scope: AuditScope::Custom("symbol"),
        numeric_bounds: NumericBounds::None,
        emits_truth_ledger: false,
    )
}

/// Static catalog of the five lifecycle descriptors in declaration
/// order. PR-C3.c consumes this in the dispatcher boot path alongside
/// the holdings + computation catalogs.
pub fn all() -> [ToolRegistration; 5] {
    [
        sync_account(),
        generate_report(),
        summarize_document(),
        estimate_price(),
        find_sharia_status(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_account_redacts_provider_secrets() {
        let r = sync_account();
        assert_eq!(r.name, "sync_account");
        assert_eq!(r.cap_weight.get(), 5);
        match r.audit_scope {
            AuditScope::FullInput { redact } => {
                assert!(redact.contains(&"plaid_access_token"));
                assert!(redact.contains(&"snaptrade_user_secret"));
            }
            other => {
                panic!("sync_account must redact tokens per CLAUDE.md §0 rule 4; got {other:?}")
            }
        }
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn generate_report_logs_report_type_and_scope() {
        let r = generate_report();
        assert_eq!(r.name, "generate_report");
        assert_eq!(r.cap_weight.get(), 3);
        assert_eq!(r.audit_scope, AuditScope::Custom("report_type_and_scope"));
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn summarize_document_has_heaviest_cap_and_doc_bounds() {
        let r = summarize_document();
        assert_eq!(r.name, "summarize_document");
        assert_eq!(r.cap_weight.get(), 8);
        match r.numeric_bounds {
            NumericBounds::Document {
                approved_kinds,
                max_words,
            } => {
                assert_eq!(max_words, 50_000);
                assert!(approved_kinds.contains(&"pdf"));
                assert!(approved_kinds.contains(&"csv"));
                assert!(approved_kinds.contains(&"xlsx"));
                assert!(approved_kinds.contains(&"txt"));
            }
            other => panic!("summarize_document needs Document bounds; got {other:?}"),
        }
    }

    #[test]
    fn estimate_price_is_read_only_with_estimate_range_bounds() {
        let r = estimate_price();
        assert_eq!(r.name, "estimate_price");
        assert_eq!(r.numeric_bounds, NumericBounds::EstimateRange);
        // The tool surfaces a draft; only the user-confirmed
        // update_holding path writes to the ledger.
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn find_sharia_status_is_cheap_read() {
        let r = find_sharia_status();
        assert_eq!(r.name, "find_sharia_status");
        assert_eq!(r.cap_weight.get(), 1);
        assert_eq!(r.audit_scope, AuditScope::Custom("symbol"));
        assert!(!r.emits_truth_ledger);
    }

    #[test]
    fn no_lifecycle_tool_emits_truth_ledger() {
        // Lifecycle invariant per ADR 0020: none of these tools write
        // to Truth Ledger directly. estimate_price surfaces drafts
        // routed through update_holding (which DOES emit); sync_account
        // emits sync-run-ledger events from its own subsystem.
        for reg in all().iter() {
            assert!(
                !reg.emits_truth_ledger,
                "{} must not emit Truth Ledger directly per ADR 0020",
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
                "sync_account",
                "generate_report",
                "summarize_document",
                "estimate_price",
                "find_sharia_status",
            ]
        );
    }

    #[test]
    fn sync_account_redact_list_does_not_include_unrelated_fields() {
        // Catches a future regression where someone overzealously
        // adds non-secret fields to the redact list (which would mask
        // useful audit data without adding security).
        let r = sync_account();
        if let AuditScope::FullInput { redact } = r.audit_scope {
            for field in redact {
                assert!(
                    *field == "plaid_access_token" || *field == "snaptrade_user_secret",
                    "unexpected redact field {field}; PR adding it should update the test + doc"
                );
            }
        }
    }
}
