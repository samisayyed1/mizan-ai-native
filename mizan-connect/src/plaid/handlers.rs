use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::json;
use time::OffsetDateTime;

use crate::audit;
use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::state::AppState;

use super::repository;
use super::types::{
    ExchangePublicTokenRequest, ExchangePublicTokenResponse, LinkTokenRequest, LinkTokenResponse,
    PlaidHealthResponse, PlaidSyncRequest, PlaidSyncResponse, PlaidWebhookPayload,
    UpsertPlaidItem,
};
use super::webhook_verifier::{self, WebhookVerifyError};

/// Minimum interval between manual `/sync` invocations against the same item.
const SYNC_COOLDOWN_SECONDS: i64 = 60;

const PLAID_VERIFICATION_HEADER: &str = "Plaid-Verification";

fn plaid_unavailable() -> AppError {
    AppError::service_unavailable("Plaid is not configured on this Mizan Connect server")
}

pub async fn health(State(state): State<AppState>) -> Json<PlaidHealthResponse> {
    let plaid = state.plaid();
    Json(PlaidHealthResponse {
        configured: plaid.is_some(),
        environment: plaid.map(|ctx| ctx.client.environment().as_str().to_string()),
        message: if plaid.is_some() {
            "Plaid is configured for Gold live sync".to_string()
        } else {
            "Plaid environment variables are missing; live sync is disabled".to_string()
        },
    })
}

pub async fn create_link_token(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<LinkTokenRequest>,
) -> Result<Json<LinkTokenResponse>, AppError> {
    let plaid = state.plaid().ok_or_else(plaid_unavailable)?;
    let response = plaid
        .client
        .create_link_token(&user.id.to_string(), req.redirect_uri.as_deref())
        .await?;
    Ok(Json(response))
}

pub async fn exchange_public_token(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<ExchangePublicTokenRequest>,
) -> Result<Json<ExchangePublicTokenResponse>, AppError> {
    if req.public_token.trim().is_empty() {
        return Err(AppError::bad_request("publicToken is required"));
    }

    let plaid = state.plaid().ok_or_else(plaid_unavailable)?;
    let token = plaid
        .client
        .exchange_public_token(req.public_token.trim())
        .await?;
    let encrypted = plaid.token_cipher.encrypt(&token.access_token).map_err(|err| {
        tracing::error!(error = %err, "Plaid access token encryption failed");
        AppError::internal("Plaid token could not be stored securely")
    })?;

    let accounts = plaid.client.accounts_get(&token.access_token).await?;
    repository::upsert_item(
        state.db(),
        UpsertPlaidItem {
            user_id: user.id,
            item_id: &token.item_id,
            access_token_encrypted: &encrypted,
            institution_id: accounts.item.institution_id.as_deref(),
            institution_name: accounts.item.institution_name.as_deref(),
        },
    )
    .await?;
    let accounts_synced =
        repository::upsert_accounts(state.db(), user.id, &token.item_id, &accounts.accounts)
            .await?;

    audit::record_event(
        state.db(),
        audit::AuditEvent::new("plaid.connect.completed")
            .user(user.id)
            .data(&json!({
                "item_id": token.item_id,
                "institution_id": accounts.item.institution_id,
                "institution_name": accounts.item.institution_name,
                "accounts_synced": accounts_synced,
            })),
    )
    .await
    .map_err(|err| AppError::internal("audit log write failed").with_source(err))?;

    Ok(Json(ExchangePublicTokenResponse {
        item_id: token.item_id,
        accounts_synced,
    }))
}

pub async fn list_connections(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<super::types::PlaidConnectionDto>>, AppError> {
    Ok(Json(repository::list_connections(state.db(), user.id).await?))
}

pub async fn list_accounts(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<super::types::PlaidAccountDto>>, AppError> {
    Ok(Json(repository::list_accounts(state.db(), user.id).await?))
}

pub async fn disconnect_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(item_id): Path<String>,
) -> Result<StatusCode, AppError> {
    repository::disconnect_item(state.db(), user.id, item_id.trim()).await?;
    audit::record_event(
        state.db(),
        audit::AuditEvent::new("plaid.connect.disconnected")
            .user(user.id)
            .data(&json!({ "item_id": item_id })),
    )
    .await
    .map_err(|err| AppError::internal("audit log write failed").with_source(err))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync_now(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<PlaidSyncRequest>,
) -> Result<Json<Vec<PlaidSyncResponse>>, AppError> {
    let plaid = state.plaid().ok_or_else(plaid_unavailable)?;
    let items = if let Some(item_id) = req.item_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        vec![repository::fetch_item(state.db(), user.id, item_id).await?]
    } else {
        repository::fetch_items(state.db(), user.id).await?
    };

    let now = OffsetDateTime::now_utc();
    for item in &items {
        if let Some(prev) =
            repository::record_sync_attempt(state.db(), user.id, &item.item_id).await?
        {
            let elapsed = (now - prev).whole_seconds();
            if (0..SYNC_COOLDOWN_SECONDS).contains(&elapsed) {
                let retry_after = SYNC_COOLDOWN_SECONDS - elapsed;
                return Err(AppError::too_many_requests(format!(
                    "Plaid sync is throttled; try again in {retry_after}s"
                )));
            }
        }
    }

    let mut responses = Vec::with_capacity(items.len());
    for item in items {
        let access_token = match plaid.token_cipher.decrypt(&item.access_token_encrypted) {
            Ok(token) => token,
            Err(err) => {
                tracing::error!(error = %err, item_id = %item.item_id, "Plaid token decrypt failed");
                repository::mark_item_error(
                    state.db(),
                    user.id,
                    &item.item_id,
                    "stored Plaid token could not be decrypted",
                )
                .await?;
                continue;
            }
        };

        let response = sync_one_item(state.clone(), plaid, user.id, &item.item_id, &access_token)
            .await?;
        responses.push(response);
    }

    Ok(Json(responses))
}

async fn sync_one_item(
    state: AppState,
    plaid: &super::types::PlaidContext,
    user_id: uuid::Uuid,
    item_id: &str,
    access_token: &secrecy::SecretString,
) -> Result<PlaidSyncResponse, AppError> {
    let accounts = plaid.client.accounts_get(access_token).await?;
    let accounts_synced =
        repository::upsert_accounts(state.db(), user_id, item_id, &accounts.accounts).await?;

    let mut cursor = repository::transaction_cursor(state.db(), user_id, item_id).await?;
    let mut transactions_added = 0;
    let mut transactions_modified = 0;
    let mut transactions_removed = 0;
    for _ in 0..20 {
        let page = plaid
            .client
            .transactions_sync(access_token, cursor.as_deref())
            .await?;
        transactions_added += page.added.len();
        transactions_modified += page.modified.len();
        transactions_removed += page.removed.len();
        repository::store_transactions(
            state.db(),
            user_id,
            item_id,
            &page.added,
            &page.modified,
            &page.removed,
            &page.next_cursor,
        )
        .await?;
        cursor = Some(page.next_cursor);
        if !page.has_more {
            break;
        }
    }

    let liabilities_synced = match plaid.client.liabilities_get(access_token).await {
        Ok(payload) => {
            repository::upsert_accounts(state.db(), user_id, item_id, &payload.accounts).await?;
            repository::store_liabilities(state.db(), user_id, item_id, &payload.liabilities)
                .await?
        }
        Err(err) => {
            tracing::warn!(error = %err, item_id = item_id, "Plaid liabilities sync skipped");
            0
        }
    };

    let holdings_synced = match plaid.client.investments_holdings_get(access_token).await {
        Ok(payload) => {
            repository::upsert_accounts(state.db(), user_id, item_id, &payload.accounts).await?;
            repository::replace_holdings(
                state.db(),
                user_id,
                item_id,
                &payload.holdings,
                &payload.securities,
            )
            .await?
        }
        Err(err) => {
            tracing::warn!(error = %err, item_id = item_id, "Plaid holdings sync skipped");
            0
        }
    };

    Ok(PlaidSyncResponse {
        item_id: item_id.to_string(),
        accounts_synced,
        transactions_added,
        transactions_modified,
        transactions_removed,
        liabilities_synced,
        holdings_synced,
    })
}

pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let plaid = state.plaid().ok_or_else(plaid_unavailable)?;

    let header_value = headers
        .get(PLAID_VERIFICATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("missing Plaid-Verification header"))?;

    match webhook_verifier::verify(&plaid.client, &plaid.webhook_keys, header_value, &body).await {
        Ok(()) => {}
        Err(err) => {
            let status_msg: &'static str = match err {
                WebhookVerifyError::MissingHeader => "missing Plaid-Verification header",
                WebhookVerifyError::MalformedHeader => "malformed Plaid-Verification header",
                WebhookVerifyError::MissingKid => "missing key id in verification token",
                WebhookVerifyError::KeyFetch => "plaid verification key unavailable",
                WebhookVerifyError::BadSignature => "invalid webhook signature",
                WebhookVerifyError::BodyHashMismatch => "webhook body hash mismatch",
                WebhookVerifyError::Expired => "webhook verification token expired",
            };
            tracing::warn!(error = ?err, "plaid webhook rejected");
            return Err(AppError::unauthorized(status_msg));
        }
    }

    // Parse-first: a malformed body is a client bug — return 400 so Plaid
    // does not retry (Plaid retries 5xx, not 4xx). `item_id` is required for
    // any meaningful processing.
    let payload: PlaidWebhookPayload = serde_json::from_slice(&body)
        .map_err(|err| AppError::bad_request(format!("malformed webhook payload: {err}")))?;
    let item_id = payload
        .item_id
        .as_deref()
        .ok_or_else(|| AppError::bad_request("webhook payload missing item_id"))?;
    let webhook_type = payload.webhook_type.as_deref().unwrap_or("UNKNOWN");
    let webhook_code = payload.webhook_code.as_deref().unwrap_or("UNKNOWN");

    tracing::info!(
        plaid.webhook_type = webhook_type,
        plaid.webhook_code = webhook_code,
        plaid.item_id = item_id,
        "Plaid webhook received"
    );

    sqlx::query(
        r#"
        INSERT INTO plaid_webhook_events (item_id, webhook_type, webhook_code, payload_json)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(item_id)
    .bind(payload.webhook_type.as_deref())
    .bind(payload.webhook_code.as_deref())
    .bind(serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null))
    .execute(state.db())
    .await?;

    let event_type = format!("plaid.webhook.{}", webhook_type.to_ascii_lowercase());
    let _ = audit::record_event(
        state.db(),
        audit::AuditEvent::new(&event_type).data(&json!({
            "item_id": item_id,
            "webhook_type": webhook_type,
            "webhook_code": webhook_code,
        })),
    )
    .await;

    Ok(StatusCode::ACCEPTED)
}
