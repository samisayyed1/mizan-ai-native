//! Financial news from TradingView's public news-flow endpoint.
//!
//! Separate from the quote providers: news has a different host
//! (`news-mediator.tradingview.com`), request shape (GET news-flow vs POST
//! scan), and return type, so it lives in its own module rather than on the
//! quote-focused `MarketDataProvider` trait.
//!
//! Like the scanner provider, this is an **unofficial** endpoint: the response
//! shape is parsed defensively (every wire field is optional; a malformed item
//! is skipped, not fatal) and the live behaviour must be confirmed with a real
//! run. Callers treat any failure as "no news" rather than an error, so a news
//! outage never breaks the dashboard.

use std::collections::HashSet;
use std::time::Duration;

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::errors::MarketDataError;

const PROVIDER_ID: &str = "TRADINGVIEW_NEWS";
const NEWS_BASE_URL: &str = "https://news-mediator.tradingview.com/public/news-flow/v2/news";
const TV_WEB_BASE: &str = "https://www.tradingview.com";

/// Yahoo Finance global RSS index (best-effort — frequently rate-limited).
const YAHOO_RSS_URL: &str = "https://finance.yahoo.com/news/rssindex";
/// MarketWatch top-stories RSS (Dow Jones host; the marketwatch.com host 301s).
const MARKETWATCH_RSS_URL: &str = "https://feeds.content.dowjones.io/public/rss/mw_topstories";

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// A financial news headline, surfaced to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsArticle {
    /// Deterministic id = SHA-256 of the normalized title. The same headline
    /// from two sources collapses to one id, which drives cross-source dedup.
    pub id: String,
    pub title: String,
    /// Publish time, unix seconds.
    pub published: i64,
    /// Publisher name, e.g. "Reuters", "MarketWatch", "Yahoo Finance".
    pub source: String,
    /// Absolute article URL.
    pub url: String,
    /// Short summary/description, when the source provides one.
    pub summary: Option<String>,
    /// Related TradingView symbols, e.g. ["NASDAQ:AAPL"]. Empty for RSS sources.
    pub related_symbols: Vec<String>,
    /// Provider urgency flag (higher = more urgent), if present.
    pub urgency: Option<i64>,
}

impl NewsArticle {
    /// Whether this article is relevant to a user holding any of `tickers`.
    ///
    /// Conservative to limit false positives: matches a related-symbol tag
    /// whose ticker (after any `EXCHANGE:` prefix) equals a held ticker, OR a
    /// `$TICKER` / whitespace-bounded mention in the title. Tickers shorter
    /// than 2 chars and a small denylist of common English words are skipped
    /// for headline matching (they still match via explicit symbol tags).
    pub fn matches_user_portfolio(&self, tickers: &[String]) -> bool {
        const HEADLINE_DENYLIST: &[&str] = &["ALL", "CAT", "ON", "IT", "OR", "BE", "SO", "ARE"];
        let title_upper = format!(" {} ", self.title.to_uppercase());
        for raw in tickers {
            let ticker = raw.trim().to_uppercase();
            if ticker.len() < 2 {
                continue;
            }
            // Explicit symbol-tag match (works for tagged sources like TradingView).
            let tag_hit = self.related_symbols.iter().any(|s| {
                let sym = s.rsplit(':').next().unwrap_or(s);
                sym.eq_ignore_ascii_case(&ticker)
            });
            if tag_hit {
                return true;
            }
            // Headline match — skipped for ambiguous common-word tickers.
            if HEADLINE_DENYLIST.contains(&ticker.as_str()) {
                continue;
            }
            if title_upper.contains(&format!("${ticker}"))
                || title_upper.contains(&format!(" {ticker} "))
            {
                return true;
            }
        }
        false
    }
}

/// Deterministic dedup id for an article: SHA-256 of the title lowercased and
/// reduced to alphanumerics, so trivial punctuation/case differences across
/// sources still collapse to one id.
fn deterministic_id(title: &str) -> String {
    let normalized: String = title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Fetches financial news from TradingView's public news flow.
pub struct NewsProvider {
    client: reqwest::Client,
    base_url: String,
}

impl NewsProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: NEWS_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Fetch the latest news, optionally filtered to a single TradingView
    /// symbol (`EXCHANGE:TICKER`). Returns articles newest-first as provided.
    pub async fn fetch_news(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<NewsArticle>, MarketDataError> {
        // `filter` is repeated; reqwest serialises a slice of tuples as repeated keys.
        let mut query: Vec<(&str, String)> = vec![
            ("filter", "lang:en".to_string()),
            ("client", "overview".to_string()),
            ("streaming", "false".to_string()),
            ("user_prostatus", "non_pro".to_string()),
        ];
        if let Some(sym) = symbol {
            query.push(("filter", format!("symbol:{sym}")));
        }

        let response = self
            .client
            .get(&self.base_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .header("Origin", TV_WEB_BASE)
            .header("Referer", "https://www.tradingview.com/")
            .query(&query)
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

        debug!(
            "News: fetched {} bytes for symbol {:?}",
            body.to_string().len(),
            symbol
        );
        Ok(parse_news_response(&body))
    }

    /// Fetch and normalize an RSS/Atom feed (Yahoo, MarketWatch). RSS items
    /// carry no symbol tags, so `related_symbols` is empty.
    async fn fetch_rss(
        &self,
        url: &str,
        source_name: &str,
    ) -> Result<Vec<NewsArticle>, MarketDataError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Accept", "application/rss+xml, application/xml, text/xml")
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    MarketDataError::Timeout {
                        provider: source_name.to_string(),
                    }
                } else {
                    MarketDataError::ProviderError {
                        provider: source_name.to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        if response.status().as_u16() == 429 {
            return Err(MarketDataError::RateLimited {
                provider: source_name.to_string(),
            });
        }
        if !response.status().is_success() {
            return Err(MarketDataError::ProviderError {
                provider: source_name.to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| MarketDataError::ProviderError {
                provider: source_name.to_string(),
                message: format!("read body: {e}"),
            })?;
        let feed =
            feed_rs::parser::parse(&bytes[..]).map_err(|e| MarketDataError::ProviderError {
                provider: source_name.to_string(),
                message: format!("parse RSS: {e}"),
            })?;

        Ok(feed
            .entries
            .into_iter()
            .filter_map(|entry| parse_rss_entry(entry, source_name))
            .collect())
    }

    /// Fetch every source concurrently and merge into one deduplicated,
    /// newest-first list. Per-source failures are logged and skipped, so the
    /// mesh returns whatever succeeded (failover — never an error).
    pub async fn fetch_global_news_mesh(&self) -> Vec<NewsArticle> {
        let (tv, yahoo, mw) = tokio::join!(
            self.fetch_news(None),
            self.fetch_rss(YAHOO_RSS_URL, "Yahoo Finance"),
            self.fetch_rss(MARKETWATCH_RSS_URL, "MarketWatch"),
        );

        let mut merged: Vec<NewsArticle> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (label, result) in [
            ("TradingView", tv),
            ("Yahoo Finance", yahoo),
            ("MarketWatch", mw),
        ] {
            match result {
                Ok(items) => {
                    for item in items {
                        if seen.insert(item.id.clone()) {
                            merged.push(item);
                        }
                    }
                }
                Err(e) => warn!("News mesh: {label} source failed (skipped): {e}"),
            }
        }
        merged.sort_by_key(|a| std::cmp::Reverse(a.published));
        merged
    }
}

impl Default for NewsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewsItemWire {
    title: Option<String>,
    published: Option<i64>,
    story_path: Option<String>,
    short_description: Option<String>,
    related_symbols: Option<Vec<RelatedSymbolWire>>,
    provider: Option<ProviderWire>,
    urgency: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ProviderWire {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RelatedSymbolWire {
    symbol: Option<String>,
}

/// Parse a news-flow response into articles. Tolerant of an `{ "items": [...] }`
/// object or a bare top-level array, missing/null fields, and malformed items
/// (which are skipped). A title and a story path are the minimum required.
pub fn parse_news_response(body: &Value) -> Vec<NewsArticle> {
    let items: &[Value] = match body {
        Value::Array(arr) => arr.as_slice(),
        Value::Object(_) => body
            .get("items")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        _ => &[],
    };
    items.iter().filter_map(parse_item).collect()
}

fn parse_item(value: &Value) -> Option<NewsArticle> {
    let wire: NewsItemWire = serde_json::from_value(value.clone()).ok()?;
    let title = wire.title.filter(|t| !t.trim().is_empty())?;
    let story_path = wire.story_path.filter(|p| !p.trim().is_empty())?;

    let url = if story_path.starts_with("http") {
        story_path.clone()
    } else {
        format!("{TV_WEB_BASE}{story_path}")
    };
    let source = wire.provider.and_then(|p| p.name).unwrap_or_default();
    let related_symbols = wire
        .related_symbols
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.symbol)
        .filter(|s| !s.is_empty())
        .collect();
    let summary = wire
        .short_description
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(NewsArticle {
        id: deterministic_id(&title),
        title,
        published: wire.published.unwrap_or(0),
        source,
        url,
        summary,
        related_symbols,
        urgency: wire.urgency,
    })
}

/// Map one RSS/Atom entry to a `NewsArticle`. Requires a title and a link.
fn parse_rss_entry(entry: feed_rs::model::Entry, source_name: &str) -> Option<NewsArticle> {
    let title = entry
        .title
        .map(|t| t.content.trim().to_string())
        .filter(|t| !t.is_empty())?;
    let url = entry
        .links
        .into_iter()
        .map(|l| l.href)
        .find(|h| !h.is_empty())?;
    let published = entry
        .published
        .or(entry.updated)
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let summary = entry
        .summary
        .map(|s| s.content.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(NewsArticle {
        id: deterministic_id(&title),
        title,
        published,
        source: source_name.to_string(),
        url,
        summary,
        related_symbols: Vec::new(),
        urgency: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_well_formed_feed() {
        let body = json!({
            "items": [
                {
                    "id": "DJN_DN1:0",
                    "title": "Meta releases new app",
                    "published": 1779478380i64,
                    "urgency": 2,
                    "storyPath": "/news/DJN_DN1:0-meta/",
                    "relatedSymbols": [{ "symbol": "NASDAQ:META", "logoid": "meta" }],
                    "provider": { "id": "dow-jones", "name": "Dow Jones Newswires" }
                }
            ]
        });
        let out = parse_news_response(&body);
        assert_eq!(out.len(), 1);
        let a = &out[0];
        assert_eq!(a.id, deterministic_id("Meta releases new app"));
        assert_eq!(a.title, "Meta releases new app");
        assert_eq!(a.published, 1779478380);
        assert_eq!(a.source, "Dow Jones Newswires");
        assert_eq!(a.url, "https://www.tradingview.com/news/DJN_DN1:0-meta/");
        assert_eq!(a.related_symbols, vec!["NASDAQ:META"]);
        assert_eq!(a.urgency, Some(2));
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        // No id, no provider, no relatedSymbols, no urgency — still valid.
        let body = json!({
            "items": [
                { "title": "Bare headline", "published": 100, "storyPath": "/news/x/" }
            ]
        });
        let out = parse_news_response(&body);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, deterministic_id("Bare headline"));
        assert_eq!(out[0].source, "");
        assert!(out[0].related_symbols.is_empty());
        assert_eq!(out[0].urgency, None);
        assert_eq!(out[0].summary, None);
    }

    #[test]
    fn skips_items_missing_required_fields() {
        let body = json!({
            "items": [
                { "title": "No path" },                       // missing storyPath -> skip
                { "storyPath": "/news/y/" },                  // missing title -> skip
                { "title": "  ", "storyPath": "/news/z/" },   // blank title -> skip
                { "title": "Good", "storyPath": "/news/ok/" } // kept
            ]
        });
        let out = parse_news_response(&body);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Good");
    }

    #[test]
    fn handles_empty_and_missing_items() {
        assert!(parse_news_response(&json!({ "items": [] })).is_empty());
        assert!(parse_news_response(&json!({ "totalCount": 0 })).is_empty());
        assert!(parse_news_response(&json!(null)).is_empty());
    }

    #[test]
    fn supports_bare_top_level_array() {
        let body = json!([
            { "title": "Array form", "published": 1, "storyPath": "/news/arr/" }
        ]);
        let out = parse_news_response(&body);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Array form");
    }

    #[test]
    fn keeps_absolute_url_as_is() {
        let body = json!({
            "items": [
                { "title": "Abs", "storyPath": "https://example.com/a" }
            ]
        });
        let out = parse_news_response(&body);
        assert_eq!(out[0].url, "https://example.com/a");
    }

    #[test]
    fn provider_constructs() {
        let p = NewsProvider::with_base_url("http://localhost:0");
        assert_eq!(p.base_url, "http://localhost:0");
    }

    #[test]
    fn deterministic_id_is_stable_and_punctuation_insensitive() {
        // Same headline, different punctuation/case → same id (cross-source dedup).
        assert_eq!(
            deterministic_id("Apple beats earnings!"),
            deterministic_id("apple beats earnings")
        );
        assert_ne!(
            deterministic_id("Apple beats earnings"),
            deterministic_id("Apple misses earnings")
        );
    }

    #[test]
    fn parses_summary_when_present() {
        let body = json!({
            "items": [
                {
                    "title": "Headline",
                    "storyPath": "/news/h/",
                    "shortDescription": "  A short summary.  "
                }
            ]
        });
        let out = parse_news_response(&body);
        assert_eq!(out[0].summary.as_deref(), Some("A short summary."));
    }

    #[test]
    fn parses_rss_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0"><channel>
          <title>MarketWatch</title>
          <item>
            <title>Fed holds rates steady</title>
            <link>https://www.marketwatch.com/story/fed-holds</link>
            <description>The Federal Reserve kept rates unchanged.</description>
            <pubDate>Fri, 22 May 2026 20:01:00 GMT</pubDate>
          </item>
        </channel></rss>"#;
        let feed = feed_rs::parser::parse(xml.as_bytes()).unwrap();
        let entry = feed.entries.into_iter().next().unwrap();
        let article = parse_rss_entry(entry, "MarketWatch").unwrap();
        assert_eq!(article.title, "Fed holds rates steady");
        assert_eq!(article.source, "MarketWatch");
        assert_eq!(article.url, "https://www.marketwatch.com/story/fed-holds");
        assert_eq!(article.id, deterministic_id("Fed holds rates steady"));
        assert!(article.published > 0);
        assert!(article.summary.is_some());
        assert!(article.related_symbols.is_empty());
    }

    fn article(title: &str, related: &[&str]) -> NewsArticle {
        NewsArticle {
            id: deterministic_id(title),
            title: title.to_string(),
            published: 0,
            source: "Test".to_string(),
            url: "https://x".to_string(),
            summary: None,
            related_symbols: related.iter().map(|s| s.to_string()).collect(),
            urgency: None,
        }
    }

    #[test]
    fn portfolio_match_via_symbol_tag() {
        let a = article("Some unrelated headline", &["NASDAQ:AAPL"]);
        assert!(a.matches_user_portfolio(&["AAPL".to_string()]));
        assert!(!a.matches_user_portfolio(&["MSFT".to_string()]));
    }

    #[test]
    fn portfolio_match_via_headline() {
        let dollar = article("$TSLA surges on delivery beat", &[]);
        assert!(dollar.matches_user_portfolio(&["TSLA".to_string()]));
        let bounded = article("Why NVDA is soaring today", &[]);
        assert!(bounded.matches_user_portfolio(&["NVDA".to_string()]));
    }

    #[test]
    fn portfolio_match_rejects_common_word_tickers_in_headline() {
        // "CAT" the English word must not match a CAT holding via headline.
        let a = article("The cat sat on the mat", &[]);
        assert!(!a.matches_user_portfolio(&["CAT".to_string()]));
        // But an explicit tag still matches.
        let tagged = article("The cat sat on the mat", &["NYSE:CAT"]);
        assert!(tagged.matches_user_portfolio(&["CAT".to_string()]));
    }
}
