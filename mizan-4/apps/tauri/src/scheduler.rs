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
    use mizan_core::sync_ledger::{SyncRunEntry, SyncRunMode, SyncRunProvider, SyncRunSummary};

    info!("Running startup market-data quote sync...");

    // §A4 — open a sync ledger entry so the user / support can see the run
    // happened. Marketdata aggregates Yahoo + TradingView + custom price
    // providers; the SyncResult does not surface per-provider counters
    // today (deeper plumbing tracked separately). Tag the run with the
    // MarketData aggregate variant so the audit row is honest.
    let run_id = uuid::Uuid::new_v4().to_string();
    let started_entry = SyncRunEntry::started(
        run_id.clone(),
        SyncRunProvider::MarketData,
        SyncRunMode::Incremental,
    );
    if let Err(e) = context.sync_ledger().append(started_entry.clone()).await {
        debug!("Sync ledger append (start) failed: {}", e);
    }

    let quote_service = std::sync::Arc::clone(&context.quote_service);
    match quote_service.sync(SyncMode::Incremental, None).await {
        Ok(result) => {
            info!(
                "Startup quote sync completed: {} quotes added across {} assets",
                result.quotes_synced, result.synced
            );

            // §A4 — close the SAME entry (preserves started_at) by
            // calling .finish on the held instance instead of building
            // a second `started()` with a fresh timestamp.
            let finished = started_entry.clone().finish(SyncRunSummary {
                fetched: result.synced as u32,
                inserted: result.quotes_synced as u32,
                skipped: result.skipped as u32,
                errors: result.failed as u32,
                ..Default::default()
            });
            if let Err(e) = context.sync_ledger().append(finished).await {
                debug!("Sync ledger append (finish) failed: {}", e);
            }

            // Emit a lightweight event so the frontend's TICKER_QUOTES +
            // HOLDINGS queries refetch immediately and the dashboard shows
            // live prices without waiting for the 6 h periodic.
            if let Err(e) = tauri::Emitter::emit(
                handle,
                "quotes:startup-sync-complete",
                &result.quotes_synced,
            ) {
                debug!("Failed to emit quotes:startup-sync-complete event: {}", e);
            }
        }
        Err(e) => {
            warn!(
                "Startup quote sync failed: {}. Health Center banner will surface this to the user.",
                e
            );
            // §A4 — close the SAME entry with failure outcome. Wrap the
            // raw error string in a §A24 envelope so support can grep
            // by `__mizan_error: true`.
            let error_json = serde_json::json!({
                "__mizan_error": true,
                "code": "MARKETDATA_SYNC_FAILED",
                "message": e.to_string(),
            })
            .to_string();
            let failed = started_entry.fail(error_json);
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

    // Build a per-category breakdown from the asset breakdown surfaced
    // by NetWorthResponse. The breakdown can contain multiple rows per
    // category (one per holding), so we aggregate by category before
    // persisting — without this the dashboard chart double-counts on
    // any account with >1 instrument.
    let mut category_totals: std::collections::BTreeMap<String, rust_decimal::Decimal> =
        std::collections::BTreeMap::new();
    for b in nw.assets.breakdown.iter() {
        *category_totals
            .entry(b.category.clone())
            .or_insert(rust_decimal::Decimal::ZERO) += b.value;
    }
    let mut breakdown: Vec<NetWorthBreakdownEntry> = category_totals
        .into_iter()
        .map(|(key, value)| NetWorthBreakdownEntry { key, value })
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

/// Personalized AI wealth-notification engine — Notify-5.
///
/// Hydrates the deterministic `InsightsInput` from the user's local
/// state (NW history, goal progress, sync ledger), runs the rule set,
/// idempotently persists the emitted notifications via the SQLite
/// `notifications` table, then fires a single native OS notification
/// for the most-important new row of severity ≥ Warning.
///
/// IDEMPOTENCY: each rule emits a `dedupe_key` shaped as
/// `<rule>:<scope>:<date>`. The UNIQUE index on
/// `notifications.dedupe_key` means rerunning this on the same day
/// is a no-op — so we can call it on every startup + on a 4h tick
/// without spamming the user.
///
/// COST: zero LLM calls in this path. The Mizan-AI digest layer
/// (Notify-4) is a separate one-call-per-day call site that reads
/// the emitted insights and synthesises a 2-sentence summary.
pub async fn run_insights_tick(handle: &AppHandle, context: &std::sync::Arc<ServiceContext>) {
    use log::{debug, info, warn};
    use mizan_insights::{evaluate, InsightsInput, NetWorthHistoryPoint};

    let today = chrono::Utc::now().date_naive();
    let base_currency = context.get_base_currency();

    // ── 1) Net-worth history (30d) for NetWorthDip + ATH ──────────────
    // The dip rule needs ≥7 days of history; the ATH baseline is
    // computed from this window's prior max (today excluded so we
    // never compare today vs. itself).
    let from = today - chrono::Duration::days(30);
    let history = match context
        .net_worth_snapshot_service()
        .range(from, today)
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|s| NetWorthHistoryPoint {
                date: s.snapshot_date,
                net_worth_base: s.net_worth,
            })
            .collect::<Vec<_>>(),
        Err(e) => {
            debug!(
                "Insights: NW range failed: {} — running with empty history",
                e
            );
            Vec::new()
        }
    };
    let previous_ath = if history.len() >= 2 {
        history
            .iter()
            .take(history.len() - 1)
            .map(|p| p.net_worth_base)
            .max()
    } else {
        None
    };

    // ── 2) BigMove — per-holding day-over-day moves ───────────────────
    let holding_moves = hydrate_holding_moves(context, &base_currency).await;

    // ── 3) CashDrag — cash % of net worth + consecutive days above ────
    let (cash_pct_of_net_worth, cash_high_for_days) = hydrate_cash_drag(context).await;

    // ── 4) DividendPosted — activities in the last 24h ────────────────
    let dividend_events = hydrate_dividend_events(context);

    // ── 5) SyncFailure — failed runs in the last 24h, one per provider─
    let sync_failures = hydrate_sync_failures(context).await;

    // ── 6) GoalMilestone — current progress per goal ──────────────────
    let goal_progress = hydrate_goal_progress(context);

    let input = InsightsInput {
        today: Some(today),
        base_currency,
        holding_moves,
        goal_progress,
        net_worth_history: history,
        previous_ath,
        cash_pct_of_net_worth,
        cash_high_for_days,
        sync_failures,
        dividend_events,
    };

    let candidates = evaluate(&input);
    if candidates.is_empty() {
        debug!("Insights tick: no notifications to emit");
        return;
    }

    let service = context.notification_service();
    let mut newly_emitted: Vec<mizan_core::notifications::Notification> = Vec::new();
    for n in candidates {
        // emit returns Ok(true) only when this is the first time the
        // dedupe_key has been seen. So `newly_emitted` is the strictly
        // new-this-tick set we can push to the OS.
        match service.emit(n.clone()).await {
            Ok(true) => newly_emitted.push(n),
            Ok(false) => debug!("Insights: dedupe-skipped {}", n.dedupe_key),
            Err(e) => warn!("Insights: persist failed for {}: {}", n.dedupe_key, e),
        }
    }

    if newly_emitted.is_empty() {
        return;
    }

    info!(
        "Insights tick: emitted {} new notifications",
        newly_emitted.len()
    );

    // Bubble a frontend event so the bell badge refreshes instantly
    // instead of waiting for the next polling tick.
    use tauri::Emitter;
    if let Err(e) = handle.emit("notifications:new", newly_emitted.len()) {
        debug!("Insights: failed to emit notifications:new event: {}", e);
    }

    // Fire one native OS notification for the highest-severity new
    // row. Doing one (not N) keeps the user's Notification Center
    // legible — they can open the in-app panel to see the full set.
    use mizan_core::notifications::NotificationSeverity;
    let banner = newly_emitted
        .iter()
        .filter(|n| n.severity.should_push_to_os())
        .max_by_key(|n| match n.severity {
            NotificationSeverity::Critical => 3,
            NotificationSeverity::Warning => 2,
            NotificationSeverity::Success => 1,
            NotificationSeverity::Info => 0,
        });
    if let Some(banner) = banner {
        push_os_notification(handle, &banner.title, &banner.body);
    }

    // Notify-4 — AI-narrated digest. Fail-soft: any path that can't
    // produce a digest (entitlement, missing key, network) is silently
    // skipped — the user still has the deterministic rows above.
    if let Err(e) = run_ai_digest(handle, context, &newly_emitted).await {
        debug!("Insights AI digest skipped: {}", e);
    }
}

/// Notify-4 — wrap the day's deterministic insights in a single
/// 2-sentence AI-generated digest and persist it as one additional
/// `AiDigest` notification row.
///
/// COST GATE
///  - `managed_ai` must be true (Silver/Gold). Free returns immediately.
///  - The persisted row's dedupe_key is `ai_digest:<today>`, so the
///    UNIQUE index naturally prevents more than one digest per day per
///    user — the entire rest of this function is a no-op on the second
///    call within the same UTC day.
///
/// FAIL-SOFT
///  - Returns `Ok(())` on the happy path AND on quiet skips (no
///    entitlement, no Connect session, no insights to summarise).
///  - Returns `Err` only for unexpected runtime failures the caller
///    should log; the user-visible bell still has all the
///    deterministic rows we emitted above, so the digest is purely
///    additive value.
async fn run_ai_digest(
    handle: &AppHandle,
    context: &std::sync::Arc<ServiceContext>,
    newly_emitted: &[mizan_core::notifications::Notification],
) -> Result<(), String> {
    use mizan_ai::insights_digest::{
        InsightForDigest, InsightsDigestService, InsightsDigestServiceTrait,
    };
    use mizan_core::notifications::{Notification, NotificationKind, NotificationSeverity};

    // 1) Filter out anything that's already an AiDigest from a prior tick
    //    so we don't recursively summarise our own output.
    let insights: Vec<&Notification> = newly_emitted
        .iter()
        .filter(|n| n.kind != NotificationKind::AiDigest)
        .collect();
    if insights.is_empty() {
        return Ok(());
    }

    // 2) Entitlement gate — only Silver/Gold get the managed-AI digest.
    //    Free tier (and any unsigned-in user) keeps the deterministic
    //    rows but skips the LLM call.
    let ent = match context.connect_service().get_entitlements().await {
        Ok(e) => e,
        Err(e) => {
            debug!("AI digest: get_entitlements failed ({}). Skipping.", e);
            return Ok(());
        }
    };
    if !ent.managed_ai {
        debug!(
            "AI digest: managed_ai disabled (plan={}). Skipping.",
            ent.plan
        );
        return Ok(());
    }

    // 3) Resolve provider+model from the user's configured AI settings.
    //    We use the *default* provider so the digest stays consistent
    //    with what the user picked for chat.
    let providers = context
        .ai_provider_service
        .get_ai_providers()
        .map_err(|e| format!("get_ai_providers failed: {e}"))?;
    let default_id = providers
        .default_provider
        .clone()
        .ok_or_else(|| "no default AI provider configured".to_string())?;
    let provider = providers
        .providers
        .iter()
        .find(|p| p.id == default_id)
        .ok_or_else(|| format!("default provider {default_id} not in catalog"))?;
    let provider_id = provider.id.clone();
    // `selected_model` is the user override; fall back to `default_model`
    // (the catalog's per-provider ship default — `gpt-4o-mini` for mizan).
    let model_id = provider
        .selected_model
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| provider.default_model.clone());

    // 4) Build the typed slice for the digest service.
    let digest_inputs: Vec<InsightForDigest> = insights
        .iter()
        .map(|n| InsightForDigest {
            title: n.title.clone(),
            body: n.body.clone(),
            severity: n.severity.as_str().to_string(),
        })
        .collect();
    let base_currency = context.get_base_currency();

    // 5) Generate. Failures here are noisy at debug level but never
    //    propagate up — this is best-effort, not a hard dependency.
    let env = std::sync::Arc::clone(&context.ai_environment);
    let digest_svc = InsightsDigestService::new(env);
    let summary = match digest_svc
        .generate(&digest_inputs, &base_currency, &provider_id, &model_id)
        .await
    {
        Ok(Some(text)) => text,
        Ok(None) => {
            debug!("AI digest: provider returned None (no managed key / empty body)");
            return Ok(());
        }
        Err(e) => {
            debug!("AI digest: provider call failed: {}", e);
            return Ok(());
        }
    };

    // 6) Persist as a single AiDigest notification, dedupe_key gates it
    //    to one-per-UTC-day. Severity = Info so the OS push doesn't
    //    fire — we already pushed the highest-severity raw row above;
    //    the digest is for the in-app bell.
    let today = chrono::Utc::now().date_naive();
    let dedupe = format!("ai_digest:{}", today.format("%Y-%m-%d"));
    let digest_row = Notification {
        id: uuid::Uuid::new_v4().to_string(),
        kind: NotificationKind::AiDigest,
        severity: NotificationSeverity::Info,
        title: format!("Mizan AI digest — {}", today.format("%b %-d")),
        body: summary,
        deep_link: Some("mizan://dashboard".to_string()),
        payload_json: serde_json::json!({
            "providerId": provider_id,
            "modelId": model_id,
            "sourceInsightIds": insights.iter().map(|n| &n.id).collect::<Vec<_>>(),
        })
        .to_string(),
        dedupe_key: dedupe,
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        read_at_ms: None,
        dismissed_at_ms: None,
    };

    match context.notification_service().emit(digest_row).await {
        Ok(true) => {
            info!("AI digest emitted for {today}");
            use tauri::Emitter;
            let _ = handle.emit("notifications:new", 1);
        }
        Ok(false) => debug!("AI digest: already exists for {today} (dedupe hit)"),
        Err(e) => return Err(format!("digest persist failed: {e}")),
    }
    Ok(())
}

/// Send a single native OS notification through tauri-plugin-notification.
/// Best-effort: the plugin requires permission, and if the user denied it
/// we silently fall back to the in-app bell. This is the right behaviour
/// — we never want to silently lose the insight if the user revoked OS
/// permission, and the in-app bell is the canonical surface anyway.
fn push_os_notification(handle: &AppHandle, title: &str, body: &str) {
    use log::debug;
    use tauri_plugin_notification::NotificationExt;

    let result = handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
    if let Err(e) = result {
        debug!("OS notification show failed (permission denied?): {}", e);
    }
}

// ─────────────────────────────────────────────────────────────────────
// Notify track — hydration helpers for the 4 deterministic rule
// families that need data outside the NW snapshot service.
// ─────────────────────────────────────────────────────────────────────

/// BigMove — fetch holdings for every active account, populate the
/// rule input with per-holding day-over-day move data.
///
/// Strategy:
///   - Iterate active, non-archived accounts (filters out the synthetic
///     TOTAL row + archived/closed accounts so we don't double-count or
///     surface stale notifications about positions the user has closed).
///   - For each, call `holdings_service.get_holdings(account_id, base)`.
///     That call resolves prices via the valuation service so each
///     returned Holding already has `day_change_pct`, `market_value.base`,
///     and `prev_close_value.base` set when a quote was available.
///   - Skip alt-asset holdings (Property / Vehicle / Collectible) since
///     they don't have daily quotes.
///   - The rule engine filters again on threshold + position size, so
///     we pass everything we have.
async fn hydrate_holding_moves(
    context: &std::sync::Arc<ServiceContext>,
    base_currency: &str,
) -> Vec<mizan_insights::HoldingDayMove> {
    use log::debug;
    use mizan_insights::HoldingDayMove;
    use rust_decimal::Decimal;

    let accounts = match context.account_service().get_active_non_archived_accounts() {
        Ok(a) => a,
        Err(e) => {
            debug!(
                "Insights/BigMove: get_active_accounts failed: {} — skipping",
                e
            );
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for account in &accounts {
        // The TOTAL synthetic account isn't a real account; even though
        // the active-non-archived filter usually excludes it, defensive
        // skip in case of catalog drift.
        if account.id == "TOTAL" {
            continue;
        }
        let holdings = match context
            .holdings_service()
            .get_holdings(&account.id, base_currency)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                debug!(
                    "Insights/BigMove: get_holdings({}) failed: {} — skipping account",
                    account.id, e
                );
                continue;
            }
        };
        for h in holdings {
            // Skip alt assets — their daily change is meaningless
            // because we don't fetch live quotes for them.
            use mizan_core::assets::AssetKind;
            if matches!(
                h.asset_kind,
                Some(AssetKind::Property)
                    | Some(AssetKind::Vehicle)
                    | Some(AssetKind::Collectible)
                    | Some(AssetKind::Other)
            ) {
                continue;
            }
            // Need a known change_pct + a positive prev close to compute
            // the move meaningfully. If either is missing, skip — the
            // engine would reject anyway.
            let Some(change_pct) = h.day_change_pct else {
                continue;
            };
            let prev_close_base = h
                .prev_close_value
                .as_ref()
                .map(|m| m.base)
                .unwrap_or(Decimal::ZERO);
            if prev_close_base <= Decimal::ZERO {
                continue;
            }
            let symbol = h
                .instrument
                .as_ref()
                .map(|i| i.symbol.clone())
                .unwrap_or_else(|| h.id.clone());
            let asset_name = h.instrument.as_ref().and_then(|i| i.name.clone());

            // We don't have asset_id directly on Holding, but `id` IS
            // the position id, which serves as the deep-link target.
            // The frontend's deepLinkToRoute routes `mizan://holding/<id>`
            // to /holdings/<id>.
            out.push(HoldingDayMove {
                symbol,
                asset_name,
                asset_id: Some(h.id.clone()),
                prev_price_base: prev_close_base,
                curr_price_base: h.market_value.base,
                change_pct,
                current_value_base: h.market_value.base,
            });
        }
    }
    out
}

/// CashDrag — full hydration: hits the NW snapshot service for a 60-day
/// window, reads each row's breakdown_json for the "CASH" bucket,
/// derives today's cash% and walks back to find the consecutive run of
/// days where cash% > 10%.
///
/// We use a 60-day window (not the same 30-day window the engine uses
/// for ATH / dip) so we can detect runs longer than 30 days — the
/// rule's threshold is "30+ days above" and we don't want a run-length
/// truncated artificially by the window.
async fn hydrate_cash_drag(
    context: &std::sync::Arc<ServiceContext>,
) -> (Option<rust_decimal::Decimal>, Option<u32>) {
    use log::debug;
    use rust_decimal::Decimal;

    let today = chrono::Utc::now().date_naive();
    let from = today - chrono::Duration::days(60); // 60d window so we can detect runs >30d.

    let snapshots = match context
        .net_worth_snapshot_service()
        .range(from, today)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            debug!("Insights/CashDrag: NW range failed: {} — skipping", e);
            return (None, None);
        }
    };
    if snapshots.is_empty() {
        return (None, None);
    }

    // Compute cash% per point. Snapshots are ASC by date.
    let pcts: Vec<(chrono::NaiveDate, Decimal)> = snapshots
        .iter()
        .filter_map(|s| {
            if s.net_worth <= Decimal::ZERO {
                return None;
            }
            let cash = s
                .breakdown
                .iter()
                .find(|e| e.key == "CASH")
                .map(|e| e.value)
                .unwrap_or(Decimal::ZERO);
            Some((s.snapshot_date, cash / s.net_worth))
        })
        .collect();

    let Some((_, today_pct)) = pcts.last().copied() else {
        return (None, None);
    };

    // Consecutive trailing days above the 10% threshold, anchored at today.
    // Walking from newest backwards, count while > 0.10; stop when not.
    let threshold = Decimal::new(10, 2); // 0.10
    let mut run: u32 = 0;
    for (_, pct) in pcts.iter().rev() {
        if *pct > threshold {
            run += 1;
        } else {
            break;
        }
    }
    (Some(today_pct), Some(run))
}

/// DividendPosted — query activities for DIVIDEND / INTEREST entries
/// with `activity_date` in the last 24 hours.
///
/// Notes on the dedupe semantics:
///   - The rule engine's `dedupe_key = "dividend:<activity_id>"` is
///     stable across runs — calling this multiple times a day re-emits
///     the same set, but only the first persists (UNIQUE on the
///     storage layer).
///   - Conversion to base currency: the activity service returns the
///     amount in activity currency. We translate using the activity's
///     stored `fx_rate` when available, falling back to the amount as
///     stated (no silent FX fabrication). If neither path yields a
///     base-currency amount, we still emit but the body will show the
///     activity's local currency — better than nothing for the user.
fn hydrate_dividend_events(
    context: &std::sync::Arc<ServiceContext>,
) -> Vec<mizan_insights::DividendEvent> {
    use log::debug;
    use mizan_insights::DividendEvent;
    use rust_decimal::Decimal;
    // Activity-type strings are stable in `activities_constants` but the
    // module is private to the core crate. Hardcoding the canonical
    // labels here is safe — they're part of the persisted schema and
    // never change.
    const ACTIVITY_TYPE_DIVIDEND: &str = "DIVIDEND";
    const ACTIVITY_TYPE_INTEREST: &str = "INTEREST";

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);

    let activities = match context.activity_service().get_income_activities() {
        Ok(a) => a,
        Err(e) => {
            debug!(
                "Insights/Dividend: get_income_activities failed: {} — skipping",
                e
            );
            return Vec::new();
        }
    };

    activities
        .into_iter()
        .filter(|a| a.activity_date >= cutoff)
        .filter(|a| {
            a.activity_type == ACTIVITY_TYPE_DIVIDEND || a.activity_type == ACTIVITY_TYPE_INTEREST
        })
        .filter_map(|a| {
            let amount_local = a.amount.unwrap_or(Decimal::ZERO);
            if amount_local <= Decimal::ZERO {
                return None;
            }
            // Convert to base if we have a stored fx_rate; else leave
            // as the local amount (the rule body will still render).
            let amount_base = match a.fx_rate {
                Some(rate) if rate > Decimal::ZERO => amount_local * rate,
                _ => amount_local,
            };
            // The symbol is the source ID if available; otherwise
            // fall back to the asset_id placeholder or "Cash".
            let symbol = a.asset_id.clone().unwrap_or_else(|| "Cash".to_string());
            Some(DividendEvent {
                activity_id: a.id,
                kind: a.activity_type,
                symbol,
                posted_on: a.activity_date.date_naive(),
                amount_base,
            })
        })
        .collect()
}

/// SyncFailure — last-24h failed runs from the §A4 sync ledger,
/// reduced to one row per distinct provider (the most recent failure).
///
/// Why dedupe by provider: if Plaid fails three times in an hour we
/// still want exactly one user-visible notification per day per
/// provider. The engine's dedupe_key already enforces "once per
/// provider per day" at the storage layer; we trim earlier here so
/// the digest doesn't list the same provider three times.
async fn hydrate_sync_failures(
    context: &std::sync::Arc<ServiceContext>,
) -> Vec<mizan_insights::SyncFailureInput> {
    use log::debug;
    use mizan_core::sync_ledger::SyncRunOutcome;
    use mizan_insights::SyncFailureInput;
    use std::collections::HashMap;

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    // 200 is enough to capture every recent failure on even a chatty
    // setup — the ledger is bounded to 10k by the in-memory impl.
    let recent = match context.sync_ledger().recent(200).await {
        Ok(r) => r,
        Err(e) => {
            debug!("Insights/SyncFailure: recent() failed: {} — skipping", e);
            return Vec::new();
        }
    };

    let mut latest_per_provider: HashMap<&'static str, SyncFailureInput> = HashMap::new();
    for entry in &recent {
        if entry.outcome != SyncRunOutcome::Failed {
            continue;
        }
        let when = entry.finished_at.unwrap_or(entry.started_at);
        if when < cutoff {
            continue;
        }
        let provider = entry.provider.as_str();
        // The error column carries a serialised §A24 MizanError JSON.
        // Surface the raw string (already redacted upstream); the rule
        // engine doesn't try to parse it — the body shows it verbatim.
        let reason = entry
            .error
            .clone()
            .unwrap_or_else(|| "sync failed without a structured reason".to_string());
        let when_ms = when.timestamp_millis();

        // Keep only the most-recent failure per provider in this window.
        let candidate = SyncFailureInput {
            provider: provider.to_string(),
            reason,
            last_success_at_ms: Some(when_ms),
        };
        latest_per_provider
            .entry(provider)
            .and_modify(|existing| {
                if when_ms > existing.last_success_at_ms.unwrap_or(0) {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    latest_per_provider.into_values().collect()
}

/// GoalMilestone — read current goals and surface progress fractions.
///
/// PREVIOUS-PROGRESS GAP: the rule engine wants the prior tick's
/// progress so it can detect "newly crossed" milestones. We don't
/// currently snapshot per-goal progress between ticks, so we pass
/// `previous_progress = 0` here. That means the engine fires the
/// HIGHEST milestone the goal has currently crossed on the first
/// run — and subsequent runs on the same day are dedupe'd by the
/// storage layer (`goal_milestone:<id>:<label>:<date>`). The user
/// will see one notification per goal per milestone per day. A
/// future slice can persist last-seen progress in a small table and
/// pass it here for the true delta semantics.
fn hydrate_goal_progress(
    context: &std::sync::Arc<ServiceContext>,
) -> Vec<mizan_insights::GoalProgress> {
    use log::debug;
    use mizan_insights::GoalProgress;
    use rust_decimal::Decimal;

    let goals = match context.goal_service().get_goals() {
        Ok(g) => g,
        Err(e) => {
            debug!("Insights/Goal: get_goals failed: {} — skipping", e);
            return Vec::new();
        }
    };

    goals
        .into_iter()
        .filter_map(|g| {
            // Need both summary progress and current value to emit.
            let current_progress = g.summary_progress?;
            // Skip goals already marked complete in lifecycle so we
            // don't re-celebrate them. `status_lifecycle` is a free-
            // form text column; "COMPLETED" / "ARCHIVED" are the
            // taxonomy entries our UI sets.
            let lifecycle = g.status_lifecycle.to_ascii_uppercase();
            if lifecycle == "COMPLETED" || lifecycle == "ARCHIVED" {
                return None;
            }
            let current_value_base = g.summary_current_value.unwrap_or(0.0);
            let target_value_base = g.summary_target_amount.or(g.target_amount);
            Some(GoalProgress {
                goal_id: g.id,
                title: g.title,
                previous_progress: Decimal::ZERO,
                current_progress: Decimal::from_f64_retain(current_progress)
                    .unwrap_or(Decimal::ZERO),
                current_value_base: Decimal::from_f64_retain(current_value_base)
                    .unwrap_or(Decimal::ZERO),
                target_value_base: target_value_base.and_then(Decimal::from_f64_retain),
            })
        })
        .collect()
}
