//! MCP gateway — Track K PR-K1 / Goal v3 §V Phase 10.
//!
//! Per ADR 0014 §21.3 (Mizan Capability Protocol sandbox) and the
//! autonomous-loop directive `Mizan_Continue_Autonomous_v3.md` lines
//! 79-91: per-user MCP gateway in Mizan Connect with:
//!
//! - **PR-K1** (this PR) — Server registration types + per-tool
//!   ToolPermission classifier ("ReadOnly" / "WriteScratchpad" /
//!   "RejectedFinancialWrite"). The read-mostly sandbox boundary is
//!   defined here in pure-math form; the actual enforcement in the
//!   dispatcher lands in PR-K2.
//! - **PR-K2** — Sandbox enforcement at the dispatcher level. Any
//!   MCP-tagged tool attempting to mutate truth_ledger / holdings /
//!   activities / balances is rejected before execution. Writes to
//!   the `scratchpad` namespace are permitted with `'mcp'` origin
//!   badge.
//! - **PR-K3** — Egress DLP filter: pattern-based rejection of
//!   SSN / PAN / Aadhaar / card / IBAN. Adversarial suite covering
//!   50+ exfiltration attempts. Per-call audit log.
//! - **PR-K4** — Public catalog capped at 5 hand-vetted servers for
//!   first 90 days per ADR 0014. Self-registered MCPs carry
//!   "not reviewed by Mizan" warning.
//!
//! # Sandbox doctrine
//!
//! MCP servers run user-controlled code. The read-mostly sandbox
//! treats them like cross-origin frames: they can READ from
//! whitelisted surfaces (mostly the agent's tool registry), can
//! WRITE only to a per-user `scratchpad` namespace, and CANNOT
//! mutate any financial-truth-bearing table. Violating tools are
//! rejected at the dispatcher boundary; the violation is logged to
//! `mcp_call_log` so support can analyse patterns.

pub mod egress_dlp;
pub mod handlers;
pub mod repository;
pub mod sandbox;
pub mod types;

pub use egress_dlp::{has_findings, scan_payload, DlpCategory, DlpFinding};
pub use sandbox::{classify_tool_permission, is_financial_truth_table, ToolPermission};
pub use types::{
    McpCallLogEntry, McpError, McpServer, McpServerRegistration, McpToolInvocation, McpTrustLevel,
};

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

/// `/v1/mcp/*` router. Mounted on both `/v1` and `/api/v1` aliases.
///
/// Per spec §21, MCP capability is Gold+ — entitlement gating lands
/// in a follow-up PR-K2.b. For now every authenticated user can
/// register a self-registered server (acknowledgement-gated) and
/// call it through the sandbox + DLP layers.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mcp/servers", post(handlers::register_server))
        .route("/mcp/servers", get(handlers::list_servers))
        .route("/mcp/servers/:id", delete(handlers::disconnect_server))
        .route("/mcp/servers/:id/call", post(handlers::call_tool))
}
