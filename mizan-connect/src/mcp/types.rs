//! MCP gateway types — Track K PR-K1 / Goal v3 §V Phase 10.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Trust level for an MCP server. Per ADR 0014 only `Vetted` servers
/// (hand-reviewed by the Mizan team) appear in the public catalog;
/// `SelfRegistered` servers carry a "not reviewed by Mizan" warning
/// + tighter rate limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTrustLevel {
    /// Hand-reviewed by Mizan + present in the public catalog.
    /// Default rate limit: 60 req/min/user, 1000 req/day/user.
    Vetted,
    /// User self-registered. UI surfaces "not reviewed by Mizan"
    /// warning. Tighter rate limit: 10 req/min/user, 100 req/day/user.
    SelfRegistered,
}

/// A registered MCP server. Persisted in `mcp_servers` (cloud-side
/// migration shipped in task #35).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: Uuid,
    pub user_id: Uuid,
    pub server_url: String,
    /// Display name on the catalog UI.
    pub name: String,
    pub trust_level: McpTrustLevel,
    /// Authentication strategy. PR-K1.b adds OAuth / API-key variants.
    pub auth_method: String,
    /// Last successful tool invocation timestamp.
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub last_invoked_at: Option<OffsetDateTime>,
    /// Server creation timestamp.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Inbound payload for `POST /v1/mcp/server` (PR-K2 wires the
/// handler). Adds the server to the user's MCP registry after
/// scrubbing the URL + verifying server liveness.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRegistration {
    pub server_url: String,
    pub name: String,
    pub auth_method: String,
    /// Per CLAUDE.md §16: a self-registered MCP must explicitly opt
    /// into "I understand this server is not reviewed by Mizan".
    pub acknowledged_unreviewed: bool,
}

/// A single tool invocation by the agent against a registered MCP
/// server. Persisted to `mcp_call_log` for the audit trail per
/// ADR 0014 §"per-call audit log".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCallLogEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub mcp_server_id: Uuid,
    /// Tool name as exposed via MCP. The classifier in `sandbox.rs`
    /// maps this to a `ToolPermission` decision.
    pub tool_name: String,
    /// Sanitized invocation params with secrets redacted.
    pub params_redacted: serde_json::Value,
    /// Whether the call was allowed by the sandbox.
    pub allowed: bool,
    /// If `allowed == false`, the reason string.
    pub denial_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub invoked_at: OffsetDateTime,
}

/// Lightweight pre-execution invocation request the dispatcher
/// passes to the sandbox for permission classification.
#[derive(Debug, Clone)]
pub struct McpToolInvocation<'a> {
    pub mcp_server_id: Uuid,
    pub tool_name: &'a str,
    /// Best-effort identification of which DB table (if any) this
    /// tool would mutate. The agent's tool registry surfaces this
    /// via the `AI Safety Runtime` properties pinned at registration
    /// time (per ADR 0008 + CLAUDE.md §0 rule 3).
    pub target_table: Option<&'a str>,
    /// Whether the tool's metadata declares it as a write operation.
    pub is_write: bool,
}

/// Errors raised by the MCP gateway handlers.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("server '{0}' is not registered for this user")]
    UnknownServer(String),
    #[error("server URL '{0}' failed liveness check")]
    LivenessCheckFailed(String),
    #[error("self-registered server requires acknowledgement of unreviewed status")]
    MissingUnreviewedAcknowledgement,
    #[error("tool invocation rejected by sandbox: {0}")]
    SandboxRejection(String),
    #[error("egress DLP filter rejected payload: {0}")]
    DlpRejection(String),
    #[error("rate limit exceeded for trust level {trust:?}")]
    RateLimitExceeded { trust: McpTrustLevel },
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mcp_trust_level_serde_kebab_case() {
        let json = serde_json::to_string(&McpTrustLevel::Vetted).expect("ok");
        assert_eq!(json, "\"vetted\"");
        let json = serde_json::to_string(&McpTrustLevel::SelfRegistered).expect("ok");
        assert_eq!(json, "\"self-registered\"");
        let parsed: McpTrustLevel = serde_json::from_str("\"vetted\"").expect("ok");
        assert_eq!(parsed, McpTrustLevel::Vetted);
    }

    #[test]
    fn registration_requires_unreviewed_acknowledgement_field() {
        let json = r#"{
            "serverUrl": "https://example.com",
            "name": "Example",
            "authMethod": "api-key",
            "acknowledgedUnreviewed": true
        }"#;
        let reg: McpServerRegistration = serde_json::from_str(json).expect("ok");
        assert!(reg.acknowledged_unreviewed);
    }
}
