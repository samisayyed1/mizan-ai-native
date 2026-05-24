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

/// Eagerly sync market data on app startup (no delay).
///
/// **Why this exists**: the periodic market-data sync waits 120 s before its
/// first attempt and then runs every 6 h. For a freshly launched app (or any
/// scenario where the local quote cache is empty/stale), the user stares at a
/// blank ticker conveyor and cost-basis-only holdings for two full minutes
/// before any live quote arrives. That looked broken (because it is broken,
/// UX-wise).
///
/// This function fires once at boot in a detached task, runs an
/// `Incremental` sync covering every asset in the DB, and emits a
/// `quotes:startup-sync-complete` event so the frontend can invalidate
/// `TICKER_QUOTES` + holdings queries and re-render with live prices.
///
/// Failure is intentionally silent in logs (warn-level) — the existing
/// Health Center / dashboard banner surfaces it to the user. Cost-basis
/// fallback remains in place so dashboards never render $0.
pub async fn run_startup_quote_sync(handle: &AppHandle, context: &std::sync::Arc<ServiceContext>) {
    use log::{debug, info, warn};
    use mizan_core::quotes::SyncMode;
    use mizan_core::sync_ledger::{
        SyncRunEntry, SyncRunMode, SyncRunProvider, SyncRunSummary,
    };

    info!("Running startup market-data quote sync...");

    // §A4 — open a sync ledger entry so the user / support can see the run
    // happened. Marketdata sync covers Yahoo + TradingView; recording the
    // top-level run is enough for triage. Per-provider granularity can
    // come later when the quote_service surfaces per-provider counters.
    let run_id = uuid::Uuid::new_v4().to_string();
    let started = SyncRunEntry::started(
        run_id.clone(),
        SyncRunProvider::Yahoo, // Top-level marketdata sync; per-provider breakdown later
        SyncRunMode::Incremental,
    );
    if let Err(e) = context.sync_ledger().append(started).await {
        debug!("Sync ledger append (start) failed: {}", e);
    }

    let quote_service = std::sync::Arc::clone(&context.quote_service);
    match quote_service.sync(SyncMode::Incremental, None).await {
        Ok(result) => {
            info!(
                "Startup quote sync completed: {} quotes added across {} assets",
                result.quotes_synced, result.synced
            );

            // §A4 — close the ledger entry with success counters.
            let finished = SyncRunEntry::started(
                run_id.clone(),
                SyncRunProvider::Yahoo,
                SyncRunMode::Incremental,
            )
            .finish(SyncRunSummary {
                fetched: result.synced as u32,
                inserted: result.quotes_synced as u32,
                ..Default::default()
            });
            if let Err(e) = context.sync_ledger().append(finished).await {
                debug!("Sync ledger append (finish) failed: {}", e);
            }

            // Emit a lightweight event so the frontend's TICKER_QUOTES +
            // HOLDINGS queries refetch immediately and the dashboard shows
            // live prices without waiting for the 6 h periodic.
            if let Err(e) =
                tauri::Emitter::emit(handle, "quotes:startup-sync-complete", &result.quotes_synced)
            {
                debug!("Failed to emit quotes:startup-sync-complete event: {}", e);
            }
        }
        Err(e) => {
            warn!(
                "Startup quote sync failed: {}. Health Center banner will surface this to the user.",
                e
            );
            // §A4 — close ledger with failure + raw error.
            let failed = SyncRunEntry::started(
                run_id,
                SyncRunProvider::Yahoo,
                SyncRunMode::Incremental,
            )
            .fail(format!("{{\"raw\":\"{}\"}}", e.to_string().replace('"', "'")));
            if let Err(emit_err) = context.sync_ledger().append(failed).await {
                debug!("Sync ledger append (fail) failed: {}", emit_err);
            }
        }
    }
}

/// §A12 — capture a Net Worth Snapshot when the app boots (and when the
/// user navigates back to the dashboard via the existing portfolio recalc
/// event; that path is wired in the recalc handler, not here).
///
/// Read total assets / liabilities from the existing NetWorthService for
/// today's date and persist a NetWorthSnapshot with breakdown by tier
/// (SECURITIES + CASH + LIABILITY + PROPERTY + …). The dashboard
/// history line + §A22 daily-brief delta both consume the resulting
/// snapshot range.
///
/// Idempotent — re-running on the same day replaces the existing row
/// (see InMemoryNetWorthSnapshotService::upsert).
pub async fn run_startup_net_worth_snapshot(context: &std::sync::Arc<ServiceContext>) {
    use log::{debug, info, warn};
    use mizan_core::net_worth_snapshot::{
        NetWorthBreakdownEntry, NetWorthSnapshotInput, SnapshotSource,
    };

    info!("Running startup net-worth snapshot...");

    let today = chrono::Utc::now().date_naive();
    let base_currency = context.get_base_currency();

    let nw = match context.net_worth_service().get_net_worth(today).await {
        Ok(nw) => nw,
        Err(e) => {
            warn!("NW snapshot: get_net_worth failed: {}. Skipping.", e);
            return;
        }
    };

    let total_assets = nw.assets.total;
    let total_liabilities = nw.liabilities.total;

    // Build a per-category breakdown from the asset breakdown surfaced by
    // NetWorthResponse. Liability sums collapse into one entry so the
    // chart can stack assets + 1 liability.
    let mut breakdown: Vec<NetWorthBreakdownEntry> = nw
        .assets
        .breakdown
        .iter()
        .map(|b| NetWorthBreakdownEntry {
            key: b.category.clone(),
            value: b.value,
        })
        .collect();
    if !total_liabilities.is_zero() {
        breakdown.push(NetWorthBreakdownEntry {
            key: "LIABILITY".to_string(),
            value: total_liabilities,
        });
    }

    let input = NetWorthSnapshotInput {
        snapshot_date: today,
        base_currency,
        total_assets,
        total_liabilities,
        breakdown,
        source: SnapshotSource::AppOpen,
    };

    match context.net_worth_snapshot_service().upsert(input).await {
        Ok(snapshot) => {
            info!(
                "NW snapshot persisted: {} = {} assets - {} liabilities (net {} {})",
                snapshot.snapshot_date,
                snapshot.total_assets,
                snapshot.total_liabilities,
                snapshot.net_worth,
                snapshot.base_currency,
            );
        }
        Err(e) => {
            debug!("NW snapshot persist failed: {}", e);
        }
    }
}

/// §A22 — generate the daily Investor Brief if today's hasn't been
/// generated yet. Reads NW deltas from the §A12 snapshot history,
/// top movers from current holdings, allocation drift from targets
/// (if set), stale signals from FX/quote services, pending drafts
/// from the chat repository.
///
/// Persists via DailyBriefService for the Settings → Notifications
/// panel. Email transport (SendGrid) is deferred and lives in a
/// separate runner once SENDGRID_API_KEY is provisioned.
pub async fn run_startup_daily_brief(context: &std::sync::Arc<ServiceContext>) {
    use log::{debug, info};
    use mizan_core::daily_brief::{DailyBrief, NetWorthDelta};

    let today = chrono::Utc::now().date_naive();

    // Skip if today's brief already exists.
    match context.daily_brief_service().get(today).await {
        Ok(Some(_)) => {
            debug!("Daily brief for {} already exists — skipping", today);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            debug!("Daily brief get() failed: {} — proceeding to recompute", e);
        }
    }

    // Pull yesterday + today net-worth points from §A12.
    let yesterday = today.pred_opt().unwrap_or(today);
    let snapshots = context.net_worth_snapshot_service();

    let today_nw = snapshots.get(today).await.ok().flatten();
    let yesterday_nw = snapshots.get(yesterday).await.ok().flatten();

    let nw_delta = match (today_nw.as_ref(), yesterday_nw.as_ref()) {
        (Some(t), Some(y)) => NetWorthDelta::new(y.net_worth, t.net_worth),
        (Some(t), None) => NetWorthDelta::new(rust_decimal::Decimal::ZERO, t.net_worth),
        (None, _) => {
            debug!("Daily brief: no NW snapshot for today yet — skipping");
            return;
        }
    };

    let base_currency = today_nw
        .as_ref()
        .map(|s| s.base_currency.clone())
        .unwrap_or_else(|| context.get_base_currency());

    // First cut emits just the NW delta. Top movers / drift / stale /
    // pending-drafts ingestion lands in follow-on slices as their source
    // surfaces are ready (movers needs ledger replay, drift needs
    // allocation targets in settings, etc.).
    let brief = DailyBrief::new(today, base_currency, nw_delta);

    match context.daily_brief_service().upsert(brief).await {
        Ok(_) => info!("Daily brief for {} persisted", today),
        Err(e) => debug!("Daily brief upsert failed: {}", e),
    }
}
