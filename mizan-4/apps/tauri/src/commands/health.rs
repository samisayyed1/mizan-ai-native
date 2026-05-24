use std::sync::Arc;

use crate::context::ServiceContext;
use crate::events::{
    emit_portfolio_trigger_recalculate, MarketSyncResult, PortfolioRequestPayload,
    MARKET_SYNC_COMPLETE, MARKET_SYNC_ERROR, MARKET_SYNC_START,
};
use log::{debug, error, info, warn};
use mizan_core::health::{FixAction, HealthConfig, HealthServiceTrait, HealthStatus};
use mizan_core::quotes::MarketSyncMode;
use mizan_core::quotes::SyncMode;
use tauri::{AppHandle, Emitter, State};

/// Get current health status (cached or fresh check).
#[tauri::command]
pub async fn get_health_status(
    client_timezone: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HealthStatus, String> {
    debug!("Getting health status...");

    let health_service = state.health_service();

    // Try to get cached status first
    if let Some(status) = health_service.get_cached_status().await {
        if !status.is_stale {
            return Ok(status);
        }
    }

    // Run fresh checks
    let base_currency = state.get_base_currency();
    run_health_checks_internal(&state, &base_currency, client_timezone.as_deref()).await
}

/// Run health checks and return fresh status.
#[tauri::command]
pub async fn run_health_checks(
    client_timezone: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HealthStatus, String> {
    debug!("Running health checks...");
    let base_currency = state.get_base_currency();
    run_health_checks_internal(&state, &base_currency, client_timezone.as_deref()).await
}

/// Internal function to run health checks.
async fn run_health_checks_internal(
    state: &State<'_, Arc<ServiceContext>>,
    base_currency: &str,
    client_timezone: Option<&str>,
) -> Result<HealthStatus, String> {
    let configured_timezone = state.get_timezone();
    state
        .health_service()
        .run_full_checks(
            base_currency,
            state.account_service(),
            state.holdings_service(),
            state.quote_service(),
            state.asset_service(),
            state.taxonomy_service(),
            state.valuation_service(),
            Some(configured_timezone.as_str()),
            client_timezone,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Dismiss a health issue.
#[tauri::command]
pub async fn dismiss_health_issue(
    issue_id: String,
    data_hash: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    debug!("Dismissing health issue: {}", issue_id);
    state
        .health_service()
        .dismiss_issue(&issue_id, &data_hash)
        .await
        .map_err(|e| e.to_string())
}

/// Restore a dismissed health issue.
#[tauri::command]
pub async fn restore_health_issue(
    issue_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    debug!("Restoring health issue: {}", issue_id);
    state
        .health_service()
        .restore_issue(&issue_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get list of dismissed issue IDs.
#[tauri::command]
pub async fn get_dismissed_health_issues(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<String>, String> {
    debug!("Getting dismissed health issues...");
    state
        .health_service()
        .get_dismissed_ids()
        .await
        .map_err(|e| e.to_string())
}

/// Execute a fix action.
#[tauri::command]
pub async fn execute_health_fix(
    action: FixAction,
    app_handle: AppHandle,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    debug!("Executing health fix: {} ({})", action.label, action.id);

    // Handle classification migration actions specially since they need
    // the taxonomy service. The data-consistency check emits action.id =
    // "migrate_classifications"; the legacy-migration check emits
    // "migrate_legacy_classifications". Both route to the same shared
    // migration function — the previous TODO no-op in the core service
    // would silently log a warning and do nothing.
    if action.id == "migrate_legacy_classifications" || action.id == "migrate_classifications" {
        crate::commands::taxonomy::run_legacy_migration(&state).await?;
        state.health_service().clear_cache().await;
        return Ok(());
    }

    // Handle fetch_fx action — refresh stale/missing FX rates.
    //
    // Earlier attempt: call quote_service.sync(SyncMode::Incremental, None)
    // directly. That doesn't work for FX rates that have NEVER been
    // fetched — Incremental needs a last_quote_date to increment from,
    // and a fresh FX asset has none, so the sync silently skips it. The
    // same path also doesn't trigger the asset-resolution code that
    // creates FX-asset rows for new currency pairs in the first place.
    //
    // The proven path that actually refreshes FX rates end-to-end is the
    // one wired up to the user-facing "Recalculate History" button:
    // emit PORTFOLIO_TRIGGER_RECALCULATE with MarketSyncMode::BackfillHistory.
    // The portfolio orchestrator picks that up, resolves required currency
    // pairs, creates any missing FX assets, runs the quote-sync provider
    // chain (Yahoo for FX), and rebuilds valuations on top. Slightly
    // heavier than Incremental but it actually works.
    if action.id == "fetch_fx" {
        let pair_ids: Vec<String> = serde_json::from_value(action.payload.clone())
            .map_err(|e| format!("Failed to parse FX pair IDs: {}", e))?;

        info!(
            "Refreshing FX rates for {} pair(s): {:?} (via portfolio recalc trigger)",
            pair_ids.len(),
            pair_ids
        );

        let payload = PortfolioRequestPayload::builder()
            .account_ids(None)
            .market_sync_mode(MarketSyncMode::BackfillHistory {
                asset_ids: None,
                days: 365 * 5,
            })
            .build();
        emit_portfolio_trigger_recalculate(&app_handle, payload);

        // Clear health cache so the next health check picks up the
        // refreshed rates (the recalc itself is async — the user will
        // see progress via the existing PORTFOLIO_TRIGGER_RECALCULATE
        // event listeners + the Recalculate History toast pipeline).
        state.health_service().clear_cache().await;
        return Ok(());
    }

    // Handle sync_prices and retry_sync actions - emit market sync events
    if action.id == "sync_prices" || action.id == "retry_sync" {
        let asset_ids: Vec<String> = serde_json::from_value(action.payload.clone())
            .map_err(|e| format!("Failed to parse asset IDs: {}", e))?;

        info!(
            "Syncing market data for {} assets: {:?}",
            asset_ids.len(),
            asset_ids
        );

        // Reset error counts so the sync won't skip these assets
        let quote_service = state.quote_service();
        quote_service
            .reset_sync_errors(&asset_ids)
            .await
            .map_err(|e| format!("Failed to reset sync errors: {}", e))?;

        if let Err(e) = app_handle.emit(MARKET_SYNC_START, &()) {
            error!("Failed to emit market:sync-start event: {}", e);
        }

        let quote_service = state.quote_service();
        match quote_service
            .sync(SyncMode::Incremental, Some(asset_ids))
            .await
        {
            Ok(result) => {
                if !result.failures.is_empty() {
                    let failed_symbols: Vec<_> =
                        result.failures.iter().map(|(s, _)| s.as_str()).collect();
                    warn!("Some assets failed to sync: {:?}", failed_symbols);
                }
                let result_payload = MarketSyncResult {
                    failed_syncs: result.failures,
                };
                if let Err(e) = app_handle.emit(MARKET_SYNC_COMPLETE, &result_payload) {
                    error!("Failed to emit market:sync-complete event: {}", e);
                }
            }
            Err(e) => {
                if let Err(e_emit) = app_handle.emit(MARKET_SYNC_ERROR, &e.to_string()) {
                    error!("Failed to emit market:sync-error event: {}", e_emit);
                }
                return Err(format!("Failed to sync market data: {}", e));
            }
        }

        // Clear health cache so next check reflects the sync results
        state.health_service().clear_cache().await;

        return Ok(());
    }

    state
        .health_service()
        .execute_fix(&action)
        .await
        .map_err(|e| e.to_string())
}

/// Get health configuration.
#[tauri::command]
pub async fn get_health_config(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HealthConfig, String> {
    debug!("Getting health config...");
    Ok(state.health_service().get_config().await)
}

/// Update health configuration.
#[tauri::command]
pub async fn update_health_config(
    config: HealthConfig,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    debug!("Updating health config...");
    state
        .health_service()
        .update_config(config)
        .await
        .map_err(|e| e.to_string())
}
