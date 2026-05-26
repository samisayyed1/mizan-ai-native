//! Commands for syncing broker data from the cloud API.

use log::{debug, error, info};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::context::ServiceContext;
use crate::events::{BROKER_SYNC_COMPLETE, BROKER_SYNC_ERROR, BROKER_SYNC_START};
use mizan_connect::{
    broker::BrokerApiClient, fetch_subscription_plans_public, BrokerAccount, BrokerConnection,
    PlaidExchangePublicTokenResponse, PlaidLinkTokenResponse, PlansResponse, Platform,
    SyncProgressPayload, SyncProgressReporter, SyncResult, UserInfo,
};

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Progress Reporter
// ─────────────────────────────────────────────────────────────────────────────

/// Progress reporter that emits Tauri events.
struct TauriProgressReporter {
    app_handle: AppHandle,
}

impl TauriProgressReporter {
    fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl SyncProgressReporter for TauriProgressReporter {
    fn report_progress(&self, payload: SyncProgressPayload) {
        if let Err(e) = self.app_handle.emit("sync-progress", &payload) {
            debug!("Failed to emit sync-progress event: {}", e);
        }
    }

    fn report_sync_start(&self) {
        self.app_handle
            .emit(BROKER_SYNC_START, ())
            .unwrap_or_else(|e| {
                error!("Failed to emit broker:sync-start event: {}", e);
            });
    }

    fn report_sync_complete(&self, result: &SyncResult) {
        if result.success {
            self.app_handle
                .emit(BROKER_SYNC_COMPLETE, result)
                .unwrap_or_else(|e| {
                    error!("Failed to emit broker:sync-complete event: {}", e);
                });
        } else {
            self.app_handle
                .emit(
                    BROKER_SYNC_ERROR,
                    serde_json::json!({ "error": result.message }),
                )
                .unwrap_or_else(|e| {
                    error!("Failed to emit broker:sync-error event: {}", e);
                });
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Broker Sync Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Sync broker data from the cloud API (non-blocking with SSE events).
/// Returns immediately after triggering the sync. Results are delivered via events:
/// - `broker:sync-start` - emitted when sync begins
/// - `broker:sync-complete` - emitted with SyncResult payload on success
/// - `broker:sync-error` - emitted with error message on failure
#[tauri::command]
pub async fn sync_broker_data(
    app: AppHandle,
    state: State<'_, Arc<ServiceContext>>,
    rate_limiter: State<'_, Arc<crate::rate_limit::RateLimiter>>,
) -> Result<(), String> {
    // Rate-limit guard. Plaid live sync is external API work; a frontend
    // loop firing this 100×/sec would create avoidable upstream pressure.
    // Pre-registered override: 3 calls / 30s.
    if let crate::rate_limit::RateLimitDecision::Deny { retry_after } =
        rate_limiter.check("trigger_broker_sync")
    {
        return Err(format!(
            "Too many broker sync requests. Try again in {:.1}s.",
            retry_after.as_secs_f32()
        ));
    }

    // Check plan entitlement before starting sync. Returns a structured
    // GatedError the frontend turns into a contextual upgrade modal.
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.broker_sync,
        "broker_sync",
        "pro",
        &entitlements.plan,
        "Connect accounts with Plaid and keep your wealth picture alive — included with Mizan Gold.",
    )?;

    // Fire-and-forget usage report after passing the gate. Authoritative
    // count lives in the cloud ledger; failure here doesn't abort the sync.
    state.connect_service().report_usage("broker_poll", 1).await;

    info!("[Connect] Starting broker data sync ...");

    // Clone what we need for the spawned task
    let context = state.inner().clone();
    let app_handle = app.clone();

    // Spawn background task
    tauri::async_runtime::spawn(async move {
        match perform_broker_sync(&context, Some(&app_handle)).await {
            Ok(_result) => {
                info!("[Connect] Broker sync completed successfully");
                // Events are emitted by the orchestrator via TauriProgressReporter
            }
            Err(err) => {
                error!("[Connect] Broker sync failed: {}", err);
                // Error event also emitted by orchestrator
            }
        }
    });

    Ok(())
}

/// Alias for `sync_broker_data` using explicit broker-ingest vocabulary.
#[tauri::command]
pub async fn broker_ingest_run(
    app: AppHandle,
    state: State<'_, Arc<ServiceContext>>,
    rate_limiter: State<'_, Arc<crate::rate_limit::RateLimiter>>,
) -> Result<(), String> {
    sync_broker_data(app, state, rate_limiter).await
}

/// Core broker sync logic that can be called from Tauri command or scheduler.
///
/// This function is public so the scheduler can call it directly.
/// FX rate registration is handled automatically by AccountService during account creation.
///
/// # Arguments
///
/// * `context` - Service context
/// * `app` - Optional AppHandle for progress reporting. If None, progress events are not emitted.
pub async fn perform_broker_sync(
    context: &Arc<ServiceContext>,
    app: Option<&AppHandle>,
) -> Result<SyncResult, String> {
    info!("Starting Plaid live sync...");

    if let Some(app_handle) = app {
        let reporter = TauriProgressReporter::new(app_handle.clone());
        reporter.report_sync_start();
    }

    let client = context.connect_service().get_api_client().await?;
    client.sync_plaid_data().await.map_err(|e| e.to_string())?;

    // Plaid-4: after the cloud finishes its Plaid pull, ingest the new
    // investment transactions into the local activities table. This is
    // what makes trades show up in the timeline + flow into TWR / cost
    // basis. Failure is non-fatal: we log + continue, because the cloud
    // sync itself succeeded and the next ingest attempt will catch up.
    let ingest_msg = match mizan_connect::plaid_ingest::ingest_plaid_investment_transactions(
        &client,
        context.activity_service(),
        context.account_service(),
        None, // server-side cursor handles the incremental window
        Some(1000),
    )
    .await
    {
        Ok(s) => {
            info!(
                "Plaid ingest: created={} updated={} skipped={} accounts={} needs_review={}",
                s.created,
                s.updated,
                s.skipped,
                s.accounts_touched.len(),
                s.mapping.needs_review
            );
            let mut parts: Vec<String> = Vec::new();
            if s.created > 0 {
                parts.push(format!("{} new activities", s.created));
            }
            if s.updated > 0 {
                parts.push(format!("{} updated", s.updated));
            }
            if s.mapping.needs_review > 0 {
                parts.push(format!("{} need review", s.mapping.needs_review));
            }
            if parts.is_empty() {
                "Plaid live sync completed.".to_string()
            } else {
                format!("Plaid live sync completed — {}.", parts.join(", "))
            }
        }
        Err(e) => {
            error!("Plaid ingest failed (non-fatal): {}", e);
            "Plaid live sync completed (activity ingest will retry on next sync).".to_string()
        }
    };

    let result = SyncResult {
        success: true,
        message: ingest_msg,
        connections_synced: None,
        accounts_synced: None,
        activities_synced: None,
        holdings_synced: None,
        new_accounts: None,
    };

    if let Some(app_handle) = app {
        let reporter = TauriProgressReporter::new(app_handle.clone());
        reporter.report_sync_complete(&result);
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Account and Platform Queries
// ─────────────────────────────────────────────────────────────────────────────

/// Get all synced accounts
#[tauri::command]
pub async fn get_synced_accounts(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_core::accounts::Account>, String> {
    state
        .sync_service()
        .get_synced_accounts()
        .map_err(|e| format!("Failed to get synced accounts: {}", e))
}

/// Get all platforms
#[tauri::command]
pub async fn get_platforms(state: State<'_, Arc<ServiceContext>>) -> Result<Vec<Platform>, String> {
    state
        .sync_service()
        .get_platforms()
        .map_err(|e| format!("Failed to get platforms: {}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
// Broker Connection Management Commands
// ─────────────────────────────────────────────────────────────────────────────

/// List broker connections from the cloud API
#[tauri::command]
pub async fn list_broker_connections(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<BrokerConnection>, String> {
    debug!("Fetching broker connections from cloud API...");

    let client = state.connect_service().get_api_client().await?;
    let connections = client.list_connections().await.map_err(|e| e.to_string())?;

    Ok(connections)
}

/// Create a short-lived Plaid Link token for Gold live sync.
#[tauri::command]
pub async fn create_plaid_link_token(
    redirect_uri: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<PlaidLinkTokenResponse, String> {
    info!("Creating Plaid Link token");

    let client = state.connect_service().get_api_client().await?;
    client
        .create_plaid_link_token(redirect_uri.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Exchange a Plaid public token for an encrypted server-side access token.
#[tauri::command]
pub async fn exchange_plaid_public_token(
    public_token: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<PlaidExchangePublicTokenResponse, String> {
    let client = state.connect_service().get_api_client().await?;
    client
        .exchange_plaid_public_token(&public_token)
        .await
        .map_err(|e| e.to_string())
}

/// Disconnect a Plaid Item.
///
/// Calls the cloud API's `DELETE /api/v1/sync/plaid/connections/:id`.
/// The backend soft-deletes the Plaid Item row. Frontend should invalidate
/// the BROKER_CONNECTIONS + BROKER_ACCOUNTS query keys after success so
/// the disconnected broker disappears from the UI without a refresh.
///
/// Note: this does NOT remove already-synced rows from the local SQLite
/// (accounts, holdings snapshots, activity history). Those are kept as
/// historical records — disconnecting a broker doesn't erase your past.
#[tauri::command]
pub async fn delete_broker_connection(
    connection_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    info!("Disconnecting broker connection: {}", connection_id);

    let client = state.connect_service().get_api_client().await?;
    client
        .delete_connection(&connection_id)
        .await
        .map_err(|e| e.to_string())
}

/// List broker accounts from the cloud API
/// Returns the live account data including sync_enabled and owner info
#[tauri::command]
pub async fn list_broker_accounts(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<BrokerAccount>, String> {
    debug!("Fetching broker accounts from cloud API...");

    let client = state.connect_service().get_api_client().await?;
    let accounts = client
        .list_accounts(None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(accounts)
}

// ─────────────────────────────────────────────────────────────────────────────
// User & Subscription Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Get subscription plans from the cloud API (requires authentication)
#[tauri::command]
pub async fn get_subscription_plans(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<PlansResponse, String> {
    debug!("Fetching subscription plans from cloud API...");

    let client = state.connect_service().get_api_client().await?;
    match client.get_subscription_plans().await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Failed to get subscription plans: {}", e);
            Err(e.to_string())
        }
    }
}

/// Get subscription plans from the cloud API (public, no authentication required)
#[tauri::command]
pub async fn get_subscription_plans_public() -> Result<PlansResponse, String> {
    debug!("Fetching subscription plans from cloud API (public)...");

    let base_url = crate::services::cloud_api_base_url().ok_or_else(|| {
        "Cloud API base URL is unavailable. Connect API operations are disabled.".to_string()
    })?;

    match fetch_subscription_plans_public(&base_url).await {
        Ok(response) => {
            debug!("Found {} subscription plans (public)", response.plans.len());
            Ok(response)
        }
        Err(e) => {
            error!("Failed to get subscription plans (public): {}", e);
            Err(e.to_string())
        }
    }
}

/// Get current user info from the cloud API
#[tauri::command]
pub async fn get_user_info(state: State<'_, Arc<ServiceContext>>) -> Result<UserInfo, String> {
    debug!("Fetching user info from cloud API...");

    let client = state.connect_service().get_api_client().await?;
    match client.get_user_info().await {
        Ok(user_info) => Ok(user_info),
        Err(e) => {
            error!("Failed to get user info: {}", e);
            Err(e.to_string())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sync State and Import Run Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Get all broker sync states
#[tauri::command]
pub async fn get_broker_sync_states(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_connect::BrokerSyncState>, String> {
    debug!("Fetching all broker sync states...");
    state
        .sync_service()
        .get_all_sync_states()
        .map_err(|e| format!("Failed to get broker sync states: {}", e))
}

/// Alias for `get_broker_sync_states` using explicit broker-ingest vocabulary.
#[tauri::command]
pub async fn get_broker_ingest_states(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_connect::BrokerSyncState>, String> {
    get_broker_sync_states(state).await
}

/// Get import runs with optional type filter and pagination
#[tauri::command]
pub async fn get_import_runs(
    run_type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_connect::ImportRun>, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    debug!(
        "Fetching import runs (type={:?}, limit={}, offset={})...",
        run_type, limit, offset
    );
    state
        .sync_service()
        .get_import_runs(run_type.as_deref(), limit, offset)
        .map_err(|e| format!("Failed to get import runs: {}", e))
}

/// Alias for `get_import_runs` using neutral terminology, because runs include
/// both broker ingest and manual CSV import operations.
#[tauri::command]
pub async fn get_data_import_runs(
    run_type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_connect::ImportRun>, String> {
    get_import_runs(run_type, limit, offset, state).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Broker Sync Profile Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Get broker sync profile (rules learned from sync)
#[tauri::command]
pub async fn get_broker_sync_profile(
    account_id: String,
    source_system: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<mizan_core::activities::BrokerSyncProfileData, String> {
    log::debug!(
        "Getting broker sync profile for account: {}, source: {}",
        account_id,
        source_system
    );
    state
        .activity_service()
        .get_broker_sync_profile(account_id, source_system)
        .map_err(|e| e.to_string())
}

/// Save broker sync profile rules (learned corrections)
#[tauri::command]
pub async fn save_broker_sync_profile_rules(
    request: mizan_core::activities::SaveBrokerSyncProfileRulesRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<mizan_core::activities::BrokerSyncProfileData, String> {
    log::debug!(
        "Saving broker sync profile rules for account: {}, source: {}",
        request.account_id,
        request.source_system
    );
    state
        .activity_service()
        .save_broker_sync_profile_rules(request)
        .await
        .map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Foreground Sync Command
// ─────────────────────────────────────────────────────────────────────────────
