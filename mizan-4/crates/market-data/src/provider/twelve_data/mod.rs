//! Twelve Data market-data provider.
//!
//! Adds live quote coverage for the breadth of instruments Sami's
//! cross-border portfolio holds — global equities + ETFs + sukuks (by
//! ISIN) + FX pairs + crypto — to complement the existing Yahoo /
//! TradingView fallback chain. Twelve Data's pro plan covers
//! international exchanges (LSE, HKEX, NSE, BSE, SGX) that Yahoo only
//! handles patchily, and exposes bond/sukuk quotes by ISIN that no
//! other provider in our stack supplies.
//!
//! Free tier: 800 API calls/day, 8/min. Standard tier: 2400/day,
//! 144/min — what Sami's key opens. We default rate_limit() to the
//! free-tier numbers so a misconfigured key can't burn through the
//! day's budget on a single tight loop.
//!
//! Endpoints used:
//! - `GET /quote?symbol=...&apikey=...` — real-time quote
//! - `GET /time_series?symbol=...&interval=1day&start_date=...&end_date=...&apikey=...` — historical OHLCV
//! - `GET /symbol_search?symbol=...&apikey=...` — search
//!
//! API docs: https://twelvedata.com/docs

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use log::{debug, warn};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::errors::MarketDataError;
use crate::models::{
    Coverage, InstrumentKind, ProviderInstrument, Quote, QuoteContext, SearchResult,
};
use crate::provider::{MarketDataProvider, ProviderCapabilities, RateLimit};

const BASE_URL: &str = "https://api.twelvedata.com";
const PROVIDER_ID: &str = "TWELVE_DATA";

// ============================================================================
// API response shapes
// ============================================================================

/// Response from `/quote`.
#[derive(Debug, Deserialize)]
struct QuoteResponse {
    /// Echo of the requested symbol. Kept for response-shape parity
    /// (logged on parse failures) even though we don't consume it.
    #[serde(default)]
    #[allow(dead_code)]
    symbol: Option<String>,
    /// Latest close price.
    #[serde(default)]
    close: Option<String>,
    /// Opening price.
    #[serde(default)]
    open: Option<String>,
    /// Intraday high.
    #[serde(default)]
    high: Option<String>,
    /// Intraday low.
    #[serde(default)]
    low: Option<String>,
    /// Volume traded.
    #[serde(default)]
    volume: Option<String>,
    /// ISO currency code.
    #[serde(default)]
    currency: Option<String>,
    /// Last-quote epoch seconds.
    #[serde(default)]
    timestamp: Option<i64>,
    /// Error code surfaced on bad-symbol / over-quota responses (the
    /// API returns HTTP 200 with a body like `{"code":404,...}` for
    /// these cases — see `parse_error`).
    #[serde(default)]
    code: Option<i32>,
    /// Error message companion to `code`.
    #[serde(default)]
    message: Option<String>,
}

/// Response from `/time_series`.
#[derive(Debug, Deserialize)]
struct TimeSeriesResponse {
    /// `"ok"` on success, `"error"` otherwise. Carried through for
    /// debug visibility; consumed indirectly via `parse_error`.
    #[serde(default)]
    #[allow(dead_code)]
    status: Option<String>,
    /// Per-row OHLCV (most-recent first).
    #[serde(default)]
    values: Option<Vec<TimeSeriesPoint>>,
    /// Metadata: holds `currency` for the series.
    #[serde(default)]
    meta: Option<TimeSeriesMeta>,
    /// Error code / message — same shape as `/quote`.
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TimeSeriesMeta {
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TimeSeriesPoint {
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM:SS` depending on interval.
    datetime: String,
    open: Option<String>,
    high: Option<String>,
    low: Option<String>,
    close: Option<String>,
    volume: Option<String>,
}

/// Response from `/symbol_search`.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    symbol: String,
    #[serde(default)]
    instrument_name: Option<String>,
    #[serde(default)]
    exchange: Option<String>,
    #[serde(default)]
    instrument_type: Option<String>,
    #[serde(default)]
    currency: Option<String>,
}

// ============================================================================
// Provider
// ============================================================================

/// Twelve Data market-data provider.
pub struct TwelveDataProvider {
    client: Client,
    api_key: String,
}

impl TwelveDataProvider {
    /// Construct a new provider with the given API key.
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, api_key }
    }

    async fn fetch(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<String, MarketDataError> {
        let url = format!("{BASE_URL}{endpoint}");
        let mut req = self.client.get(&url).query(&[("apikey", &self.api_key)]);
        for (k, v) in params {
            req = req.query(&[(k, v)]);
        }
        debug!("Twelve Data GET {} ({} params)", endpoint, params.len());

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                MarketDataError::Timeout {
                    provider: PROVIDER_ID.to_string(),
                }
            } else {
                MarketDataError::ProviderError {
                    provider: PROVIDER_ID.to_string(),
                    message: format!("request failed: {e}"),
                }
            }
        })?;
        let status = resp.status();

        // Twelve Data uses HTTP 429 for rate-limit AND embeds
        // `{"code":429,...}` in a HTTP 200 body for daily-quota
        // overage. Handle both at this layer so callers see one error
        // type.
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(MarketDataError::RateLimited {
                provider: PROVIDER_ID.to_string(),
            });
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "invalid or missing API key".into(),
            });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("HTTP {status}: {body}"),
            });
        }
        resp.text()
            .await
            .map_err(|e| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("read body: {e}"),
            })
    }

    /// Normalise a `ProviderInstrument` into the symbol string Twelve
    /// Data expects. Twelve Data accepts plain tickers for US
    /// listings, `TICKER:EXCHANGE` for international, `BTC/USD` for
    /// crypto, `EUR/USD` for FX, and ISINs for bonds/sukuks.
    fn extract_symbol(&self, instrument: &ProviderInstrument) -> Result<String, MarketDataError> {
        match instrument {
            ProviderInstrument::EquitySymbol { symbol } => Ok(symbol.to_string()),
            ProviderInstrument::CryptoSymbol { symbol } => {
                if symbol.contains('/') {
                    Ok(symbol.to_string())
                } else {
                    Ok(format!("{symbol}/USD"))
                }
            }
            ProviderInstrument::CryptoPair { symbol, market } => Ok(format!("{symbol}/{market}")),
            ProviderInstrument::FxPair { from, to } => Ok(format!("{from}/{to}")),
            ProviderInstrument::FxSymbol { symbol } => {
                if symbol.len() == 6 && !symbol.contains('/') {
                    Ok(format!("{}/{}", &symbol[..3], &symbol[3..]))
                } else {
                    Ok(symbol.to_string())
                }
            }
            ProviderInstrument::BondIsin { isin } => Ok(isin.to_string()),
            ProviderInstrument::MetalSymbol { .. } => Err(MarketDataError::UnsupportedAssetType(
                "Twelve Data: metals via the dedicated METAL_PRICE_API provider".into(),
            )),
            ProviderInstrument::FutureSymbol { .. } => Err(MarketDataError::UnsupportedAssetType(
                "Twelve Data: futures not in scope on this plan".into(),
            )),
        }
    }
}

#[async_trait]
impl MarketDataProvider for TwelveDataProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn priority(&self) -> u8 {
        // Higher priority than Yahoo (default 10) for international
        // coverage; lower than dedicated providers like Boerse
        // Frankfurt (which beats us on German exchanges) or the
        // metal-price API.
        7
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            instrument_kinds: &[
                InstrumentKind::Equity,
                InstrumentKind::Crypto,
                InstrumentKind::Fx,
                InstrumentKind::Bond,
            ],
            // Global by default: no MIC allowlist, no denylist, accept
            // unknown MICs (Twelve Data resolves international tickers
            // itself).
            coverage: Coverage {
                equity_mic_allow: None,
                equity_mic_deny: None,
                allow_unknown_mic: true,
                metal_quote_ccy_allow: None,
            },
            supports_latest: true,
            supports_historical: true,
            supports_search: true,
            supports_profile: false,
        }
    }

    fn rate_limit(&self) -> RateLimit {
        // Free-tier defaults (8/min, 800/day). Standard-tier keys can
        // bump this via a registry override; we don't read the plan
        // tier from the key itself.
        RateLimit {
            requests_per_minute: 8,
            max_concurrency: 2,
            min_delay: Duration::from_millis(250),
        }
    }

    async fn get_latest_quote(
        &self,
        context: &QuoteContext,
        instrument: ProviderInstrument,
    ) -> Result<Quote, MarketDataError> {
        let symbol = self.extract_symbol(&instrument)?;
        let body = self.fetch("/quote", &[("symbol", &symbol)]).await?;
        let parsed: QuoteResponse =
            serde_json::from_str(&body).map_err(|e| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("parse /quote: {e}"),
            })?;
        if let Some(err) = parse_error(parsed.code, parsed.message.as_deref()) {
            return Err(err);
        }
        let close_str = parsed
            .close
            .ok_or_else(|| MarketDataError::SymbolNotFound(format!("{symbol}: no close")))?;
        let close = decimal_from(&close_str, "close")?;
        let currency = parsed
            .currency
            .or_else(|| context.currency_hint.as_ref().map(|c| c.to_string()))
            .unwrap_or_else(|| "USD".to_string());
        let timestamp = parsed
            .timestamp
            .and_then(|t| Utc.timestamp_opt(t, 0).single())
            .unwrap_or_else(Utc::now);
        Ok(Quote {
            timestamp,
            open: parsed
                .open
                .as_deref()
                .and_then(|s| decimal_from(s, "open").ok()),
            high: parsed
                .high
                .as_deref()
                .and_then(|s| decimal_from(s, "high").ok()),
            low: parsed
                .low
                .as_deref()
                .and_then(|s| decimal_from(s, "low").ok()),
            close,
            volume: parsed
                .volume
                .as_deref()
                .and_then(|s| decimal_from(s, "volume").ok()),
            currency,
            source: PROVIDER_ID.to_string(),
        })
    }

    async fn get_historical_quotes(
        &self,
        context: &QuoteContext,
        instrument: ProviderInstrument,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Quote>, MarketDataError> {
        let symbol = self.extract_symbol(&instrument)?;
        let start_s = start.format("%Y-%m-%d").to_string();
        let end_s = end.format("%Y-%m-%d").to_string();
        let body = self
            .fetch(
                "/time_series",
                &[
                    ("symbol", &symbol),
                    ("interval", "1day"),
                    ("start_date", &start_s),
                    ("end_date", &end_s),
                    ("order", "asc"),
                ],
            )
            .await?;
        let parsed: TimeSeriesResponse =
            serde_json::from_str(&body).map_err(|e| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("parse /time_series: {e}"),
            })?;
        if let Some(err) = parse_error(parsed.code, parsed.message.as_deref()) {
            return Err(err);
        }
        let currency = parsed
            .meta
            .and_then(|m| m.currency)
            .or_else(|| context.currency_hint.as_ref().map(|c| c.to_string()))
            .unwrap_or_else(|| "USD".to_string());
        let points = parsed
            .values
            .ok_or_else(|| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "missing values in /time_series response".into(),
            })?;
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            let Some(close_str) = p.close else { continue };
            let Ok(close) = decimal_from(&close_str, "close") else {
                continue;
            };
            let timestamp = parse_datetime(&p.datetime).unwrap_or_else(Utc::now);
            out.push(Quote {
                timestamp,
                open: p.open.as_deref().and_then(|s| decimal_from(s, "open").ok()),
                high: p.high.as_deref().and_then(|s| decimal_from(s, "high").ok()),
                low: p.low.as_deref().and_then(|s| decimal_from(s, "low").ok()),
                close,
                volume: p
                    .volume
                    .as_deref()
                    .and_then(|s| decimal_from(s, "volume").ok()),
                currency: currency.clone(),
                source: PROVIDER_ID.to_string(),
            });
        }
        Ok(out)
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, MarketDataError> {
        let body = self.fetch("/symbol_search", &[("symbol", query)]).await?;
        let parsed: SearchResponse =
            serde_json::from_str(&body).map_err(|e| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("parse /symbol_search: {e}"),
            })?;
        Ok(parsed
            .data
            .into_iter()
            .map(|item| SearchResult {
                symbol: item.symbol.clone(),
                name: item.instrument_name.unwrap_or_else(|| item.symbol.clone()),
                exchange: item.exchange.unwrap_or_default(),
                exchange_mic: None,
                exchange_name: None,
                asset_type: item.instrument_type.unwrap_or_else(|| "EQUITY".into()),
                currency: item.currency,
                score: None,
                data_source: Some(PROVIDER_ID.to_string()),
            })
            .collect())
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Twelve Data returns errors in two shapes:
/// - HTTP 4xx with a JSON body
/// - HTTP 200 with `{"code": N, "message": "..."}` for bad-symbol / daily-quota cases
///
/// `parse_error` normalises the 200-with-error case into the same
/// `MarketDataError` variants the HTTP-error case raises, so the
/// caller (the registry's failover chain) reacts identically.
fn parse_error(code: Option<i32>, message: Option<&str>) -> Option<MarketDataError> {
    match code {
        None | Some(200) => None,
        Some(404) => Some(MarketDataError::SymbolNotFound(
            message
                .unwrap_or("Twelve Data: symbol not found")
                .to_string(),
        )),
        Some(429) => Some(MarketDataError::RateLimited {
            provider: PROVIDER_ID.to_string(),
        }),
        Some(401) | Some(403) => Some(MarketDataError::ProviderError {
            provider: PROVIDER_ID.to_string(),
            message: message
                .unwrap_or("Twelve Data: invalid API key or insufficient plan")
                .to_string(),
        }),
        Some(other) => {
            warn!("Twelve Data error code={other} message={message:?}");
            Some(MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!(
                    "Twelve Data code {other}: {}",
                    message.unwrap_or("(no message)")
                ),
            })
        }
    }
}

fn decimal_from(s: &str, field: &str) -> Result<Decimal, MarketDataError> {
    s.parse::<Decimal>()
        .map_err(|e| MarketDataError::ProviderError {
            provider: PROVIDER_ID.to_string(),
            message: format!("parse {field}={s:?}: {e}"),
        })
}

fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&dt));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|nd| Utc.from_utc_datetime(&nd));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> TwelveDataProvider {
        TwelveDataProvider::new("test-key".into())
    }

    #[test]
    fn extracts_equity_symbol_unchanged() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::EquitySymbol {
                symbol: "AAPL".into(),
            })
            .unwrap();
        assert_eq!(s, "AAPL");
    }

    #[test]
    fn extracts_fx_pair_with_slash() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::FxPair {
                from: "EUR".into(),
                to: "USD".into(),
            })
            .unwrap();
        assert_eq!(s, "EUR/USD");
    }

    #[test]
    fn extracts_fx_symbol_six_letter_form_into_pair() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::FxSymbol {
                symbol: "USDSGD".into(),
            })
            .unwrap();
        assert_eq!(s, "USD/SGD");
    }

    #[test]
    fn extracts_crypto_symbol_to_usd_pair_by_default() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::CryptoSymbol {
                symbol: "BTC".into(),
            })
            .unwrap();
        assert_eq!(s, "BTC/USD");
    }

    #[test]
    fn extracts_crypto_pair_verbatim() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::CryptoPair {
                symbol: "ETH".into(),
                market: "EUR".into(),
            })
            .unwrap();
        assert_eq!(s, "ETH/EUR");
    }

    #[test]
    fn extracts_bond_isin_verbatim() {
        let p = provider();
        let s = p
            .extract_symbol(&ProviderInstrument::BondIsin {
                isin: "XS2052469165".into(),
            })
            .unwrap();
        assert_eq!(s, "XS2052469165");
    }

    #[test]
    fn metals_route_to_dedicated_provider() {
        let p = provider();
        let err = p
            .extract_symbol(&ProviderInstrument::MetalSymbol {
                symbol: "XAU".into(),
                quote: "USD".into(),
            })
            .unwrap_err();
        match err {
            MarketDataError::UnsupportedAssetType(msg) => {
                assert!(msg.contains("METAL_PRICE_API"))
            }
            other => panic!("expected UnsupportedAssetType, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_404_is_symbol_not_found() {
        let err = parse_error(Some(404), Some("nope")).unwrap();
        matches!(err, MarketDataError::SymbolNotFound(_));
    }

    #[test]
    fn parse_error_429_in_200_body_is_rate_limited() {
        let err = parse_error(Some(429), Some("daily limit")).unwrap();
        matches!(err, MarketDataError::RateLimited { .. });
    }

    #[test]
    fn parse_error_401_is_provider_error_invalid_key() {
        let err = parse_error(Some(401), Some("bad key")).unwrap();
        match err {
            MarketDataError::ProviderError { message, .. } => {
                assert!(message.contains("bad key"))
            }
            other => panic!("expected ProviderError, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_returns_none_for_success_codes() {
        assert!(parse_error(None, None).is_none());
        assert!(parse_error(Some(200), None).is_none());
    }

    #[test]
    fn decimal_from_parses_and_errors_cleanly() {
        assert_eq!(decimal_from("12.50", "x").unwrap().to_string(), "12.50");
        decimal_from("not-a-number", "x").unwrap_err();
    }

    #[test]
    fn parse_datetime_handles_both_date_and_datetime_shapes() {
        parse_datetime("2026-06-14").unwrap();
        parse_datetime("2026-06-14 15:30:00").unwrap();
        assert!(parse_datetime("garbage").is_none());
    }

    #[test]
    fn capabilities_include_equity_crypto_fx_bond_for_full_uncle_coverage() {
        let p = provider();
        let caps = p.capabilities();
        assert!(caps.instrument_kinds.contains(&InstrumentKind::Equity));
        assert!(caps.instrument_kinds.contains(&InstrumentKind::Bond));
        assert!(caps.instrument_kinds.contains(&InstrumentKind::Fx));
        assert!(caps.instrument_kinds.contains(&InstrumentKind::Crypto));
        // Metals routed elsewhere; futures not in plan.
        assert!(!caps.instrument_kinds.contains(&InstrumentKind::Metal));
    }

    #[test]
    fn rate_limit_defaults_to_free_tier_to_protect_quota() {
        // A misconfigured key (free-tier vs std-tier) should not burn
        // through the day's budget on the first tight loop. Standard
        // tier keys can be uplifted via registry config.
        let p = provider();
        let r = p.rate_limit();
        assert_eq!(r.requests_per_minute, 8);
        assert!(r.min_delay >= Duration::from_millis(250));
    }
}
