//! Track J PR-J1.b — OAuth flow HTTP handlers.
//!
//! Four endpoints:
//!
//! - `POST /v1/oauth/connect/{provider}` — authenticated. Builds an
//!   HMAC-signed `state` token + the provider's authorization URL.
//!   The desktop opens the URL in the system browser; the user
//!   authorizes; the provider redirects back to the callback URL with
//!   `code` + `state`.
//!
//! - `GET /v1/oauth/callback/{provider}` — unauthenticated (the user
//!   is on the provider's domain at this point). Verifies the state's
//!   HMAC, exchanges the `code` for a token set against the provider's
//!   token endpoint, encrypts the tokens via SecretCipher, and upserts
//!   a `user_oauth_connections` row.
//!
//! - `POST /v1/oauth/disconnect/{provider}` — authenticated. Marks the
//!   user's connection as disconnected (we don't delete — the audit
//!   trail wants the lifecycle preserved).
//!
//! - `GET /v1/oauth/connections` — authenticated. Lists the user's
//!   active connections.
//!
//! Token encryption: all four providers (Plaid + SnapTrade + the
//! eight in this catalog) share one encryption key today —
//! `OAUTH_TOKEN_ENCRYPTION_KEY`. Per-provider keys land when a
//! provider needs key rotation independent of the others.

use axum::extract::{Path, Query, State};
use axum::Json;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::secret_crypto::SecretCipher;
use crate::state::AppState;

use super::catalog::provider_descriptor;
use super::repository::{self, ConnectionInsert};
use super::state::{self as oauth_state, StatePayload};
use super::types::Provider;

// ─── POST /v1/oauth/connect/{provider} ──────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResponse {
    pub provider: Provider,
    pub authorization_url: String,
    pub state: String,
    pub scopes: Vec<&'static str>,
}

pub async fn connect(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(provider_slug): Path<String>,
) -> Result<Json<ConnectResponse>, AppError> {
    let provider = Provider::parse(&provider_slug)
        .ok_or_else(|| AppError::not_found(format!("unknown provider '{provider_slug}'")))?;
    let descriptor = provider_descriptor(provider).ok_or_else(|| {
        AppError::internal(format!(
            "catalog missing descriptor for {provider:?} — out-of-sync seed",
        ))
    })?;

    let oauth_cfg = state
        .config()
        .oauth
        .as_ref()
        .ok_or_else(|| AppError::not_implemented("OAuth not configured on this server"))?;

    let payload = StatePayload {
        user_id: user.id,
        provider,
        issued_at: OffsetDateTime::now_utc().unix_timestamp(),
    };
    let token = oauth_state::sign(&payload, oauth_cfg.state_secret.expose_secret().as_bytes())
        .map_err(|e| AppError::internal(format!("state signing failed: {e}")))?;

    let scopes: Vec<&'static str> = descriptor
        .required_scopes
        .iter()
        .map(|s| s.raw_scope())
        .collect();

    let authorization_url = build_authorization_url(
        descriptor.authorization_endpoint,
        &oauth_cfg.callback_url(provider),
        &oauth_cfg.client_id_for(provider),
        &scopes,
        &token,
    );

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("oauth.connect_initiated")
            .user(user.id)
            .data(&serde_json::json!({
                "provider": provider.slug(),
                "scopes": scopes,
            })),
    )
    .await;

    Ok(Json(ConnectResponse {
        provider,
        authorization_url,
        state: token,
        scopes,
    }))
}

fn build_authorization_url(
    endpoint: &str,
    redirect_uri: &str,
    client_id: &str,
    scopes: &[&str],
    state: &str,
) -> String {
    let scope = scopes.join(" ");
    // Catalog endpoints are validated at module-load via the
    // descriptor tests in catalog.rs; if a future PR slips in an
    // invalid URL here we'd rather return a recognisable malformed
    // string than panic in the handler.
    let mut url = match url::Url::parse(endpoint) {
        Ok(u) => u,
        Err(_) => return format!("{endpoint}#invalid-endpoint"),
    };
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &scope)
        .append_pair("state", state)
        // Google + Microsoft return refresh tokens only when explicit.
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    url.to_string()
}

// ─── GET /v1/oauth/callback/{provider} ──────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallbackResponse {
    pub provider: Provider,
    pub connection_id: Uuid,
    pub scopes_granted: String,
    pub expires_at: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    Path(provider_slug): Path<String>,
    Query(params): Query<CallbackParams>,
) -> Result<Json<CallbackResponse>, AppError> {
    if let Some(err) = params.error {
        return Err(AppError::bad_request(format!(
            "provider rejected authorization: {err}"
        )));
    }

    let provider = Provider::parse(&provider_slug)
        .ok_or_else(|| AppError::not_found(format!("unknown provider '{provider_slug}'")))?;

    let oauth_cfg = state
        .config()
        .oauth
        .as_ref()
        .ok_or_else(|| AppError::not_implemented("OAuth not configured on this server"))?;

    let payload = oauth_state::verify(
        &params.state,
        oauth_cfg.state_secret.expose_secret().as_bytes(),
    )
    .map_err(|e| AppError::unauthorized(format!("invalid state: {e}")))?;

    if payload.provider != provider {
        return Err(AppError::bad_request(
            "state's provider does not match callback path provider",
        ));
    }

    let descriptor = provider_descriptor(provider)
        .ok_or_else(|| AppError::internal("catalog missing descriptor at callback time"))?;

    // Exchange the auth code for an access token.
    let token_set = exchange_code(
        descriptor.token_endpoint,
        &params.code,
        &oauth_cfg.callback_url(provider),
        &oauth_cfg.client_id_for(provider),
        &oauth_cfg.client_secret_for(provider),
    )
    .await?;

    let cipher = SecretCipher::from_bytes(oauth_cfg.encryption_key.expose_secret().as_bytes())
        .map_err(|e| AppError::internal(format!("encryption key invalid: {e}")))?;

    let encrypted_access = cipher
        .encrypt(&SecretString::from(token_set.access_token.clone()))
        .map_err(|_| AppError::internal("access token encryption failed"))?;
    let (access_nonce, access_ct) = split_nonce(&encrypted_access);

    let (encrypted_refresh, refresh_nonce_owned) = if let Some(rt) = &token_set.refresh_token {
        let blob = cipher
            .encrypt(&SecretString::from(rt.clone()))
            .map_err(|_| AppError::internal("refresh token encryption failed"))?;
        let (nonce, ct) = split_nonce(&blob);
        (Some(ct.to_vec()), Some(nonce.to_vec()))
    } else {
        (None, None)
    };

    let provider_row = repository::provider_row(state.db(), provider)
        .await?
        .ok_or_else(|| {
            AppError::internal(format!(
                "oauth_providers row missing for {provider:?} — run migration 0015"
            ))
        })?;

    let mut tx = state.db().begin().await?;
    let connection_id = repository::upsert_connection(
        &mut tx,
        &ConnectionInsert {
            user_id: payload.user_id,
            provider_id: provider_row.id,
            encrypted_access_token: access_ct,
            access_token_nonce: access_nonce,
            encrypted_refresh_token: encrypted_refresh.as_deref(),
            refresh_token_nonce: refresh_nonce_owned.as_deref(),
            scopes_granted: &token_set.granted_scopes,
            expires_at: token_set.expires_at,
        },
    )
    .await?;
    tx.commit().await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("oauth.connect_completed")
            .user(payload.user_id)
            .data(&serde_json::json!({
                "provider": provider.slug(),
                "connection_id": connection_id.to_string(),
                "scopes_granted": token_set.granted_scopes,
                "has_refresh_token": token_set.refresh_token.is_some(),
            })),
    )
    .await;

    Ok(Json(CallbackResponse {
        provider,
        connection_id,
        scopes_granted: token_set.granted_scopes,
        expires_at: token_set.expires_at.and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
    }))
}

fn split_nonce(blob: &[u8]) -> (&[u8], &[u8]) {
    // SecretCipher prefixes a 12-byte nonce before the AES-GCM ciphertext.
    blob.split_at(12)
}

#[derive(Debug, Clone)]
struct TokenSetExchanged {
    access_token: String,
    refresh_token: Option<String>,
    granted_scopes: String,
    expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<TokenSetExchanged, AppError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("provider token endpoint unreachable: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(
            %status, %body,
            "oauth token exchange failed",
        );
        return Err(AppError::bad_request(format!(
            "token exchange failed with status {status}"
        )));
    }

    let body: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("token response not JSON: {e}")))?;

    // We persist only token_type=Bearer; reject Apple-style mac tokens
    // until we wire their HMAC signing (none of the catalog providers
    // use them).
    if matches!(body.token_type.as_deref(), Some(t) if !t.eq_ignore_ascii_case("bearer")) {
        return Err(AppError::bad_request(format!(
            "unsupported token_type '{}'",
            body.token_type.unwrap_or_default()
        )));
    }

    Ok(TokenSetExchanged {
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        granted_scopes: body.scope.unwrap_or_default(),
        expires_at: body.expires_in.and_then(|s| {
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp() + s).ok()
        }),
    })
}

// ─── POST /v1/oauth/disconnect/{provider} ───────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectResponse {
    pub disconnected: bool,
    pub provider: Provider,
}

pub async fn disconnect(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(provider_slug): Path<String>,
) -> Result<Json<DisconnectResponse>, AppError> {
    let provider = Provider::parse(&provider_slug)
        .ok_or_else(|| AppError::not_found(format!("unknown provider '{provider_slug}'")))?;

    let rows = repository::disconnect(state.db(), user.id, provider, "user_initiated").await?;

    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new("oauth.disconnected")
            .user(user.id)
            .data(&serde_json::json!({
                "provider": provider.slug(),
                "rows_affected": rows,
            })),
    )
    .await;

    Ok(Json(DisconnectResponse {
        disconnected: rows > 0,
        provider,
    }))
}

// ─── GET /v1/oauth/connections ──────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListItem {
    pub provider: Provider,
    pub display_name: String,
    pub scopes: Vec<String>,
    pub connected_at: String,
    pub expires_at: Option<String>,
    pub reconsent_due_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListResponse {
    pub connections: Vec<ConnectionListItem>,
}

pub async fn list_connections(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<ConnectionListResponse>, AppError> {
    let rows = repository::list_active_connections(state.db(), user.id).await?;
    let connections = rows
        .into_iter()
        .map(|r| {
            let scopes = r
                .scopes_granted
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            ConnectionListItem {
                provider: r.provider,
                display_name: r.provider_display_name,
                scopes,
                connected_at: r
                    .connected_at
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                expires_at: r.expires_at.and_then(|t| {
                    t.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                reconsent_due_at: r
                    .reconsent_due_at
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
            }
        })
        .collect();
    Ok(Json(ConnectionListResponse { connections }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn build_authorization_url_includes_required_params() {
        let url = build_authorization_url(
            "https://accounts.google.com/o/oauth2/v2/auth",
            "http://localhost:8080/v1/oauth/callback/google-drive",
            "client-id",
            &["https://www.googleapis.com/auth/drive.readonly", "openid"],
            "STATE_TOKEN",
        );
        assert!(url.contains("client_id=client-id"));
        assert!(url.contains("state=STATE_TOKEN"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("access_type=offline"));
        // Scopes are joined with space, URL-encoded → '+' or '%20'.
        assert!(url.contains("scope="));
    }

    #[allow(dead_code)] // Used as a compile-time fence so the API stays
    fn _api_compiles() {
        let _: fn(State<AppState>, AuthenticatedUser, Path<String>) -> _ =
            |s, u, p| async move { connect(s, u, p).await };
        let _: fn(State<AppState>, Path<String>, Query<CallbackParams>) -> _ =
            |s, p, q| async move { callback(s, p, q).await };
    }

    #[derive(serde::Deserialize, Debug)]
    struct TokenResponseProbe {
        access_token: String,
    }

    #[test]
    fn token_response_parses_minimal_payload() {
        let body = r#"{"access_token":"abc","scope":"x y","expires_in":3600}"#;
        let parsed: TokenResponseProbe = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.access_token, "abc");
    }
}
