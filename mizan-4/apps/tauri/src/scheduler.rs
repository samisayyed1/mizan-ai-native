//! Startup sync for broker data.
//!
//! Syncs broker data once on app startup. After that, user manually triggers sync.

#[cfg(feature = "connect-sync")]
use std::sync::Arc;

#[cfg(feature = "connect-sync")]
use log::{debug, info, warn};
#[cfg(not(feature = "connect-sync"))]
use tauri::AppHandle;
#[cfg(feature = "connect-sync")]
use tauri::AppHandle;

#[cfg(feature = "connect-sync")]
use mizan_core::quotes::MarketSyncMode;

#[cfg(feature = "connect-sync")]
use crate::commands::brokers_sync::perform_broker_sync;
use crate::context::ServiceContext;

/// Runs broker sync once on startup (async, non-blocking).
///
/// This function:
/// - Checks if user's plan includes broker sync
/// - Performs the sync silently (no toast - user didn't request it)
/// - Triggers portfolio update if activities were synced
#[cfg(feature = "connect-sync")]
pub async fn run_startup_sync(handle: &AppHandle, context: &Arc<ServiceContext>) {
    info!("Running startup broker sync...");

    // Check if user's plan includes broker sync
    match context.connect_service().has_broker_sync().await {
        Ok(true) => {
            // User has broker sync, proceed
        }
        Ok(false) => {
            debug!("Startup sync skipped: plan does not include broker sync");
            return;
        }
        Err(e) => {
            // If we can't check (no token, network error, etc.), skip silently
            debug!(
                "Startup sync skipped: could not verify broker sync access ({})",
                e
            );
            return;
        }
    }

    // Perform sync (orchestrator emits broker:sync-start and broker:sync-complete events)
    match perform_broker_sync(context, Some(handle)).await {
        Ok(result) => {
            info!(
                "Startup sync completed: success={}, message={}",
                result.success, result.message
            );

            // Note: broker:sync-complete event is emitted by the orchestrator via TauriProgressReporter

            // Trigger portfolio update if sync was successful
            // Note: Asset enrichment is handled automatically via domain events (AssetsCreated)
            if result.success {
                if let Some(ref activities) = result.activities_synced {
                    if activities.activities_upserted > 0 {
                        info!(
                            "Triggering portfolio update after startup sync ({} activities synced)",
                            activities.activities_upserted
                        );
                        crate::events::emit_portfolio_trigger_recalculate(
                            handle,
                            crate::events::PortfolioRequestPayload::builder()
                                .market_sync_mode(MarketSyncMode::Incremental { asset_ids: None })
                                .build(),
                        );
                    }
                }

                if let Some(ref holdings) = result.holdings_synced {
                    if holdings.positions_upserted > 0 {
                        info!(
                            "Triggering portfolio update after holdings sync ({} positions synced)",
                            holdings.positions_upserted
                        );
                        crate::events::emit_portfolio_trigger_recalculate(
                            handle,
                            crate::events::PortfolioRequestPayload::builder()
                                .market_sync_mode(MarketSyncMode::Incremental { asset_ids: None })
                                .build(),
                        );
                    }
                }
            }
        }
        Err(e) => {
            // Check if this is an auth error (user not logged in)
            if e.contains("No access token") || e.contains("not authenticated") {
                debug!("Startup sync skipped: user not authenticated");
            } else {
                warn!("Startup sync failed: {}", e);
                // Note: broker:sync-error event is emitted by the orchestrator via TauriProgressReporter
            }
        }
    }
}

#[cfg(not(feature = "connect-sync"))]
pub async fn run_startup_sync(_handle: &AppHandle, _context: &std::sync::Arc<ServiceContext>) {}

/// FX rates older than this are considered stale enough to silently
/// auto-refresh on startup. Matches the health check's "warning"
/// threshold so users never see the red dot for stale FX in the
/// normal case (open the app, FX rates auto-refresh in the background,
/// red dot never materialises).
const FX_AUTO_REFRESH_STALE_HOURS: i64 = 24;

/// Auto-refresh FX rates on app startup if any are stale or missing.
///
/// **Why this exists**: Mizan's health check raises a Critical "Exchange
/// rate update needed" issue whenever any FX rate is older than the
/// critical threshold (72h by default). The user's "Fix" button on
/// that issue triggers a portfolio recalculate. We can pre-empt the
/// red dot entirely by doing the same recalc proactively at startup
/// whenever rates are even slightly stale (24h+).
///
/// Cheap check: just iterate latest exchange rates and look at their
/// timestamps. If any are missing entirely (no rate for a held
/// currency) the existing periodic + on-activity sync paths catch it
/// — but for stale rates that ARE present, we trigger an early
/// recalc rather than wait for the 6-hour periodic.
///
/// Runs after a brief delay so it doesn't compete with the broker
/// startup sync on a single network connection. Non-blocking and
/// silent — same UX as the periodic sync that already runs every
/// 6 hours.
pub async fn run_startup_fx_refresh(handle: &AppHandle, context: &std::sync::Arc<ServiceContext>) {
    use chrono::{Duration as ChronoDuration, Utc};
    use log::{debug, info};
    use mizan_core::quotes::MarketSyncMode;

    // Brief delay so we don't pile up on the broker sync that fires
    // simultaneously. The user's first dashboard render can complete
    // first; this kicks in a few seconds later.
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    let rates = match context.fx_service().get_latest_exchange_rates() {
        Ok(rates) => rates,
        Err(e) => {
            debug!(
                "Startup FX refresh: skipped (could not load latest rates: {})",
                e
            );
            return;
        }
    };

    if rates.is_empty() {
        debug!("Startup FX refresh: no FX rates registered yet, nothing to refresh");
        return;
    }

    let stale_threshold = Utc::now() - ChronoDuration::hours(FX_AUTO_REFRESH_STALE_HOURS);
    let stale_pairs: Vec<String> = rates
        .iter()
        .filter(|r| r.timestamp < stale_threshold)
        .map(|r| format!("{}:{}", r.from_currency, r.to_currency))
        .collect();

    if stale_pairs.is_empty() {
        debug!(
            "Startup FX refresh: all {} rates are fresh (< {}h)",
            rates.len(),
            FX_AUTO_REFRESH_STALE_HOURS
        );
        return;
    }

    info!(
        "Startup FX refresh: {} stale pair(s) detected ({:?}) — emitting portfolio recalculate to refresh rates",
        stale_pairs.len(),
        stale_pairs
    );

    crate::events::emit_portfolio_trigger_recalculate(
        handle,
        crate::events::PortfolioRequestPayload::builder()
            .account_ids(None)
            .market_sync_mode(MarketSyncMode::BackfillHistory {
                asset_ids: None,
                days: 365 * 5,
            })
            .build(),
    );
}
