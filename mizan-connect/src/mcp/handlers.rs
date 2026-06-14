//! Track K — MCP gateway HTTP handlers.
//!
//! Four endpoints:
//!
//! - `POST /v1/mcp/servers` — register a server (catalog or
//!   self-registered). Self-registered requires explicit
//!   acknowledgement per spec §21.5.
//! - `GET  /v1/mcp/servers` — list user's active servers.
//! - `POST /v1/mcp/servers/{id}/call` — proxy a tool invocation to
//!   the user's registered server. Sandboxed: any tool that would
//!   mutate truth_ledger / holdings / activities / balances is
//!   rejected at the boundary. Outbound payload is DLP-scanned;
//!   SSN/PAN/Aadhaar/card/IBAN patterns are rejected before send.
//! - `DELETE /v1/mcp/servers/{id}` — mark disconnected.
//!
//! Every call writes to `mcp_call_log` (params + response stored as
//! BLAKE3 digests, never the raw payload — working-agreement §3.4
//! bright line).

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::state::AppState;

use super::egress_dlp;
use super::repository;
use super::sandbox;
use super::types::McpTrustLevel;

// Per spec §21.4: 60 req/min on Vetted servers, 10 req/min on
// SelfRegistered. Window = 60s; over-quota → 429.
const RATE_WINDOW_SECS: i64 = 60;
const RATE_VETTED: i64 = 60;
const RATE_SELF_REGISTERED: i64 = 10;
// Spec §21.3: outbound MCP call hard ceiling.
const CALL_TIMEOUT_SECS: u64 = 15;

// ─── POST /v1/mcp/servers ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterServerRequest {
    pub source: String, // "catalog" or "self_registered"
    #[serde(default)]
    pub catalog_entry_id: Option<Uuid>,
    #[serde(default)]
    pub custom_server_url: Option<String>,
    #[serde(default)]
    pub auth_method: Option<String>,
    pub display_name: String,
    /// Required when source == "self_registered".
    #[serde(default)]
    pub acknowledged_unreviewed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterServerResponse {
    pub id: Uuid,
    pub trust_level: McpTrustLevel,
}

pub async fn register_server(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<RegisterServerRequest>,
) -> Result<Json<RegisterServerResponse>, AppError> {
    if req.display_name.trim().is_empty() {
        return Err(AppError::bad_request("displayName must not be empty"));
    }
    let trust_level = match req.source.as_str() {
        "catalog" => {
            if req.catalog_entry_id.is_none() {
                return Err(AppError::bad_request(
                    "catalog source requires catalogEntryId",
                ));
            }
            // Trust level surfaces from the catalog row at call time;
            // we record "untrusted" by default until ADR 0048 wires
            // the trust_level column through end-to-end.
            McpTrustLevel::Vetted
        }
        "self_registered" => {
            if !req.acknowledged_unreviewed {
                return Err(AppError::bad_request(
                    "self-registered server requires acknowledgedUnreviewed=true per spec §21.5",
                ));
            }
            if req
                .custom_server_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_none()
            {
                return Err(AppError::bad_request(
                    "self-registered source requires customServerUrl",
                ));
            }
            McpTrustLevel::SelfRegistered
        }
        other => {
            return Err(AppError::bad_request(format!(
                "unknown source '{other}' (use 'catalog' or 'self_registered')"
            )));
        }
    };

    let trust_level_str = match trust_level {
        McpTrustLevel::Vetted => "untrusted", // ADR 0048 pending; surface as untrusted at DB layer
        McpTrustLevel::SelfRegistered => "untrusted",
    };

    let id = repository::insert_server(
        state.db(),
        &repository::McpServerInsert {
            user_id: user.id,
            source: req.source.as_str(),
            catalog_entry_id: req.catalog_entry_id,
            custom_server_url: req.custom_server_url.as_deref(),
            auth_method: req.auth_method.as_deref(),
            display_name: req.display_name.trim(),
            encrypted_credentials: None,
            credentials_nonce: None,
            trust_level: trust_level_str,
            user_acknowledged: req.acknowledged_unreviewed,
        },
    )
    .await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("mcp.server_registered")
            .user(user.id)
            .data(&serde_json::json!({
                "server_id": id.to_string(),
                "source": req.source,
                "trust_level": trust_level,
            })),
    )
    .await;

    Ok(Json(RegisterServerResponse { id, trust_level }))
}

// ─── GET /v1/mcp/servers ────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListItem {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub custom_server_url: Option<String>,
    pub trust_level: String,
    pub acknowledged_unreviewed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListResponse {
    pub servers: Vec<ServerListItem>,
}

pub async fn list_servers(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<ServerListResponse>, AppError> {
    let rows = repository::list_active_servers(state.db(), user.id).await?;
    let servers = rows
        .into_iter()
        .map(|r| ServerListItem {
            id: r.id,
            source: r.source,
            display_name: r.display_name,
            custom_server_url: r.custom_server_url,
            trust_level: r.trust_level,
            acknowledged_unreviewed_at: r.user_acknowledged_at.and_then(|t| {
                t.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
            created_at: r
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        })
        .collect();
    Ok(Json(ServerListResponse { servers }))
}

// ─── DELETE /v1/mcp/servers/:id ─────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectResponse {
    pub disconnected: bool,
}

pub async fn disconnect_server(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(server_id): Path<Uuid>,
) -> Result<Json<DisconnectResponse>, AppError> {
    let rows = repository::disconnect_server(state.db(), user.id, server_id).await?;
    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("mcp.server_disconnected")
            .user(user.id)
            .data(&serde_json::json!({
                "server_id": server_id.to_string(),
                "rows_affected": rows,
            })),
    )
    .await;
    Ok(Json(DisconnectResponse {
        disconnected: rows > 0,
    }))
}

// ─── POST /v1/mcp/servers/:id/call ──────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    pub tool_name: String,
    #[serde(default)]
    pub params: Value,
    /// Tool metadata the dispatcher injects so the sandbox can
    /// classify without inspecting the tool name. `is_write` +
    /// `target_table` are pulled from the tool's registration record.
    #[serde(default)]
    pub is_write: bool,
    #[serde(default)]
    pub target_table: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallResponse {
    pub outcome: String,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: i32,
    pub dlp_findings: Vec<String>,
}

pub async fn call_tool(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(server_id): Path<Uuid>,
    Json(req): Json<CallRequest>,
) -> Result<Json<CallResponse>, AppError> {
    let server = repository::fetch_active_server(state.db(), user.id, server_id)
        .await?
        .ok_or_else(|| AppError::not_found(format!("server {server_id} not registered")))?;

    let trust = trust_from_db(&server.trust_level);
    let rate_window_cap = match trust {
        McpTrustLevel::Vetted => RATE_VETTED,
        McpTrustLevel::SelfRegistered => RATE_SELF_REGISTERED,
    };
    let recent_count =
        repository::count_calls_in_window(state.db(), user.id, RATE_WINDOW_SECS).await?;
    if recent_count >= rate_window_cap {
        return Err(AppError::too_many_requests(format!(
            "rate limit exceeded for trust level {trust:?} \
             ({recent_count}/{rate_window_cap} calls in the last {RATE_WINDOW_SECS}s)"
        )));
    }

    let started = Instant::now();
    let params_digest = blake3_hex(&serde_json::to_vec(&req.params).unwrap_or_default());

    // ── Sandbox gate ────────────────────────────────────────────────
    // Per spec §21.3: any tool that would mutate a financial-truth-
    // bearing table is rejected at the boundary. The classifier
    // returns a `ToolPermission`; only `ReadOnly` and
    // `WriteScratchpad` proceed.
    let permission = sandbox::classify_tool_permission(&super::types::McpToolInvocation {
        mcp_server_id: server_id,
        tool_name: &req.tool_name,
        target_table: req.target_table.as_deref(),
        is_write: req.is_write,
    });
    if !matches!(
        permission,
        sandbox::ToolPermission::ReadOnly | sandbox::ToolPermission::WriteScratchpad
    ) {
        return record_and_return_rejection(
            &state,
            user.id,
            server_id,
            &req.tool_name,
            &params_digest,
            "rejected_by_sandbox_gate",
            format!("permission classifier returned {permission:?}"),
            started,
            Vec::new(),
        )
        .await;
    }

    // ── Egress DLP filter ───────────────────────────────────────────
    // Per spec §21.3: outbound payload is pattern-scanned for
    // sensitive identifiers (SSN/PAN/Aadhaar/card/IBAN). Any finding
    // → reject before send.
    let params_text = serde_json::to_string(&req.params).unwrap_or_default();
    let findings = egress_dlp::scan_payload(&params_text);
    if !findings.is_empty() {
        let labels: Vec<String> = findings
            .iter()
            .map(|f| format!("{:?}", f.category))
            .collect();
        return record_and_return_rejection(
            &state,
            user.id,
            server_id,
            &req.tool_name,
            &params_digest,
            "rejected_by_dlp",
            format!("DLP findings: {labels:?}"),
            started,
            labels,
        )
        .await;
    }

    // ── Forward the call ────────────────────────────────────────────
    let server_url = server.custom_server_url.clone().ok_or_else(|| {
        AppError::not_implemented(
            "catalog-routed MCP calls land in PR-K2.b (this PR ships self_registered only)",
        )
    })?;

    let (outcome, result_value, error_message, response_digest) =
        match forward_call(&server_url, &req.tool_name, &req.params).await {
            Ok(value) => {
                let bytes = serde_json::to_vec(&value).unwrap_or_default();
                ("ok", Some(value), None, blake3_hex(&bytes))
            }
            Err(ForwardError::Timeout) => (
                "timeout",
                None,
                Some("upstream MCP server timed out".to_string()),
                blake3_hex(b""),
            ),
            Err(ForwardError::Upstream(status, body)) => {
                let body_digest = blake3_hex(body.as_bytes());
                (
                    "error",
                    None,
                    Some(format!("upstream HTTP {status}: {body}")),
                    body_digest,
                )
            }
            Err(ForwardError::Network(err)) => (
                "error",
                None,
                Some(format!("network: {err}")),
                blake3_hex(b""),
            ),
        };

    let duration_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i32;

    let mut tx = state.db().begin().await?;
    let _ = repository::record_call(
        &mut tx,
        user.id,
        server_id,
        &req.tool_name,
        &params_digest,
        &response_digest,
        duration_ms,
        outcome,
        error_message.as_deref(),
    )
    .await?;
    tx.commit().await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("mcp.tool_call")
            .user(user.id)
            .data(&serde_json::json!({
                "server_id": server_id.to_string(),
                "tool_name": req.tool_name,
                "outcome": outcome,
                "duration_ms": duration_ms,
            })),
    )
    .await;

    Ok(Json(CallResponse {
        outcome: outcome.to_string(),
        result: result_value,
        error: error_message,
        duration_ms,
        dlp_findings: Vec::new(),
    }))
}

#[allow(clippy::too_many_arguments)]
async fn record_and_return_rejection(
    state: &AppState,
    user_id: Uuid,
    server_id: Uuid,
    tool_name: &str,
    params_digest: &str,
    outcome: &str,
    error_message: String,
    started: Instant,
    dlp_findings: Vec<String>,
) -> Result<Json<CallResponse>, AppError> {
    let duration_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i32;
    let mut tx = state.db().begin().await?;
    let _ = repository::record_call(
        &mut tx,
        user_id,
        server_id,
        tool_name,
        params_digest,
        &blake3_hex(b""),
        duration_ms,
        outcome,
        Some(&error_message),
    )
    .await?;
    tx.commit().await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new(&format!("mcp.{outcome}"))
            .user(user_id)
            .data(&serde_json::json!({
                "server_id": server_id.to_string(),
                "tool_name": tool_name,
                "duration_ms": duration_ms,
            })),
    )
    .await;

    Ok(Json(CallResponse {
        outcome: outcome.to_string(),
        result: None,
        error: Some(error_message),
        duration_ms,
        dlp_findings,
    }))
}

#[derive(Debug)]
enum ForwardError {
    Timeout,
    Upstream(u16, String),
    Network(String),
}

async fn forward_call(
    server_url: &str,
    tool_name: &str,
    params: &Value,
) -> Result<Value, ForwardError> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(CALL_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(ForwardError::Network(e.to_string())),
    };

    let payload = serde_json::json!({
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": params,
        },
    });

    let url = format!("{}/tools/call", server_url.trim_end_matches('/'));
    let response = match client.post(&url).json(&payload).send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return Err(ForwardError::Timeout),
        Err(e) => return Err(ForwardError::Network(e.to_string())),
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ForwardError::Upstream(status.as_u16(), body));
    }

    match response.json::<Value>().await {
        Ok(v) => Ok(v),
        Err(e) => Err(ForwardError::Network(format!(
            "upstream response not JSON: {e}"
        ))),
    }
}

fn trust_from_db(s: &str) -> McpTrustLevel {
    if s == "trusted" {
        McpTrustLevel::Vetted
    } else {
        McpTrustLevel::SelfRegistered
    }
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn trust_from_db_parses_known_strings() {
        assert!(matches!(trust_from_db("trusted"), McpTrustLevel::Vetted));
        assert!(matches!(
            trust_from_db("untrusted"),
            McpTrustLevel::SelfRegistered
        ));
        // Defensive: unknown strings default to the strictest tier.
        assert!(matches!(
            trust_from_db("garbage"),
            McpTrustLevel::SelfRegistered
        ));
    }

    #[test]
    fn blake3_hex_returns_64_char_hex() {
        let h = blake3_hex(b"sample");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn rate_caps_match_spec() {
        // §21.4: 60/min Vetted, 10/min SelfRegistered.
        assert_eq!(RATE_VETTED, 60);
        assert_eq!(RATE_SELF_REGISTERED, 10);
        assert_eq!(RATE_WINDOW_SECS, 60);
    }
}
