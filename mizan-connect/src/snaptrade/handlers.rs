//! HTTP handlers for the SnapTrade router.
//!
//! Every handler that needs the SnapTrade client wraps it in a "is the
//! integration configured?" gate so we degrade cleanly when the cloud
//! is deployed without SnapTrade secrets. The pattern matches Plaid's.

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::auth::AuthenticatedUser;
use crate::error::{AppError, ErrorCode};
use crate::state::AppState;

use super::client::SnapTradeClient;
use super::repository;
use super::types::{
    ConnectionEnvelope, LoginPortalEnvelope, SnapTradeAccount, SnapTradeActivity,
    SnapTradePosition, SyncSummary,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub configured: bool,
    pub environment: Option<&'static str>,
    pub message: &'static str,
}

/// `GET /v1/sync/snaptrade/health` — public-ish diagnostic so the
/// desktop "Connect a brokerage" button can light up only when the
/// cloud is actually wired. Same shape as the Plaid health endpoint
/// so the desktop discovery code can share the parser.
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    match state.config().snaptrade.as_ref() {
        Some(cfg) => Json(HealthResponse {
            configured: true,
            environment: Some(cfg.environment.as_str()),
            message: "SnapTrade is configured for Gold brokerage sync",
        }),
        None => Json(HealthResponse {
            configured: false,
            environment: None,
            message: "SnapTrade is not configured on this cloud",
        }),
    }
}

/// `POST /v1/sync/snaptrade/login-portal` — register the Mizan user
/// with SnapTrade if they aren't already, then issue a one-time portal
/// URL the desktop opens in the system browser.
pub async fn create_login_portal(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<LoginPortalEnvelope>, AppError> {
    let cfg = state.config().snaptrade.as_ref().ok_or_else(|| {
        AppError::new(
            ErrorCode::ServiceUnavailable,
            "SnapTrade not configured".to_string(),
        )
    })?;
    let client = SnapTradeClient::new(cfg);

    // 1) Register (or fetch existing) SnapTrade user.
    let (snaptrade_user_id, user_secret) =
        repository::ensure_user_registered(&state, &client, &user.0.id, &cfg.token_encryption_key)
            .await?;

    // 2) Generate the login portal URL.
    let portal = client
        .login_portal(
            &snaptrade_user_id,
            &user_secret,
            cfg.custom_redirect.as_deref(),
            None,
        )
        .await?;

    Ok(Json(LoginPortalEnvelope {
        redirect_uri: portal.redirect_uri,
        session_id: portal.session_id,
    }))
}

/// `GET /v1/sync/snaptrade/connections` — list active brokerage
/// authorizations for the signed-in user. Used by the desktop to
/// render the connected-brokerages card.
pub async fn list_connections(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<ConnectionEnvelope>>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    let auths = client.list_authorizations(&snap_user, &secret).await?;
    let envelopes = auths
        .into_iter()
        .map(ConnectionEnvelope::from_authorization)
        .collect();
    Ok(Json(envelopes))
}

/// `DELETE /v1/sync/snaptrade/connections/:authorization_id` —
/// revoke a brokerage authorization.
pub async fn disconnect_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(authorization_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    client
        .disconnect_authorization(&snap_user, &secret, &authorization_id)
        .await?;
    Ok(Json(
        serde_json::json!({ "disconnected": authorization_id }),
    ))
}

pub async fn list_accounts(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<SnapTradeAccount>>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    let accounts = client.list_accounts(&snap_user, &secret).await?;
    Ok(Json(accounts))
}

pub async fn list_positions(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<SnapTradePosition>>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    let positions = client
        .list_positions(&snap_user, &secret, &account_id)
        .await?;
    Ok(Json(positions))
}

pub async fn list_activities(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<SnapTradeActivity>>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    let activities = client
        .list_activities(&snap_user, &secret, &account_id)
        .await?;
    Ok(Json(activities))
}

/// `POST /v1/sync/snaptrade/sync` — run a full read-only sync pass:
/// list accounts → for each, fetch positions + activities. Returns
/// the count of each so the desktop can display a "synced N
/// accounts / M positions / K activities" toast.
pub async fn sync_now(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<SyncSummary>, AppError> {
    let (client, snap_user, secret) = resolve(&state, &user.0.id).await?;
    let accounts = client.list_accounts(&snap_user, &secret).await?;
    let mut positions_synced = 0u32;
    let mut activities_synced = 0u32;
    for acct in &accounts {
        if let Ok(positions) = client.list_positions(&snap_user, &secret, &acct.id).await {
            positions_synced = positions_synced.saturating_add(positions.len() as u32);
        }
        if let Ok(acts) = client.list_activities(&snap_user, &secret, &acct.id).await {
            activities_synced = activities_synced.saturating_add(acts.len() as u32);
        }
    }
    Ok(Json(SyncSummary {
        accounts_synced: accounts.len() as u32,
        positions_synced,
        activities_synced,
    }))
}

/// Common gate: confirm SnapTrade is configured, hydrate the client,
/// and resolve the signed-in Mizan user → SnapTrade (userId, userSecret).
async fn resolve(
    state: &AppState,
    mizan_user_id: &uuid::Uuid,
) -> Result<(SnapTradeClient, String, String), AppError> {
    let cfg = state.config().snaptrade.as_ref().ok_or_else(|| {
        AppError::new(
            ErrorCode::ServiceUnavailable,
            "SnapTrade not configured".to_string(),
        )
    })?;
    let client = SnapTradeClient::new(cfg);
    let (uid, secret) =
        repository::load_user_credentials(state, mizan_user_id, &cfg.token_encryption_key)
            .await?
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::UnprocessableEntity,
                    "SnapTrade not yet linked for this user — open the login portal first"
                        .to_string(),
                )
            })?;
    Ok((client, uid, secret))
}
