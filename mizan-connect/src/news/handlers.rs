//! News feed HTTP handler — Track D PR-D2.b / Goal v3 §V Phase 6.
//!
//! `POST /v1/news/feed` composes the NewsAPI provider + personalization
//! layer into a single endpoint the desktop calls every ~5 min per the
//! existing `useFinancialNews` hook. The desktop sends its
//! `RankingInput` (tickers + categories + memory keywords) plus the
//! NewsAPI search `q` parameter; the handler fetches + ranks + returns.
//!
//! # Why POST not GET
//!
//! The personalization context can carry up to ~12 ticker symbols +
//! 6 categories + 10 memory keywords. Encoded in a query string this
//! would push the URL past common gateway limits (and the symbols may
//! contain `/`, `=` etc.). POST keeps the request body untyped.
//!
//! # API key plumbing
//!
//! Reads `MIZAN_NEWSAPI_KEY` from the process environment at request
//! time. When absent (development without a paid key, or staging
//! before secrets are vaulted) the handler returns 503
//! `service_unavailable` so the desktop falls back to its existing
//! TradingView-RSS path. The "vault the NewsAPI key" step is one of
//! the remaining production tasks per Goal v3's readiness checklist.
//!
//! # Out of scope (deferred)
//!
//! - Multi-provider fan-out (PR-D2.c..g add Benzinga / Polygon /
//!   Refinitiv / Bondevalue / regional)
//! - Per-user caching + the `news_items_per_user` materialized view
//!   (PR-D4)
//! - Provider trust badge surfacing (`'mcp'`-style for community feeds)

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::AuthenticatedUser;
use crate::error::{AppError, ErrorCode};
use crate::state::AppState;

use super::personalization::{rank_articles, RankedArticle, RankingInput};
use super::providers::newsapi::{fetch as fetch_newsapi, FetchQuery, NewsApiError};
use super::types::NewsTab;

/// Query-string envelope for `POST /v1/news/feed`. The `tab` selects
/// the ranking mode; `query` is the upstream NewsAPI `q` parameter.
#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    /// `"relevant"` or `"global"`. Unknown values default to Global
    /// per [`NewsTab::parse`].
    #[serde(default = "default_tab")]
    pub tab: String,
    /// Free-text query passed through to NewsAPI's `q` parameter.
    /// Default narrows to broad finance coverage so a probe with no
    /// `q` still returns useful results.
    #[serde(default = "default_query")]
    pub query: String,
    /// Optional language code (NewsAPI ISO-639-1). Defaults to `"en"`.
    #[serde(default)]
    pub language: Option<String>,
    /// Page size [1, 100]. Defaults to 30.
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_tab() -> String {
    "global".to_string()
}

fn default_query() -> String {
    "finance OR markets OR stocks OR economy".to_string()
}

fn default_page_size() -> u32 {
    30
}

/// Body for `POST /v1/news/feed`. Carries the user's personalization
/// context. Pure data — no auth tokens, no secrets.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedBody {
    #[serde(default)]
    pub holding_symbols: Vec<String>,
    #[serde(default)]
    pub holding_categories: Vec<super::types::NewsCategory>,
    #[serde(default)]
    pub memory_keywords: Vec<String>,
}

impl From<FeedBody> for RankingInput {
    fn from(b: FeedBody) -> Self {
        Self {
            holding_symbols: b.holding_symbols,
            holding_categories: b.holding_categories,
            memory_keywords: b.memory_keywords,
        }
    }
}

/// Response envelope. `articles` is desc-sorted by score (and
/// published-at as the tiebreaker, per `rank_articles`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedResponse {
    pub tab: NewsTab,
    pub provider: &'static str,
    pub articles: Vec<RankedArticle>,
}

/// `POST /v1/news/feed?tab=...&query=...` — fetch + rank.
///
/// Auth required. Reads `MIZAN_NEWSAPI_KEY` per-request (PR-D2.c
/// promotes to state-injected once additional providers ship). When
/// the key is missing or the upstream fails, returns 503 so the
/// desktop can fall back to its existing path.
pub async fn feed_handler(
    State(_state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(query): Query<FeedQuery>,
    Json(body): Json<FeedBody>,
) -> Result<Json<FeedResponse>, AppError> {
    let tab = NewsTab::parse(&query.tab);

    let api_key = std::env::var("MIZAN_NEWSAPI_KEY").ok();

    let fetch_query = FetchQuery {
        query: query.query,
        language: query.language,
        page_size: query.page_size,
    };

    let client = reqwest::Client::new();
    let articles = match fetch_newsapi(&client, api_key.as_deref(), fetch_query).await {
        Ok(articles) => articles,
        Err(e) => return Err(map_newsapi_error(e)),
    };

    let ranking_input: RankingInput = match tab {
        NewsTab::Relevant => body.into(),
        NewsTab::Global => RankingInput::default(),
    };
    let ranked = rank_articles(&articles, &ranking_input);

    Ok(Json(FeedResponse {
        tab,
        provider: "newsapi",
        articles: ranked,
    }))
}

/// Translate a `NewsApiError` into the right `AppError` for the
/// HTTP surface. Missing API key → 503 (deploy-time issue); upstream
/// 4xx/5xx → 502 bad-gateway; parse failures → 502 (we don't trust
/// the provider's body, so it's the provider's fault, not ours).
fn map_newsapi_error(err: NewsApiError) -> AppError {
    match err {
        NewsApiError::MissingApiKey => AppError::new(
            ErrorCode::ServiceUnavailable,
            "news provider not configured",
        ),
        NewsApiError::Transport(msg) => AppError::new(
            ErrorCode::BadGateway,
            format!("news provider transport: {msg}"),
        ),
        NewsApiError::HttpError { status, message } => AppError::new(
            ErrorCode::BadGateway,
            format!("news provider returned {status}: {message}"),
        ),
        NewsApiError::BadBody(msg) => AppError::new(
            ErrorCode::BadGateway,
            format!("news provider sent malformed payload: {msg}"),
        ),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn default_tab_defaults_to_global() {
        let q: FeedQuery = serde_json::from_str("{}").expect("ok");
        assert_eq!(q.tab, "global");
        assert!(!q.query.is_empty());
        assert_eq!(q.page_size, 30);
        assert!(q.language.is_none());
    }

    #[test]
    fn feed_body_default_empty_lists() {
        let body: FeedBody = serde_json::from_str("{}").expect("ok");
        assert!(body.holding_symbols.is_empty());
        assert!(body.holding_categories.is_empty());
        assert!(body.memory_keywords.is_empty());
    }

    #[test]
    fn feed_body_into_ranking_input_preserves_fields() {
        let body = FeedBody {
            holding_symbols: vec!["AAPL".into()],
            holding_categories: vec![super::super::types::NewsCategory::Sukuks],
            memory_keywords: vec!["ramadan".into()],
        };
        let input: RankingInput = body.into();
        assert_eq!(input.holding_symbols, vec!["AAPL"]);
        assert_eq!(input.memory_keywords, vec!["ramadan"]);
        assert_eq!(input.holding_categories.len(), 1);
    }

    #[test]
    fn map_missing_api_key_to_service_unavailable() {
        let err = map_newsapi_error(NewsApiError::MissingApiKey);
        // The status is inferred from the code at response-render time —
        // we can render to a Response to check.
        let rendered = format!("{err:?}");
        assert!(rendered.contains("ServiceUnavailable"), "got: {rendered}");
    }

    #[test]
    fn map_transport_to_bad_gateway() {
        let err = map_newsapi_error(NewsApiError::Transport("DNS fail".into()));
        let rendered = format!("{err:?}");
        assert!(rendered.contains("BadGateway"), "got: {rendered}");
        assert!(rendered.contains("DNS fail"));
    }

    #[test]
    fn map_http_error_to_bad_gateway_with_status() {
        let err = map_newsapi_error(NewsApiError::HttpError {
            status: 429,
            message: "rate limited".into(),
        });
        let rendered = format!("{err:?}");
        assert!(rendered.contains("BadGateway"), "got: {rendered}");
        assert!(rendered.contains("429"));
        assert!(rendered.contains("rate limited"));
    }

    #[test]
    fn map_bad_body_to_bad_gateway() {
        let err = map_newsapi_error(NewsApiError::BadBody("not json".into()));
        let rendered = format!("{err:?}");
        assert!(rendered.contains("BadGateway"), "got: {rendered}");
        assert!(rendered.contains("not json"));
    }

    #[test]
    fn feed_query_parses_relevant() {
        let q = NewsTab::parse("relevant");
        assert_eq!(q, NewsTab::Relevant);
    }

    #[test]
    fn feed_query_unknown_tab_falls_through_to_global() {
        let q = NewsTab::parse("randomgarbage");
        assert_eq!(q, NewsTab::Global);
    }

    #[test]
    fn feed_body_deserialises_with_partial_fields() {
        let body: FeedBody =
            serde_json::from_str(r#"{"holdingSymbols":["DAR","EMAAR"]}"#).expect("ok");
        assert_eq!(body.holding_symbols, vec!["DAR", "EMAAR"]);
        assert!(body.memory_keywords.is_empty());
    }
}
