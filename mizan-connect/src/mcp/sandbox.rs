//! MCP §21.3 read-mostly sandbox classifier — Track K PR-K1 / Phase 10.
//!
//! Per ADR 0014 §21.3, MCP-tagged tools are routed through a
//! read-mostly gate at the dispatcher level. This module defines
//! the **pure classification function** that decides whether an
//! invocation is allowed; PR-K2 wires it into the dispatcher.
//!
//! # Sandbox boundaries
//!
//! The financial-truth-bearing tables are the absolute boundary:
//!
//! | Table              | Mutation by MCP? |
//! |--------------------|------------------|
//! | `truth_ledger`     | ❌ rejected       |
//! | `holdings`         | ❌ rejected       |
//! | `activities`       | ❌ rejected       |
//! | `balances`         | ❌ rejected       |
//! | `accounts`         | ❌ rejected       |
//! | `assets`           | ❌ rejected       |
//! | `liabilities`      | ❌ rejected       |
//! | `transactions`     | ❌ rejected       |
//! | `goals`            | ❌ rejected       |
//! | `zakat_payments`   | ❌ rejected       |
//! | `hawl_anchors`     | ❌ rejected       |
//! | `scratchpad`       | ✅ allowed (with `'mcp'` origin badge) |
//! | (read-only ops)    | ✅ allowed        |
//!
//! # Why pure-math
//!
//! The classifier is a single deterministic function. Tests can
//! enumerate every (tool_name, target_table, is_write) combination
//! and assert the decision matches the table above. PR-K2 wires it
//! into the dispatcher; PR-K3 adds the egress DLP filter on top.

use super::types::McpToolInvocation;

/// Decision returned by the sandbox classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPermission {
    /// Read-only tool — allowed without restriction.
    ReadOnly,
    /// Writes confined to the `scratchpad` namespace — allowed with
    /// `'mcp'` origin badge attached to written rows.
    WriteScratchpad,
    /// Attempted write to a financial-truth-bearing table — rejected
    /// at the dispatcher boundary. The denial reason is rendered for
    /// the audit log + the user's UI.
    RejectedFinancialWrite,
    /// Mutating tool not classified by the registry — rejected
    /// conservatively. The agent's tool registry must classify each
    /// tool at registration time (per ADR 0008); missing
    /// classification is a developer error, not a runtime fallback.
    RejectedUnclassified,
}

/// The full list of financial-truth-bearing tables an MCP server
/// must NOT mutate. Mirrors the table in the module docstring.
const PROTECTED_TABLES: &[&str] = &[
    "truth_ledger",
    "holdings",
    "activities",
    "balances",
    "accounts",
    "assets",
    "liabilities",
    "transactions",
    "goals",
    "zakat_payments",
    "hawl_anchors",
    "portfolio_snapshots",
    "asset_history",
    "fx_rates",
    "user_oauth_connections",
    "user_memory",
    "user_memory_embeddings",
    "news_items_per_user",
    "advisor_links",
    "team_memberships",
];

/// Check whether a target table name is on the protected list.
/// Returns true for any case + whitespace variation of a protected
/// table name.
pub fn is_financial_truth_table(table_name: &str) -> bool {
    let normalized = table_name.trim().to_lowercase();
    PROTECTED_TABLES.contains(&normalized.as_str())
}

/// Classify an MCP tool invocation per the §21.3 sandbox doctrine.
/// Returns the permission decision; the dispatcher then either
/// proceeds or rejects with the appropriate audit-log entry.
pub fn classify_tool_permission(invocation: &McpToolInvocation<'_>) -> ToolPermission {
    if !invocation.is_write {
        return ToolPermission::ReadOnly;
    }
    match invocation.target_table {
        None => ToolPermission::RejectedUnclassified,
        Some(table) if is_financial_truth_table(table) => ToolPermission::RejectedFinancialWrite,
        Some("scratchpad") => ToolPermission::WriteScratchpad,
        // Any other table is unclassified — conservative reject.
        Some(_) => ToolPermission::RejectedUnclassified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_inv<'a>(tool: &'a str, target: Option<&'a str>, write: bool) -> McpToolInvocation<'a> {
        McpToolInvocation {
            mcp_server_id: Uuid::nil(),
            tool_name: tool,
            target_table: target,
            is_write: write,
        }
    }

    #[test]
    fn read_only_tools_always_allowed() {
        // Even if "target_table" is a protected table, a read is fine.
        let inv = make_inv("get_holdings", Some("holdings"), false);
        assert_eq!(classify_tool_permission(&inv), ToolPermission::ReadOnly);
        let inv = make_inv("query_ledger", Some("truth_ledger"), false);
        assert_eq!(classify_tool_permission(&inv), ToolPermission::ReadOnly);
    }

    #[test]
    fn writes_to_truth_ledger_rejected() {
        let inv = make_inv("write_entry", Some("truth_ledger"), true);
        assert_eq!(
            classify_tool_permission(&inv),
            ToolPermission::RejectedFinancialWrite
        );
    }

    #[test]
    fn writes_to_holdings_activities_balances_rejected() {
        for table in [
            "holdings",
            "activities",
            "balances",
            "accounts",
            "assets",
            "liabilities",
            "transactions",
            "goals",
            "zakat_payments",
            "hawl_anchors",
        ] {
            let inv = make_inv("evil_write", Some(table), true);
            assert_eq!(
                classify_tool_permission(&inv),
                ToolPermission::RejectedFinancialWrite,
                "{table} must be protected from MCP writes",
            );
        }
    }

    #[test]
    fn writes_to_scratchpad_allowed() {
        let inv = make_inv("note", Some("scratchpad"), true);
        assert_eq!(
            classify_tool_permission(&inv),
            ToolPermission::WriteScratchpad
        );
    }

    #[test]
    fn writes_with_no_target_table_rejected_unclassified() {
        // Tool registry didn't declare what table it writes to —
        // safer to reject than to assume scratchpad.
        let inv = make_inv("mystery_write", None, true);
        assert_eq!(
            classify_tool_permission(&inv),
            ToolPermission::RejectedUnclassified
        );
    }

    #[test]
    fn writes_to_unknown_table_rejected_unclassified() {
        // A table that isn't in PROTECTED_TABLES AND isn't
        // `scratchpad` is conservative-rejected. New tables MUST be
        // explicitly added to PROTECTED_TABLES + a follow-up PR
        // before MCP can write to them.
        let inv = make_inv("write_user_settings", Some("user_settings"), true);
        assert_eq!(
            classify_tool_permission(&inv),
            ToolPermission::RejectedUnclassified
        );
    }

    #[test]
    fn is_financial_truth_table_case_and_whitespace_insensitive() {
        assert!(is_financial_truth_table("truth_ledger"));
        assert!(is_financial_truth_table("TRUTH_LEDGER"));
        assert!(is_financial_truth_table("  truth_ledger  "));
        assert!(!is_financial_truth_table("TruthLedger")); // intentional: CamelCase isn't a table-name convention
        assert!(!is_financial_truth_table("scratchpad"));
        assert!(!is_financial_truth_table("dropbox_files"));
    }

    #[test]
    fn protected_tables_include_all_financial_truth_bearing() {
        // Pin the protected list at code-review time so adding a new
        // financial-truth-bearing table without protecting it from
        // MCP writes fails CI immediately.
        let must_protect = [
            "truth_ledger",
            "holdings",
            "activities",
            "balances",
            "accounts",
            "assets",
            "liabilities",
            "transactions",
            "goals",
            "zakat_payments",
            "hawl_anchors",
            "user_memory",
            "user_oauth_connections",
        ];
        for table in must_protect {
            assert!(
                is_financial_truth_table(table),
                "{table} is financial-truth-bearing but not in PROTECTED_TABLES",
            );
        }
    }

    #[test]
    fn adversarial_case_variations_still_protected() {
        // Defence in depth: an MCP server attempting to bypass the
        // gate by sending mixed-case or whitespace-padded target
        // table names is still rejected.
        for evil in [
            "Truth_Ledger",
            "  TRUTH_LEDGER  ",
            "holdings\t",
            " HOLDINGS ",
        ] {
            let inv = make_inv("evil", Some(evil), true);
            assert_eq!(
                classify_tool_permission(&inv),
                ToolPermission::RejectedFinancialWrite,
                "{evil:?} bypass attempt must be rejected",
            );
        }
    }
}
