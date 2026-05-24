use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    context::ServiceContext,
    events::{
        emit_portfolio_trigger_recalculate, emit_portfolio_trigger_update, PortfolioRequestPayload,
    },
};

use log::{debug, error};
use mizan_core::quotes::{
    service::{ProviderInfo, TickerQuote},
    LatestQuoteSnapshot, MarketSyncMode, Quote, QuoteImport, SymbolSearchResult,
};
use mizan_market_data::{ExchangeInfo, NewsArticle};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn search_symbol(
    query: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<SymbolSearchResult>, String> {
    state
        .quote_service()
        .search_symbol(&query)
        .await
        .map_err(|e| format!("Failed to search ticker: {}", e))
}

/// Current health of every market-data provider (circuit state, consecutive
/// failures, rate-limit headroom), highest-priority first. Read-only.
#[tauri::command]
pub async fn get_provider_health(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<mizan_core::quotes::ProviderHealth>, String> {
    Ok(state.quote_service().get_provider_health().await)
}

#[tauri::command]
pub async fn sync_market_data(
    asset_ids: Option<Vec<String>>,
    refetch_all: bool,
    refetch_recent_days: Option<i64>,
    handle: AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::context::ServiceContext>>,
    rate_limiter: tauri::State<'_, std::sync::Arc<crate::rate_limit::RateLimiter>>,
) -> Result<(), String> {
    // Rate-limit guard. Market data sync hits Yahoo / Alpha Vantage /
    // Finnhub / etc. — all external providers with rate limits of
    // their own. Hammering this command burns through the user's
    // per-day quota and the providers respond with 429 storms that
    // make the next legitimate sync look broken. 3 calls / 30s is
    // generous for any real user clicking "refresh prices".
    if let crate::rate_limit::RateLimitDecision::Deny { retry_after } =
        rate_limiter.check("trigger_market_sync")
    {
        return Err(format!(
            "Too many market-data sync requests. Try again in {:.1}s.",
            retry_after.as_secs_f32()
        ));
    }
    // Determine the appropriate market sync mode based on refetch_all flag
    let market_sync_mode = if let Some(days) = refetch_recent_days {
        MarketSyncMode::RefetchRecent { asset_ids, days }
    } else if refetch_all {
        MarketSyncMode::BackfillHistory {
            asset_ids,
            days: 365 * 5, // 5 years of history as fallback
        }
    } else {
        MarketSyncMode::Incremental { asset_ids }
    };

    let payload = PortfolioRequestPayload::builder()
        .account_ids(None)
        .market_sync_mode(market_sync_mode)
        .build();
    emit_portfolio_trigger_update(&handle, payload);

    // Fire-and-forget usage report. The cloud ledger is authoritative for the
    // per-day cap (Free = 5/day); the local rate-limiter above only prevents
    // burst abuse. Failure here doesn't abort the sync.
    state
        .connect_service()
        .report_usage("market_refresh", 1)
        .await;

    Ok(())
}

#[tauri::command]
pub async fn update_quote(
    quote: Quote,
    state: State<'_, Arc<ServiceContext>>,
    handle: AppHandle,
) -> Result<(), String> {
    debug!("Updating quote: {:?}", quote);
    state
        .quote_service()
        .update_quote(quote.clone())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())?;

    // Manual quote update - no market sync needed, but force full recalculation
    // so historical valuations are recomputed with the updated quotes
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let payload = PortfolioRequestPayload::builder()
            .account_ids(None)
            .market_sync_mode(MarketSyncMode::None)
            .build();
        emit_portfolio_trigger_recalculate(&handle, payload);
    });
    Ok(())
}

#[tauri::command]
pub async fn delete_quote(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
    handle: AppHandle,
) -> Result<(), String> {
    debug!("Deleting quote: {}", id);
    state
        .quote_service()
        .delete_quote(&id)
        .await
        .map_err(|e| e.to_string())?;

    // Manual quote deletion - no market sync needed, but force full recalculation
    // so historical valuations are recomputed without the deleted quotes
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let payload = PortfolioRequestPayload::builder()
            .account_ids(None)
            .market_sync_mode(MarketSyncMode::None)
            .build();
        emit_portfolio_trigger_recalculate(&handle, payload);
    });
    Ok(())
}

#[tauri::command]
pub async fn get_quote_history(
    symbol: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<Quote>, String> {
    debug!("Fetching quote history for symbol: {}", symbol);
    state
        .quote_service()
        .get_historical_quotes(&symbol)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_latest_quotes(
    asset_ids: Vec<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HashMap<String, LatestQuoteSnapshot>, String> {
    state
        .quote_service()
        .get_latest_quotes_snapshot(&asset_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_market_data_providers(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ProviderInfo>, String> {
    debug!("Received request to get market data providers");
    state
        .quote_service()
        .get_providers_info()
        .await
        .map_err(|e| {
            error!("Failed to get market data providers: {}", e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn check_quotes_import(
    content: Vec<u8>,
    has_header_row: bool,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<QuoteImport>, String> {
    debug!(
        "Checking quotes import from {} bytes CSV (has_header={})",
        content.len(),
        has_header_row
    );
    state
        .quote_service()
        .check_quotes_import(&content, has_header_row)
        .await
        .map_err(|e| {
            error!("Failed to check quotes import: {}", e);
            format!("Failed to check quotes import: {}", e)
        })
}

#[tauri::command]
pub async fn import_quotes_csv(
    quotes: Vec<QuoteImport>,
    overwrite_existing: bool,
    state: State<'_, Arc<ServiceContext>>,
    handle: AppHandle,
) -> Result<Vec<QuoteImport>, String> {
    debug!(
        "Importing {} quotes from CSV (overwrite_existing={})",
        quotes.len(),
        overwrite_existing
    );
    let result = state
        .quote_service()
        .import_quotes(quotes, overwrite_existing)
        .await
        .map_err(|e| {
            error!("TAURI COMMAND: import_quotes_csv failed: {}", e);
            format!("Failed to import CSV quotes: {}", e)
        })?;

    // Quote import - no market sync needed, just recalculate
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        debug!("Triggering portfolio recalculation after quote import");
        let payload = PortfolioRequestPayload::builder()
            .account_ids(None)
            .market_sync_mode(MarketSyncMode::None)
            .build();
        emit_portfolio_trigger_recalculate(&handle, payload);
    });

    Ok(result)
}

#[tauri::command]
pub async fn resolve_symbol_quote(
    symbol: String,
    exchange_mic: Option<String>,
    instrument_type: Option<String>,
    quote_ccy: Option<String>,
    provider_id: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<mizan_core::quotes::ResolvedQuote, String> {
    let inst_type = instrument_type
        .as_deref()
        .and_then(mizan_core::assets::InstrumentType::from_external_str);
    state
        .quote_service()
        .resolve_symbol_quote(
            &symbol,
            exchange_mic.as_deref(),
            inst_type.as_ref(),
            quote_ccy.as_deref(),
            provider_id.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to resolve symbol quote: {}", e))
}

#[tauri::command]
pub fn get_exchanges() -> Vec<ExchangeInfo> {
    mizan_market_data::get_exchange_list()
}

/// Fetch dividend events for a symbol from Yahoo Finance.
/// Routes through the Rust backend to avoid CORS restrictions in the webview.
#[tauri::command]
pub async fn fetch_yahoo_dividends(
    symbol: String,
) -> Result<Vec<mizan_market_data::YahooDividend>, String> {
    let provider = mizan_market_data::YahooProvider::new()
        .await
        .map_err(|e| e.to_string())?;
    provider
        .fetch_dividends(&symbol)
        .await
        .map_err(|e| e.to_string())
}

/// Fetch financial news via the multi-source mesh (TradingView + MarketWatch +
/// Yahoo), served from the local SQLite cache. With no symbols → the general
/// "Markets" feed; with symbols → personalized "For You" (cached articles
/// matched to the held tickers). Infallible by design — a transient source
/// outage falls back to the cached feed, so the page never goes empty.
#[tauri::command]
pub async fn fetch_financial_news(
    symbols: Option<Vec<String>>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<NewsArticle>, String> {
    Ok(state.news_service().get_news(symbols).await)
}

/// Live quotes for the curated dashboard ticker (indices/commodities).
#[tauri::command]
pub async fn get_ticker_quotes(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<TickerQuote>, String> {
    Ok(state.quote_service().get_ticker_quotes().await)
}
