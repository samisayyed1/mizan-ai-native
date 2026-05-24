//! TradingView screener provider (fallback, latest-quote only).
//!
//! Replicates the public requests the `tvscreener` Python library makes against
//! TradingView's screener endpoint (`https://scanner.tradingview.com/{market}/scan`)
//! to retrieve a current price for an instrument. It exists so that when the
//! primary provider (Yahoo) has no quote for something, Mizan can still surface
//! a live price — broadening coverage to crypto, forex, and exchanges Yahoo
//! handles weakly.
//!
//! ## Important properties
//!
//! - **Latest quote only.** The screener returns a *current snapshot*, not an
//!   OHLC history, so `supports_historical` is false. Historical backfill stays
//!   with Yahoo; this provider only fills the "what is it worth right now" gap.
//! - **Fallback priority.** A high `priority()` value keeps it below Yahoo, so
//!   it is only consulted when higher-priority providers can't serve the asset.
//! - **Unofficial endpoint.** TradingView's scanner is undocumented and
//!   ToS-restricted, and unauthenticated data may be delayed. This provider is
//!   off until a user explicitly enables it; failures are non-fatal and route
//!   to the next provider via the registry's circuit breaker.
//!
//! The request/response *shape* is fixed and unit-tested here; the live
//! behaviour (exact exchange disambiguation, real-time vs delayed) must be
//! confirmed with a real run before relying on it.

use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::debug;
use rust_decimal::Decimal;
use serde_json::{json, Value};

use crate::errors::MarketDataError;
use crate::models::{
    AssetProfile, Coverage, InstrumentKind, ProviderInstrument, Quote, QuoteContext, SearchResult,
};
use crate::provider::capabilities::{ProviderCapabilities, RateLimit};
use crate::provider::traits::MarketDataProvider;

const PROVIDER_ID: &str = "TRADINGVIEW";
const DEFAULT_BASE_URL: &str = "https://scanner.tradingview.com";
/// Columns requested from the screener, in order. The response `d` array is
/// aligned to this list, so the parser reads by index.
const REQUEST_COLUMNS: [&str; 2] = ["close", "currency"];

/// Fallback provider backed by TradingView's public screener endpoint.
pub struct TradingViewProvider {
    client: reqwest::Client,
    base_url: String,
}

impl TradingViewProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }
}

impl Default for TradingViewProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Instrument kind for a resolved provider instrument.
fn kind_of(instrument: &ProviderInstrument) -> InstrumentKind {
    match instrument {
        ProviderInstrument::EquitySymbol { .. } => InstrumentKind::Equity,
        ProviderInstrument::CryptoSymbol { .. } | ProviderInstrument::CryptoPair { .. } => {
            InstrumentKind::Crypto
        }
        ProviderInstrument::FxSymbol { .. } | ProviderInstrument::FxPair { .. } => {
            InstrumentKind::Fx
        }
        ProviderInstrument::MetalSymbol { .. } => InstrumentKind::Metal,
        ProviderInstrument::BondIsin { .. } => InstrumentKind::Bond,
        ProviderInstrument::FutureSymbol { .. } => InstrumentKind::Futures,
    }
}

/// TradingView screener market segment for an instrument kind. `None` for kinds
/// this provider does not serve.
fn market_for_kind(kind: InstrumentKind) -> Option<&'static str> {
    match kind {
        InstrumentKind::Equity => Some("global"),
        InstrumentKind::Crypto => Some("crypto"),
        // Spot metals (XAUUSD, XAGUSD) trade on TradingView's forex/CFD feed.
        InstrumentKind::Fx | InstrumentKind::Metal => Some("forex"),
        // Government/sovereign bonds (e.g. TVC:US10Y) via the bond feed.
        InstrumentKind::Bond => Some("bond"),
        // Futures (oil CL, gas NG, gold GC, index ES/NQ) via the futures feed.
        InstrumentKind::Futures => Some("futures"),
        // Options aren't in TradingView's screener.
        InstrumentKind::Option => None,
    }
}

/// The screener `name` to match for a resolved provider instrument. The scanner
/// is filtered on this so we don't depend on an (unverifiable) MIC → TradingView
/// exchange-code crosswalk.
fn screener_name(instrument: &ProviderInstrument) -> Option<String> {
    match instrument {
        ProviderInstrument::EquitySymbol { symbol } => Some(symbol.to_uppercase()),
        ProviderInstrument::CryptoSymbol { symbol } => Some(symbol.to_uppercase()),
        ProviderInstrument::CryptoPair { symbol, market } => Some(format!(
            "{}{}",
            symbol.to_uppercase(),
            market.to_uppercase()
        )),
        ProviderInstrument::FxSymbol { symbol } => Some(symbol.to_uppercase()),
        ProviderInstrument::FxPair { from, to } => {
            Some(format!("{}{}", from.to_uppercase(), to.to_uppercase()))
        }
        // Spot metal as code + quote, e.g. XAU + USD -> "XAUUSD".
        ProviderInstrument::MetalSymbol { symbol, quote } => {
            Some(format!("{}{}", symbol.to_uppercase(), quote.to_uppercase()))
        }
        // Sovereign bonds are matched by their TradingView ticker (e.g.
        // "US10Y"); the asset's stored bond symbol/ISIN field is used as the
        // name filter. ISIN-only corporate bonds won't match and fall through.
        ProviderInstrument::BondIsin { isin } => Some(isin.to_uppercase()),
        // Futures root/ticker, e.g. "CL", "NG", "GC", "ES".
        ProviderInstrument::FutureSymbol { symbol } => Some(symbol.to_uppercase()),
    }
}

/// Full scan URL for a market segment.
fn scan_url(base_url: &str, market: &str) -> String {
    format!("{}/{}/scan", base_url.trim_end_matches('/'), market)
}

/// Build the `/scan` POST body that fetches the latest price for one symbol.
fn build_scan_payload(name: &str) -> Value {
    json!({
        "filter": [{ "left": "name", "operation": "equal", "right": name }],
        "options": { "lang": "en" },
        "symbols": { "query": { "types": [] }, "tickers": [] },
        "columns": REQUEST_COLUMNS,
        "sort": { "sortBy": "close", "sortOrder": "desc" },
        "range": [0, 1],
    })
}

/// Parse the first row of a `/scan` response into `(close, currency)`.
///
/// Response shape: `{"data":[{"s":"NASDAQ:AAPL","d":[<close>, <currency>]}]}`,
/// the `d` array aligned to [`REQUEST_COLUMNS`]. `currency_fallback` is used
/// when the row omits a currency (e.g. some forex/crypto rows).
fn parse_scan_response(body: &Value, currency_fallback: &str) -> Option<(Decimal, String)> {
    let row = body.get("data")?.as_array()?.first()?;
    let d = row.get("d")?.as_array()?;

    // close — index 0. Parse from the numeric literal to preserve precision.
    let close_val = d.first()?;
    let close = match close_val {
        Value::Number(n) => Decimal::from_str(&n.to_string()).ok()?,
        Value::String(s) => Decimal::from_str(s.trim()).ok()?,
        _ => return None,
    };

    // currency — index 1, optional.
    let currency = d
        .get(1)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| currency_fallback.to_uppercase());

    Some((close, currency))
}

#[async_trait]
impl MarketDataProvider for TradingViewProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn priority(&self) -> u8 {
        // Well below Yahoo (10) and the keyed providers — strictly a fallback.
        90
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            instrument_kinds: &[
                InstrumentKind::Equity,
                InstrumentKind::Crypto,
                InstrumentKind::Fx,
                InstrumentKind::Metal,
                InstrumentKind::Bond,
                InstrumentKind::Futures,
            ],
            coverage: Coverage::global_best_effort(),
            supports_latest: true,
            // Screener returns a current snapshot, not an OHLC series.
            supports_historical: false,
            supports_search: false,
            supports_profile: false,
        }
    }

    fn rate_limit(&self) -> RateLimit {
        // Conservative for an unofficial endpoint.
        RateLimit {
            requests_per_minute: 30,
            max_concurrency: 2,
            min_delay: Duration::from_millis(200),
        }
    }

    async fn get_latest_quote(
        &self,
        context: &QuoteContext,
        instrument: ProviderInstrument,
    ) -> Result<Quote, MarketDataError> {
        let market = market_for_kind(kind_of(&instrument)).ok_or_else(|| {
            MarketDataError::ResolutionFailed {
                provider: PROVIDER_ID.to_string(),
            }
        })?;
        let name = screener_name(&instrument).ok_or_else(|| MarketDataError::ResolutionFailed {
            provider: PROVIDER_ID.to_string(),
        })?;

        let url = scan_url(&self.base_url, market);
        let payload = build_scan_payload(&name);
        debug!("TradingView: scanning {} for {}", market, name);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .header("Origin", "https://www.tradingview.com")
            .header("Referer", "https://www.tradingview.com/")
            .json(&payload)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    MarketDataError::Timeout {
                        provider: PROVIDER_ID.to_string(),
                    }
                } else {
                    MarketDataError::ProviderError {
                        provider: PROVIDER_ID.to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        if response.status().as_u16() == 429 {
            return Err(MarketDataError::RateLimited {
                provider: PROVIDER_ID.to_string(),
            });
        }
        if !response.status().is_success() {
            return Err(MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: format!("invalid JSON: {e}"),
            })?;

        let currency_fallback = context.currency_hint.as_deref().unwrap_or("USD");
        let (close, currency) = parse_scan_response(&body, currency_fallback)
            .ok_or_else(|| MarketDataError::SymbolNotFound(name.clone()))?;

        Ok(Quote::new(
            Utc::now(),
            close,
            currency,
            PROVIDER_ID.to_string(),
        ))
    }

    async fn get_historical_quotes(
        &self,
        _context: &QuoteContext,
        _instrument: ProviderInstrument,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<Quote>, MarketDataError> {
        Err(MarketDataError::NotSupported {
            operation: "historical".to_string(),
            provider: PROVIDER_ID.to_string(),
        })
    }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>, MarketDataError> {
        Err(MarketDataError::NotSupported {
            operation: "search".to_string(),
            provider: PROVIDER_ID.to_string(),
        })
    }

    async fn get_profile(&self, _symbol: &str) -> Result<AssetProfile, MarketDataError> {
        Err(MarketDataError::NotSupported {
            operation: "profile".to_string(),
            provider: PROVIDER_ID.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Currency, ProviderSymbol};
    use std::sync::Arc;

    fn equity(symbol: &str) -> ProviderInstrument {
        ProviderInstrument::EquitySymbol {
            symbol: ProviderSymbol::from(symbol),
        }
    }

    #[test]
    fn market_routing_covers_supported_kinds_only() {
        assert_eq!(market_for_kind(InstrumentKind::Equity), Some("global"));
        assert_eq!(market_for_kind(InstrumentKind::Crypto), Some("crypto"));
        assert_eq!(market_for_kind(InstrumentKind::Fx), Some("forex"));
        // Spot metals ride the forex/CFD feed.
        assert_eq!(market_for_kind(InstrumentKind::Metal), Some("forex"));
        // Sovereign bonds via the bond feed.
        assert_eq!(market_for_kind(InstrumentKind::Bond), Some("bond"));
        // Futures via the futures feed.
        assert_eq!(market_for_kind(InstrumentKind::Futures), Some("futures"));
        assert_eq!(market_for_kind(InstrumentKind::Option), None);
    }

    #[test]
    fn screener_name_formats_each_instrument() {
        assert_eq!(screener_name(&equity("aapl")).as_deref(), Some("AAPL"));
        assert_eq!(
            screener_name(&ProviderInstrument::FxPair {
                from: Currency::from("eur"),
                to: Currency::from("usd"),
            })
            .as_deref(),
            Some("EURUSD")
        );
        assert_eq!(
            screener_name(&ProviderInstrument::CryptoPair {
                symbol: ProviderSymbol::from("btc"),
                market: Currency::from("usd"),
            })
            .as_deref(),
            Some("BTCUSD")
        );
        // Spot metal: code + quote.
        assert_eq!(
            screener_name(&ProviderInstrument::MetalSymbol {
                symbol: ProviderSymbol::from("xau"),
                quote: Currency::from("usd"),
            })
            .as_deref(),
            Some("XAUUSD")
        );
        // Sovereign bond matched by its TradingView ticker / stored symbol.
        assert_eq!(
            screener_name(&ProviderInstrument::BondIsin {
                isin: ProviderSymbol::from("us10y")
            })
            .as_deref(),
            Some("US10Y")
        );
        // Futures root/ticker.
        assert_eq!(
            screener_name(&ProviderInstrument::FutureSymbol {
                symbol: ProviderSymbol::from("cl")
            })
            .as_deref(),
            Some("CL")
        );
    }

    #[test]
    fn scan_url_joins_cleanly() {
        assert_eq!(
            scan_url("https://scanner.tradingview.com", "global"),
            "https://scanner.tradingview.com/global/scan"
        );
        // Trailing slash on base is tolerated.
        assert_eq!(
            scan_url("https://scanner.tradingview.com/", "crypto"),
            "https://scanner.tradingview.com/crypto/scan"
        );
    }

    #[test]
    fn payload_filters_by_name_and_requests_columns() {
        let payload = build_scan_payload("AAPL");
        assert_eq!(payload["filter"][0]["left"], "name");
        assert_eq!(payload["filter"][0]["operation"], "equal");
        assert_eq!(payload["filter"][0]["right"], "AAPL");
        assert_eq!(payload["columns"][0], "close");
        assert_eq!(payload["columns"][1], "currency");
        assert_eq!(payload["range"][0], 0);
        assert_eq!(payload["range"][1], 1);
    }

    #[test]
    fn parses_close_and_currency_preserving_precision() {
        let body = json!({
            "data": [{ "s": "NASDAQ:AAPL", "d": [231.123456789, "USD"] }],
            "totalCount": 1
        });
        let (close, currency) = parse_scan_response(&body, "EUR").unwrap();
        assert_eq!(close, Decimal::from_str("231.123456789").unwrap());
        assert_eq!(currency, "USD");
    }

    #[test]
    fn uses_currency_fallback_when_row_has_no_currency() {
        let body = json!({ "data": [{ "s": "FX:EURUSD", "d": [1.0850] }] });
        let (close, currency) = parse_scan_response(&body, "usd").unwrap();
        assert_eq!(close, Decimal::from_str("1.0850").unwrap());
        // Fallback is upper-cased.
        assert_eq!(currency, "USD");
    }

    #[test]
    fn empty_data_yields_none() {
        let body = json!({ "data": [], "totalCount": 0 });
        assert!(parse_scan_response(&body, "USD").is_none());
        let missing = json!({ "totalCount": 0 });
        assert!(parse_scan_response(&missing, "USD").is_none());
    }

    #[test]
    fn provider_is_fallback_latest_only() {
        let p = TradingViewProvider::with_base_url("http://localhost:0");
        assert_eq!(p.id(), "TRADINGVIEW");
        assert!(p.priority() > 10, "must rank below Yahoo");
        let caps = p.capabilities();
        assert!(caps.supports_latest);
        assert!(!caps.supports_historical);
        assert!(caps.instrument_kinds.contains(&InstrumentKind::Crypto));
    }

    #[tokio::test]
    async fn historical_is_not_supported() {
        let p = TradingViewProvider::new();
        let ctx = QuoteContext {
            instrument: crate::models::InstrumentId::Equity {
                ticker: Arc::from("AAPL"),
                mic: None,
            },
            overrides: None,
            currency_hint: None,
            preferred_provider: None,
            bond_metadata: None,
            custom_provider_code: None,
        };
        let res = p
            .get_historical_quotes(&ctx, equity("AAPL"), Utc::now(), Utc::now())
            .await;
        assert!(matches!(res, Err(MarketDataError::NotSupported { .. })));
    }
}
