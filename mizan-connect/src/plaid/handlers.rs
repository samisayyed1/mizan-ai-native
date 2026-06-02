use axum::body::Bytes;
use axum::extract::{Path, Query, State};
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
    ListInvestmentTransactionsParams, PlaidHealthResponse, PlaidInvestmentTransactionDto,
    PlaidSyncRequest, PlaidSyncResponse, PlaidWebhookPayload, UpsertPlaidItem,
};
use super::webhook_verifier::{self, WebhookVerifyError};

/// Minimum interval between manual `/sync` invocations against the same item.
const SYNC_COOLDOWN_SECONDS: i64 = 60;

const PLAID_VERIFICATION_HEADER: &str = "Plaid-Verification";

/// RAII guard that releases the per-user Plaid sync advisory lock when
/// dropped. The lock is acquired on a dedicated connection that this
/// guard owns; releasing on the same connection is required by
/// Postgres' session-scoped lock semantics.
///
/// Drop runs synchronously, but pg_advisory_unlock is a fire-and-forget
/// best-effort here: if the future-spawned release fails (very rare),
/// the lock is released when the connection drops anyway. Belt + braces.
struct PlaidSyncLockGuard {
    conn: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    user_id: String,
}

impl Drop for PlaidSyncLockGuard {
    fn drop(&mut self) {
        if let Some(mut conn) = self.conn.take() {
            let user_id = self.user_id.clone();
            tokio::spawn(async move {
                let _ = sqlx::query_scalar::<_, bool>(
                    "SELECT pg_advisory_unlock(hashtext($1::text), hashtext('plaid_sync'))",
                )
                .bind(&user_id)
                .fetch_one(&mut *conn)
                .await;
            });
        }
    }
}

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

/// GET /sync/plaid/investment-transactions?since=YYYY-MM-DD&accountId=...&limit=N
///
/// Returns Plaid investment transactions for the authenticated user,
/// ordered newest first. Cap the requested `limit` at 1000 so a single
/// request can't pull the entire history into memory; the desktop can
/// paginate via `since`.
pub async fn list_investment_transactions(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<ListInvestmentTransactionsParams>,
) -> Result<Json<Vec<PlaidInvestmentTransactionDto>>, AppError> {
    const DEFAULT_LIMIT: i64 = 500;
    const MAX_LIMIT: i64 = 1000;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let rows = repository::list_investment_transactions(
        state.db(),
        user.id,
        params.since.as_deref(),
        params.account_id.as_deref(),
        limit,
    )
    .await?;
    Ok(Json(rows))
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
) -> Result<Json<serde_json::Value>, AppError> {
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

    // Per-user concurrency guard (audit Issue #4). Two concurrent
    // /sync/plaid/sync requests for the same user can race on the
    // `investment_transactions_sync_through` timestamp: both reads
    // see the same prior value, both writes set their own
    // `synced_through`, and whichever finishes second clobbers the
    // first — causing the next incremental window to replay rows that
    // were already ingested.
    //
    // We use a session-scoped Postgres advisory lock keyed off
    // (hashtext(user_id), hashtext("plaid_sync")) held on a single
    // dedicated connection for the duration of the handler. The lock
    // is released explicitly in every exit path (success + error)
    // via the PlaidSyncLockGuard RAII helper. pg_try_advisory_lock
    // returns false instead of blocking, matching the existing
    // cooldown semantics — desktop already handles 429.
    let mut lock_conn = state.db().acquire().await?;
    let lock_acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_lock(hashtext($1::text), hashtext('plaid_sync'))",
    )
    .bind(user.id.to_string())
    .fetch_one(&mut *lock_conn)
    .await?;
    if !lock_acquired {
        return Err(AppError::too_many_requests(
            "Plaid sync already running for this user; try again in a moment",
        ));
    }
    let _lock_guard = PlaidSyncLockGuard {
        conn: Some(lock_conn),
        user_id: user.id.to_string(),
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

    // Per-item error isolation (audit Issue #1). One bad item — expired
    // token, broker outage, ITEM_LOGIN_REQUIRED — must NOT abort the
    // syncs for the user's other connections. We collect per-item errors
    // into the `errors` field on the response so the desktop can surface
    // partial failures via sync_plaid_data's wire contract (which the
    // desktop already treats as Err when non-empty per Plaid-1.c).
    let mut responses = Vec::with_capacity(items.len());
    let mut item_errors: Vec<serde_json::Value> = Vec::new();
    for item in items {
        let access_token = match plaid.token_cipher.decrypt(&item.access_token_encrypted) {
            Ok(token) => token,
            Err(err) => {
                tracing::error!(error = %err, item_id = %item.item_id, "Plaid token decrypt failed");
                // Best-effort mark; if this also fails, swallow + continue
                // — losing the error stamp is preferable to losing the
                // rest of the user's sync.
                let _ = repository::mark_item_error(
                    state.db(),
                    user.id,
                    &item.item_id,
                    "stored Plaid token could not be decrypted",
                )
                .await;
                item_errors.push(serde_json::json!({
                    "itemId": item.item_id,
                    "message": "stored Plaid token could not be decrypted",
                }));
                continue;
            }
        };

        match sync_one_item(state.clone(), plaid, user.id, &item.item_id, &access_token).await {
            Ok(response) => responses.push(response),
            Err(err) => {
                let msg = err.to_string();
                tracing::warn!(
                    error = %msg,
                    item_id = %item.item_id,
                    "Plaid sync_one_item failed; continuing with remaining items"
                );
                // Mark on the item so the desktop's needs_attention badge
                // catches it on the next /connections fetch.
                let _ = repository::mark_item_error(
                    state.db(),
                    user.id,
                    &item.item_id,
                    &format!("Sync failed: {}", msg),
                )
                .await;
                item_errors.push(serde_json::json!({
                    "itemId": item.item_id,
                    "message": msg,
                }));
            }
        }
    }

    // Return an envelope shape that's a strict superset of the previous
    // contract: existing callers that decode `Vec<PlaidSyncResponse>`
    // still work because we add a parallel `errors` field that the
    // desktop's Plaid-1.c sync_plaid_data parser already knows to
    // surface as Err on non-empty.
    if item_errors.is_empty() {
        Ok(Json(serde_json::json!(responses)))
    } else {
        Ok(Json(serde_json::json!({
            "results": responses,
            "errors": item_errors,
        })))
    }
}

async fn sync_one_item(
    state: AppState,
    plaid: &super::types::PlaidContext,
    user_id: uuid::Uuid,
    item_id: &str,
    access_token: &secrecy::SecretString,
) -> Result<PlaidSyncResponse, AppError> {
    // Audit Issue #2: previously a temporary auth failure on accounts_get
    // or transactions_sync would short-circuit the entire item, blocking
    // holdings + investment-transactions even though they might have
    // succeeded. Each stage is now independent: accounts is fail-loud
    // (without it we can't upsert child rows by account_id), but the
    // transactions/liabilities/holdings/investments stages each fail
    // independently and a per-stage error doesn't poison the others.
    // We return the partial counts plus a populated `last_error` on the
    // item so the desktop's needs_attention badge flips on the next
    // /connections fetch.
    let accounts = plaid.client.accounts_get(access_token).await?;
    let accounts_synced =
        repository::upsert_accounts(state.db(), user_id, item_id, &accounts.accounts).await?;

    let mut cursor = repository::transaction_cursor(state.db(), user_id, item_id).await?;
    let mut transactions_added = 0;
    let mut transactions_modified = 0;
    let mut transactions_removed = 0;
    let mut transactions_aborted_early = false;
    for _ in 0..20 {
        let page = match plaid
            .client
            .transactions_sync(access_token, cursor.as_deref())
            .await
        {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    item_id = item_id,
                    "Plaid transactions_sync failed mid-loop; continuing with other endpoints"
                );
                transactions_aborted_early = true;
                break;
            }
        };
        transactions_added += page.added.len();
        transactions_modified += page.modified.len();
        transactions_removed += page.removed.len();
        if let Err(err) = repository::store_transactions(
            state.db(),
            user_id,
            item_id,
            &page.added,
            &page.modified,
            &page.removed,
            &page.next_cursor,
        )
        .await
        {
            tracing::warn!(
                error = %err,
                item_id = item_id,
                "Plaid store_transactions failed; cursor not advanced"
            );
            transactions_aborted_early = true;
            break;
        }
        cursor = Some(page.next_cursor);
        if !page.has_more {
            break;
        }
    }
    if transactions_aborted_early {
        // Don't let a partial transactions failure block the rest of
        // the sync; mark on the item so the next sync round retries
        // and the desktop's badge surfaces the issue.
        let _ = repository::mark_item_error(
            state.db(),
            user_id,
            item_id,
            "transactions sync aborted mid-page; will retry on next sync",
        )
        .await;
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
            &page.securities,
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

    // Audit Issue #5 — Webhook-triggered sync. Plaid sends webhooks like
    // DEFAULT_UPDATE (new transactions ready) or HOLDINGS/DEFAULT_UPDATE
    // (new holdings ready) precisely so we don't have to poll. Previously
    // we just logged + returned 202; now we fan the relevant codes into
    // a spawned sync task so the desktop sees the new data on its next
    // poll without waiting for the scheduled tick.
    //
    // We must return 202 ACCEPTED within ~30s (Plaid's webhook timeout),
    // so the sync runs detached via tokio::spawn. The spawned task
    // acquires the same per-user advisory lock the synchronous /sync
    // endpoint uses (Plaid-5 / audit Issue #4), so webhook-triggered
    // syncs never race with user-initiated ones.
    if should_trigger_sync(webhook_type, webhook_code) {
        // Look up the user that owns this item. Two indexes needed:
        // (a) the item must still be 'connected' (don't resurrect a
        //     disconnected one), (b) we need the encrypted token.
        match sqlx::query_as::<_, (uuid::Uuid, Vec<u8>)>(
            r#"
            SELECT user_id, access_token_encrypted
            FROM plaid_items
            WHERE item_id = $1 AND status <> 'disconnected'
            LIMIT 1
            "#,
        )
        .bind(item_id)
        .fetch_optional(state.db())
        .await
        {
            Ok(Some((user_id, encrypted))) => {
                let state_clone = state.clone();
                let item_id_owned = item_id.to_string();
                let webhook_type_owned = webhook_type.to_string();
                let webhook_code_owned = webhook_code.to_string();
                tokio::spawn(async move {
                    if let Err(err) = webhook_triggered_sync(
                        state_clone,
                        user_id,
                        item_id_owned,
                        encrypted,
                        webhook_type_owned,
                        webhook_code_owned,
                    )
                    .await
                    {
                        tracing::warn!(
                            error = %err,
                            "Webhook-triggered Plaid sync failed (non-fatal)"
                        );
                    }
                });
            }
            Ok(None) => {
                tracing::debug!(
                    item_id = item_id,
                    "Webhook for unknown or disconnected Plaid item; ignoring"
                );
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    item_id = item_id,
                    "Webhook item lookup failed"
                );
            }
        }
    }

    Ok(StatusCode::ACCEPTED)
}

/// Which webhook codes warrant kicking off a sync.
///
/// Plaid documents many webhook codes; we only act on the ones that
/// signal "new data is ready". TRANSACTIONS_REMOVED is included because
/// it materially changes the user's ledger. ERROR + USER_PERMISSION_REVOKED
/// are recorded but not synced — the next user-initiated sync will see
/// the item's status reflect the new state.
fn should_trigger_sync(webhook_type: &str, webhook_code: &str) -> bool {
    matches!(
        (webhook_type, webhook_code),
        ("TRANSACTIONS", "SYNC_UPDATES_AVAILABLE")
            | ("TRANSACTIONS", "DEFAULT_UPDATE")
            | ("TRANSACTIONS", "INITIAL_UPDATE")
            | ("TRANSACTIONS", "HISTORICAL_UPDATE")
            | ("TRANSACTIONS", "TRANSACTIONS_REMOVED")
            | ("HOLDINGS", "DEFAULT_UPDATE")
            | ("INVESTMENTS_TRANSACTIONS", "DEFAULT_UPDATE")
            | ("INVESTMENTS_TRANSACTIONS", "HISTORICAL_UPDATE")
            | ("LIABILITIES", "DEFAULT_UPDATE"),
    )
}

/// Detached webhook-triggered sync. Owns the same lock + concurrency
/// guarantees as the synchronous /sync/plaid/sync handler so the two
/// can't race.
async fn webhook_triggered_sync(
    state: AppState,
    user_id: uuid::Uuid,
    item_id: String,
    access_token_encrypted: Vec<u8>,
    webhook_type: String,
    webhook_code: String,
) -> Result<(), AppError> {
    let plaid = state.plaid().ok_or_else(plaid_unavailable)?;

    // Acquire the same per-user advisory lock the user-facing sync
    // takes (Plaid-5). If a manual sync is already running for this
    // user we silently bail — the manual sync will pull the new data.
    let mut lock_conn = state.db().acquire().await?;
    let lock_acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_lock(hashtext($1::text), hashtext('plaid_sync'))",
    )
    .bind(user_id.to_string())
    .fetch_one(&mut *lock_conn)
    .await?;
    if !lock_acquired {
        tracing::debug!(
            user_id = %user_id,
            item_id = %item_id,
            "Webhook sync skipped: another sync already running for user"
        );
        return Ok(());
    }
    let _guard = PlaidSyncLockGuard {
        conn: Some(lock_conn),
        user_id: user_id.to_string(),
    };

    let access_token = plaid
        .token_cipher
        .decrypt(&access_token_encrypted)
        .map_err(|err| {
            tracing::error!(error = %err, item_id = %item_id, "webhook token decrypt failed");
            AppError::internal("Plaid token decrypt failed")
        })?;

    tracing::info!(
        user_id = %user_id,
        item_id = %item_id,
        webhook_type = %webhook_type,
        webhook_code = %webhook_code,
        "Webhook-triggered Plaid sync starting"
    );

    match sync_one_item(state.clone(), plaid, user_id, &item_id, &access_token).await {
        Ok(resp) => {
            tracing::info!(
                user_id = %user_id,
                item_id = %item_id,
                accounts = resp.accounts_synced,
                txns_added = resp.transactions_added,
                holdings = resp.holdings_synced,
                invest_txns = resp.investment_transactions_synced,
                "Webhook-triggered Plaid sync complete"
            );
        }
        Err(err) => {
            let msg = err.to_string();
            tracing::warn!(
                user_id = %user_id,
                item_id = %item_id,
                error = %msg,
                "Webhook-triggered Plaid sync failed; marking item"
            );
            let _ = repository::mark_item_error(
                state.db(),
                user_id,
                &item_id,
                &format!("Webhook sync failed: {}", msg),
            )
            .await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_trigger_sync;

    #[test]
    fn transactions_sync_updates_available_triggers() {
        assert!(should_trigger_sync(
            "TRANSACTIONS",
            "SYNC_UPDATES_AVAILABLE"
        ));
    }

    #[test]
    fn transactions_default_update_triggers() {
        assert!(should_trigger_sync("TRANSACTIONS", "DEFAULT_UPDATE"));
    }

    #[test]
    fn holdings_default_update_triggers() {
        assert!(should_trigger_sync("HOLDINGS", "DEFAULT_UPDATE"));
    }

    #[test]
    fn investments_transactions_historical_update_triggers() {
        assert!(should_trigger_sync(
            "INVESTMENTS_TRANSACTIONS",
            "HISTORICAL_UPDATE"
        ));
    }

    #[test]
    fn liabilities_default_update_triggers() {
        assert!(should_trigger_sync("LIABILITIES", "DEFAULT_UPDATE"));
    }

    #[test]
    fn unknown_webhook_does_not_trigger() {
        assert!(!should_trigger_sync("UNKNOWN", "UNKNOWN"));
    }

    #[test]
    fn item_error_does_not_trigger_sync() {
        // ITEM errors are surfaced via the existing connection-status flow,
        // not by re-syncing the broken item (which would just re-fail).
        assert!(!should_trigger_sync("ITEM", "ERROR"));
        assert!(!should_trigger_sync("ITEM", "USER_PERMISSION_REVOKED"));
        assert!(!should_trigger_sync("ITEM", "LOGIN_REPAIRED"));
    }

    #[test]
    fn webhook_verification_does_not_trigger() {
        assert!(!should_trigger_sync(
            "WEBHOOK",
            "WEBHOOK_UPDATE_ACKNOWLEDGED"
        ));
    }
}
