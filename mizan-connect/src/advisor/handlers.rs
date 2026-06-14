//! Track G — Advisor surface HTTP handlers.

use axum::extract::{Path, State};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::state::AppState;

use super::repository;

const DEFAULT_GRANT_DAYS: i64 = 365;
const MAX_GRANT_DAYS: i64 = 730;
const TOKEN_BYTES: usize = 32;

// ─── POST /v1/advisor/links ─────────────────────────────────────────
//
// Client issues a grant token to an advisor. Token returned raw ONCE
// in the response body; the DB stores only its SHA-256 hash.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueGrantRequest {
    /// The Mizan user id of the advisor being granted access. The
    /// client must already know this id (e.g. via an invite handshake
    /// on the advisor's team page).
    pub advisor_user_id: Uuid,
    /// `read_only` or `read_write`. Per spec §13.5 default is
    /// read_only; `read_write` is rare and currently used only by
    /// the family-office tier.
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Number of days until expiry. Defaults to 365; capped at 730.
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}

fn default_scope() -> String {
    "read_only".to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueGrantResponse {
    pub link_id: Uuid,
    /// The raw grant token. **Only present in this response.** The
    /// client surfaces it to the user once, never again — the DB only
    /// keeps the SHA-256 hash.
    pub grant_token: String,
    pub expires_at: String,
}

pub async fn issue_grant(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<IssueGrantRequest>,
) -> Result<Json<IssueGrantResponse>, AppError> {
    if req.advisor_user_id == user.id {
        return Err(AppError::bad_request(
            "cannot grant advisor access to yourself",
        ));
    }
    let scope = match req.scope.as_str() {
        "read_only" | "read_write" => req.scope.as_str(),
        _ => {
            return Err(AppError::bad_request(
                "scope must be read_only or read_write",
            ))
        }
    };
    let days = req
        .expires_in_days
        .unwrap_or(DEFAULT_GRANT_DAYS)
        .clamp(1, MAX_GRANT_DAYS);
    let expires_at = OffsetDateTime::now_utc() + Duration::days(days);

    let raw_token = generate_token();
    let hash = sha256_hex(raw_token.as_bytes());

    let link_id = repository::insert_link(
        state.db(),
        req.advisor_user_id,
        user.id,
        scope,
        &hash,
        expires_at,
    )
    .await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("advisor.grant_issued")
            .user(user.id)
            .data(&serde_json::json!({
                "link_id": link_id.to_string(),
                "advisor_user_id": req.advisor_user_id.to_string(),
                "scope": scope,
                "expires_in_days": days,
            })),
    )
    .await;

    Ok(Json(IssueGrantResponse {
        link_id,
        grant_token: raw_token,
        expires_at: expires_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    }))
}

fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ─── GET /v1/advisor/grants ─────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantListItem {
    pub id: Uuid,
    pub advisor_user_id: Uuid,
    pub scope: String,
    pub granted_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantListResponse {
    pub grants: Vec<GrantListItem>,
}

pub async fn list_my_grants(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<GrantListResponse>, AppError> {
    let now = OffsetDateTime::now_utc();
    let rows = repository::list_grants_by_client(state.db(), user.id).await?;
    let grants = rows
        .into_iter()
        .map(|r| GrantListItem {
            id: r.id,
            advisor_user_id: r.advisor_user_id,
            scope: r.scope,
            granted_at: fmt_rfc3339(r.granted_at),
            expires_at: fmt_rfc3339(r.expires_at),
            revoked_at: r.revoked_at.map(fmt_rfc3339),
            status: if r.revoked_at.is_some() {
                "revoked"
            } else if r.expires_at <= now {
                "expired"
            } else {
                "active"
            },
        })
        .collect();
    Ok(Json(GrantListResponse { grants }))
}

fn fmt_rfc3339(t: OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

// ─── DELETE /v1/advisor/links/:id ───────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RevokeQuery {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeResponse {
    pub revoked: bool,
}

pub async fn revoke_grant(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(link_id): Path<Uuid>,
) -> Result<Json<RevokeResponse>, AppError> {
    let rows = repository::revoke_link(state.db(), link_id, user.id, Some("user_revoked")).await?;
    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("advisor.grant_revoked")
            .user(user.id)
            .data(&serde_json::json!({
                "link_id": link_id.to_string(),
                "rows_affected": rows,
            })),
    )
    .await;
    Ok(Json(RevokeResponse { revoked: rows > 0 }))
}

// ─── GET /v1/advisor/clients ────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientLink {
    pub link_id: Uuid,
    pub client_user_id: Uuid,
    pub scope: String,
    pub granted_at: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListResponse {
    pub clients: Vec<ClientLink>,
}

pub async fn list_my_clients(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<ClientListResponse>, AppError> {
    let rows = repository::list_active_links_for_advisor(state.db(), user.id).await?;
    let clients = rows
        .into_iter()
        .map(|r| ClientLink {
            link_id: r.id,
            client_user_id: r.client_user_id,
            scope: r.scope,
            granted_at: fmt_rfc3339(r.granted_at),
            expires_at: fmt_rfc3339(r.expires_at),
        })
        .collect();
    Ok(Json(ClientListResponse { clients }))
}

// ─── POST /v1/advisor/sign-offs ─────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSignOffRequest {
    pub client_user_id: Uuid,
    pub holding_account_id: String,
    pub holding_symbol: String,
    /// `YYYY-MM-DD` — the `as_of_date` of the holdings_metadata row
    /// being signed off on.
    pub holding_as_of_date: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSignOffResponse {
    pub sign_off_id: Uuid,
}

pub async fn create_sign_off(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateSignOffRequest>,
) -> Result<Json<CreateSignOffResponse>, AppError> {
    let scope = repository::fetch_active_scope(state.db(), user.id, req.client_user_id).await?;
    if scope.is_none() {
        return Err(AppError::forbidden(
            "no active advisor link to this client (or scope insufficient)",
        ));
    }

    let as_of_date = Date::parse(
        &req.holding_as_of_date,
        time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|e| AppError::bad_request(format!("holdingAsOfDate must be YYYY-MM-DD ({e})")))?;

    let id = repository::insert_sign_off(
        state.db(),
        user.id,
        req.client_user_id,
        &req.holding_account_id,
        &req.holding_symbol,
        as_of_date,
        req.note.as_deref(),
    )
    .await?;

    // Per-action access log (advisor_access_log) + cross-system audit_log.
    let _ = repository::record_access(
        state.db(),
        user.id,
        req.client_user_id,
        "sign_off",
        Some(
            &serde_json::json!({
                "sign_off_id": id.to_string(),
                "holding_account_id": req.holding_account_id,
                "holding_symbol": req.holding_symbol,
            })
            .to_string(),
        ),
    )
    .await;
    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("advisor.sign_off_created")
            .user(user.id)
            .data(&serde_json::json!({
                "sign_off_id": id.to_string(),
                "client_user_id": req.client_user_id.to_string(),
                "holding_account_id": req.holding_account_id,
                "holding_symbol": req.holding_symbol,
            })),
    )
    .await;

    Ok(Json(CreateSignOffResponse { sign_off_id: id }))
}

// ─── POST /v1/advisor/sign-offs/:id/retract ─────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetractRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetractResponse {
    pub retracted: bool,
}

pub async fn retract_sign_off(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(sign_off_id): Path<Uuid>,
    Json(req): Json<RetractRequest>,
) -> Result<Json<RetractResponse>, AppError> {
    let rows =
        repository::retract_sign_off(state.db(), sign_off_id, user.id, req.reason.as_deref())
            .await?;
    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("advisor.sign_off_retracted")
            .user(user.id)
            .data(&serde_json::json!({
                "sign_off_id": sign_off_id.to_string(),
                "rows_affected": rows,
            })),
    )
    .await;
    Ok(Json(RetractResponse {
        retracted: rows > 0,
    }))
}

// ─── GET /v1/advisor/clients/:client_id/sign-offs ───────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOffListItem {
    pub id: Uuid,
    pub holding_account_id: String,
    pub holding_symbol: String,
    pub holding_as_of_date: String,
    pub note: Option<String>,
    pub signed_off_at: String,
    pub retracted_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOffListResponse {
    pub sign_offs: Vec<SignOffListItem>,
}

pub async fn list_sign_offs_for_client(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(client_user_id): Path<Uuid>,
) -> Result<Json<SignOffListResponse>, AppError> {
    let scope = repository::fetch_active_scope(state.db(), user.id, client_user_id).await?;
    if scope.is_none() {
        return Err(AppError::forbidden("no active advisor link to this client"));
    }
    let rows =
        repository::list_sign_offs_for_client_by_advisor(state.db(), user.id, client_user_id)
            .await?;
    let sign_offs = rows
        .into_iter()
        .map(|r| SignOffListItem {
            id: r.id,
            holding_account_id: r.holding_account_id,
            holding_symbol: r.holding_symbol,
            holding_as_of_date: r
                .holding_as_of_date
                .format(time::macros::format_description!("[year]-[month]-[day]"))
                .unwrap_or_default(),
            note: r.note,
            signed_off_at: fmt_rfc3339(r.signed_off_at),
            retracted_at: r.retracted_at.map(fmt_rfc3339),
        })
        .collect();

    let _ = repository::record_access(
        state.db(),
        user.id,
        client_user_id,
        "view_dashboard",
        Some(&serde_json::json!({ "endpoint": "sign-offs" }).to_string()),
    )
    .await;

    Ok(Json(SignOffListResponse { sign_offs }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_64_chars_lowercase() {
        let h = sha256_hex(b"sample");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        // sha256("abc")
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generate_token_length_is_stable() {
        // 32 bytes → base64url no-pad: 32 * 4 / 3 rounded up = 43 chars.
        let t = generate_token();
        assert_eq!(t.len(), 43, "got {t:?}");
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn generated_tokens_are_distinct() {
        // Probability of collision over two 256-bit tokens is ~2^-256.
        // If this ever fires, fix your RNG.
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }
}
