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
    PlaidHealthResponse, PlaidSyncRequest, PlaidSyncResponse, PlaidWebhookPayload, UpsertPlaidItem,
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
    let encrypted = plaid
        .token_cipher
        .encrypt(&token.access_token)
        .map_err(|err| {
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
    Ok(Json(
        repository::list_connections(state.db(), user.id).await?,
    ))
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
    let item_id_trimmed = item_id.trim();

    // GAP 12: Plaid /item/remove on disconnect so the upstream link is
    // actually severed, not just our local soft-delete. We try the
    // Plaid call first; if it fails (token already revoked, network
    // glitch, Plaid disabled) we log + audit and still proceed with
    // the local disconnect — the user's intent was to remove the
    // connection, and leaving them stuck because Plaid is slow would
    // be the worse failure mode. The audit row captures the outcome.
    let mut plaid_remove_outcome = "skipped";
    if let Some(plaid) = state.plaid() {
        match repository::fetch_item(state.db(), user.id, item_id_trimmed).await {
            Ok(stored) => match plaid.token_cipher.decrypt(&stored.access_token_encrypted) {
                Ok(token) => match plaid.client.item_remove(&token).await {
                    Ok(()) => plaid_remove_outcome = "revoked",
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            item_id = item_id_trimmed,
                            "Plaid /item/remove failed; proceeding with local disconnect"
                        );
                        plaid_remove_outcome = "remove_failed";
                    }
                },
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        item_id = item_id_trimmed,
                        "Plaid token decrypt failed during disconnect"
                    );
                    plaid_remove_outcome = "decrypt_failed";
                }
            },
            Err(_) => {
                // Item not found (or already disconnected) — the local
                // soft-delete below handles the 404 anyway.
                plaid_remove_outcome = "not_found";
            }
        }
    }

    repository::disconnect_item(state.db(), user.id, item_id_trimmed).await?;
    audit::record_event(
        state.db(),
        audit::AuditEvent::new("plaid.connect.disconnected")
            .user(user.id)
            .data(&json!({
                "item_id": item_id,
                "plaid_remove": plaid_remove_outcome,
            })),
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
    let items = if let Some(item_id) = req
        .item_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
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

        let response =
            sync_one_item(state.clone(), plaid, user.id, &item.item_id, &access_token).await?;
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

    // Plaid-2 / GAP 1: pull the investment-transaction feed so the
    // desktop has trade-level traceability behind the holdings snapshot.
    // Window strategy:
    //   - First sync (no `investment_transactions_sync_through`): 24
    //     months of backfill — Plaid's documented historical limit
    //     varies per institution, but 24 months is a reasonable cap
    //     that won't exhaust per-account API budgets.
    //   - Incremental: from (prior_synced_through - 7d safety overlap)
    //     to today. The overlap catches broker-side amendments Plaid
    //     backdates; the ON CONFLICT upsert handles re-seen rows.
    let investment_transactions_synced =
        match sync_investment_transactions(&state, plaid, user_id, item_id, access_token).await {
            Ok(count) => count,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    item_id = item_id,
                    "Plaid investment transactions sync skipped"
                );
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
        investment_transactions_synced,
    })
}

/// Per-item investment-transaction sync helper. Decides the date window,
/// drains pages until `total_investment_transactions` is exhausted, then
/// stamps `investment_transactions_sync_through` so the next incremental
/// run narrows its window. We intentionally cap the page loop at a hard
/// safety limit (PAGE_SAFETY_CAP) so a runaway Plaid response can never
/// burn through the API budget — if the cap is hit, we log and return
/// the partial count.
async fn sync_investment_transactions(
    state: &AppState,
    plaid: &super::types::PlaidContext,
    user_id: uuid::Uuid,
    item_id: &str,
    access_token: &secrecy::SecretString,
) -> Result<usize, AppError> {
    use time::{macros::format_description, Duration};

    const PAGE_SIZE: u32 = 500;
    const PAGE_SAFETY_CAP: u32 = 200; // 200 pages × 500 = 100k tx hard ceiling
    const INITIAL_BACKFILL_DAYS: i64 = 730; // ~24 months
    const INCREMENTAL_OVERLAP_DAYS: i64 = 7;
    let ymd = format_description!("[year]-[month]-[day]");

    let now = time::OffsetDateTime::now_utc();
    let prior_through =
        repository::investment_transactions_sync_through(state.db(), user_id, item_id).await?;

    let start = match prior_through {
        // Incremental: rewind 7 days for safety overlap.
        Some(t) => t - Duration::days(INCREMENTAL_OVERLAP_DAYS),
        // Initial: 24-month backfill.
        None => now - Duration::days(INITIAL_BACKFILL_DAYS),
    };
    let end = now;

    let start_str = start.format(ymd).map_err(|err| {
        tracing::error!(error = %err, "Failed to format investment_transactions start_date");
        AppError::internal("Plaid sync window could not be encoded")
    })?;
    let end_str = end.format(ymd).map_err(|err| {
        tracing::error!(error = %err, "Failed to format investment_transactions end_date");
        AppError::internal("Plaid sync window could not be encoded")
    })?;

    tracing::debug!(
        item_id = item_id,
        plaid.start_date = %start_str,
        plaid.end_date = %end_str,
        plaid.kind = if prior_through.is_some() { "incremental" } else { "initial-backfill" },
        "Plaid investments/transactions/get window"
    );

    let mut offset: u32 = 0;
    let mut total_written = 0usize;
    let mut pages = 0u32;

    loop {
        if pages >= PAGE_SAFETY_CAP {
            tracing::warn!(
                item_id = item_id,
                pages_drained = pages,
                total_written = total_written,
                "Investment transactions sync hit page safety cap; returning partial result"
            );
            break;
        }

        let page = plaid
            .client
            .investments_transactions_get(access_token, &start_str, &end_str, offset, PAGE_SIZE)
            .await?;

        // Upsert accounts + securities seen in this page so any new
        // account that landed mid-sync is also reflected in plaid_accounts.
        repository::upsert_accounts(state.db(), user_id, item_id, &page.accounts).await?;

        let returned = page.investment_transactions.len() as u32;
        let written = repository::store_investment_transactions(
            state.db(),
            user_id,
            item_id,
            &page.investment_transactions,
            end,
        )
        .await?;
        total_written += written;

        offset += returned;
        pages += 1;

        // Plaid signals the end of the window by either returning fewer
        // rows than PAGE_SIZE or by having offset >= total. The latter is
        // authoritative; the former is the early-out for the last page.
        if returned == 0 || offset >= page.total_investment_transactions {
            break;
        }
    }

    Ok(total_written)
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
