//! NewsAPI.org provider client — Track D PR-D2 / Goal v3 §V Phase 6.
//!
//! NewsAPI.org returns JSON of the shape documented at
//! <https://newsapi.org/docs/endpoints/everything>. We hit the
//! `/v2/everything` endpoint with a small set of query parameters
//! and translate the response into `RawArticle`s.
//!
//! # Authentication
//!
//! API key is read from `MIZAN_NEWSAPI_KEY` env via the app config
//! at startup. When the key is absent, `fetch` returns
//! `NewsApiError::MissingApiKey` so the caller can fall back to a
//! provider that doesn't require auth (none today; PR-D2.b adds
//! Benzinga's free tier).
//!
//! # Rate limits
//!
//! NewsAPI's free Developer tier caps at 100 requests/day. The
//! handler layer (PR-D2.b) wraps every call in the same per-user
//! rate limit + per-tenant cache used by `/v1/sharia/*` so we never
//! burst.
//!
//! # Determinism
//!
//! The fetch function is a thin HTTP wrapper. The parser
//! (`parse_response`) is a pure function over the response body —
//! tests pin the translation against fixture payloads from the
//! NewsAPI docs.

use serde::Deserialize;
use thiserror::Error;

use crate::news::types::RawArticle;

const NEWSAPI_BASE_URL: &str = "https://newsapi.org/v2/everything";

/// Per-call inputs. `query` is the NewsAPI `q` parameter — we accept
/// it pre-built by the handler. `language` defaults to `"en"` if
/// `None`. `page_size` is clamped to `[1, 100]` per NewsAPI's
/// hard limit.
#[derive(Debug, Clone)]
pub struct FetchQuery {
    pub query: String,
    pub language: Option<String>,
    pub page_size: u32,
}

impl FetchQuery {
    /// Normalise inputs against NewsAPI's documented limits.
    pub fn normalised(self) -> Self {
        let page_size = self.page_size.clamp(1, 100);
        Self { page_size, ..self }
    }
}

#[derive(Debug, Error)]
pub enum NewsApiError {
    /// `MIZAN_NEWSAPI_KEY` was not set in the environment.
    #[error("MIZAN_NEWSAPI_KEY is not configured")]
    MissingApiKey,

    /// The transport layer (DNS, TLS, timeout) failed.
    #[error("transport error: {0}")]
    Transport(String),

    /// NewsAPI returned a non-2xx status with an error payload.
    #[error("NewsAPI returned status {status}: {message}")]
    HttpError { status: u16, message: String },

    /// The response was 200 but the body didn't deserialize.
    #[error("failed to parse NewsAPI response: {0}")]
    BadBody(String),
}

/// Fetch a single page of articles from NewsAPI. The function is
/// generic over the HTTP client so the test suite can inject a
/// `mockito` server URL (the prod path uses `reqwest::Client::new()`).
pub async fn fetch(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: FetchQuery,
) -> Result<Vec<RawArticle>, NewsApiError> {
    fetch_with_base(client, NEWSAPI_BASE_URL, api_key, query).await
}

/// Variant of `fetch` that accepts a custom base URL — used by the
/// `mockito`-backed tests in this file to redirect traffic to the
/// test server.
pub async fn fetch_with_base(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    query: FetchQuery,
) -> Result<Vec<RawArticle>, NewsApiError> {
    let api_key = api_key.ok_or(NewsApiError::MissingApiKey)?;
    let q = query.normalised();
    let language = q.language.as_deref().unwrap_or("en");

    let response = client
        .get(base_url)
        .header("X-Api-Key", api_key)
        .query(&[
            ("q", q.query.as_str()),
            ("language", language),
            ("pageSize", &q.page_size.to_string()),
        ])
        .send()
        .await
        .map_err(|e| NewsApiError::Transport(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| NewsApiError::Transport(e.to_string()))?;

    if !status.is_success() {
        return Err(NewsApiError::HttpError {
            status: status.as_u16(),
            message: body,
        });
    }

    parse_response(&body)
}

#[derive(Debug, Deserialize)]
struct NewsApiResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    articles: Vec<NewsApiArticle>,
}

#[derive(Debug, Deserialize)]
struct NewsApiArticle {
    #[serde(default)]
    source: NewsApiSource,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "publishedAt")]
    published_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct NewsApiSource {
    #[serde(default)]
    name: Option<String>,
}

/// Translate the NewsAPI JSON body into `RawArticle`s. Exposed for
/// tests; production code calls it via [`fetch`].
pub fn parse_response(body: &str) -> Result<Vec<RawArticle>, NewsApiError> {
    let parsed: NewsApiResponse =
        serde_json::from_str(body).map_err(|e| NewsApiError::BadBody(e.to_string()))?;

    if parsed.status != "ok" {
        return Err(NewsApiError::BadBody(format!(
            "expected status \"ok\", got {:?}",
            parsed.status
        )));
    }

    let articles: Vec<RawArticle> = parsed
        .articles
        .into_iter()
        .filter_map(|a| {
            let title = a.title?;
            let url = a.url?;
            let summary = a.description.unwrap_or_default();
            // Truncate summary to <= 800 chars (long-form text doesn't belong in cards).
            let summary = if summary.len() > 800 {
                summary.chars().take(800).collect()
            } else {
                summary
            };
            Some(RawArticle::classify(
                "newsapi",
                url.clone(), // url is the most stable provider_id
                title,
                summary,
                url,
                a.source.name.unwrap_or_else(|| "Unknown".into()),
                a.published_at.unwrap_or_else(|| "".into()),
                Vec::new(), // NewsAPI doesn't carry tickers; derived later.
            ))
        })
        .collect();

    Ok(articles)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::news::types::NewsCategory;

    #[test]
    fn fetch_query_normalised_clamps_page_size() {
        let q = FetchQuery {
            query: "sukuk".into(),
            language: None,
            page_size: 5000,
        }
        .normalised();
        assert_eq!(q.page_size, 100);

        let q = FetchQuery {
            query: "sukuk".into(),
            language: None,
            page_size: 0,
        }
        .normalised();
        assert_eq!(q.page_size, 1);

        let q = FetchQuery {
            query: "sukuk".into(),
            language: None,
            page_size: 25,
        }
        .normalised();
        assert_eq!(q.page_size, 25);
    }

    #[test]
    fn parse_response_handles_canonical_payload() {
        // Shape from https://newsapi.org/docs/endpoints/everything
        let body = r#"{
            "status": "ok",
            "totalResults": 2,
            "articles": [
                {
                    "source": { "id": "reuters", "name": "Reuters" },
                    "author": "Jane Doe",
                    "title": "Emaar sukuk matures next month",
                    "description": "Investors weigh refinancing options",
                    "url": "https://example.com/article/1",
                    "urlToImage": null,
                    "publishedAt": "2026-06-01T12:00:00Z",
                    "content": null
                },
                {
                    "source": { "id": null, "name": "Bloomberg" },
                    "author": null,
                    "title": "Tesla earnings beat",
                    "description": "Strong share price reaction",
                    "url": "https://example.com/article/2",
                    "urlToImage": null,
                    "publishedAt": "2026-06-02T08:00:00Z",
                    "content": null
                }
            ]
        }"#;
        let articles = parse_response(body).expect("parse ok");
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].provider, "newsapi");
        assert_eq!(articles[0].source, "Reuters");
        assert_eq!(articles[0].category, NewsCategory::Sukuks);
        assert_eq!(articles[1].category, NewsCategory::Equities);
        assert_eq!(articles[0].published_at, "2026-06-01T12:00:00Z");
    }

    #[test]
    fn parse_response_skips_articles_missing_required_fields() {
        let body = r#"{
            "status": "ok",
            "articles": [
                { "source": { "name": "X" }, "title": null, "url": "https://e.com/1", "description": "...", "publishedAt": "2026-06-01T00:00:00Z" },
                { "source": { "name": "Y" }, "title": "Has title", "url": null, "description": "...", "publishedAt": "2026-06-01T00:00:00Z" },
                { "source": { "name": "Z" }, "title": "Complete", "url": "https://e.com/3", "description": "ok", "publishedAt": "2026-06-01T00:00:00Z" }
            ]
        }"#;
        let articles = parse_response(body).expect("parse ok");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Complete");
    }

    #[test]
    fn parse_response_handles_missing_description() {
        let body = r#"{
            "status": "ok",
            "articles": [
                { "source": { "name": "X" }, "title": "Title", "url": "https://e.com", "publishedAt": "2026-06-01T00:00:00Z" }
            ]
        }"#;
        let articles = parse_response(body).expect("parse ok");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].summary, "");
    }

    #[test]
    fn parse_response_truncates_long_summary() {
        let long = "a".repeat(2000);
        let body = format!(
            r#"{{"status":"ok","articles":[{{"source":{{"name":"X"}},"title":"T","url":"https://e.com","description":"{}","publishedAt":"2026-06-01T00:00:00Z"}}]}}"#,
            long
        );
        let articles = parse_response(&body).expect("parse ok");
        assert_eq!(articles[0].summary.chars().count(), 800);
    }

    #[test]
    fn parse_response_rejects_non_ok_status() {
        let body = r#"{"status":"error","message":"apiKeyInvalid"}"#;
        let err = parse_response(body).expect_err("should reject");
        let msg = err.to_string();
        assert!(msg.contains("status"), "got: {msg}");
    }

    #[test]
    fn parse_response_rejects_garbage_json() {
        let err = parse_response("not json at all").expect_err("should fail");
        assert!(matches!(err, NewsApiError::BadBody(_)));
    }

    #[test]
    fn parse_response_unknown_source_defaults_to_unknown() {
        let body = r#"{
            "status": "ok",
            "articles": [
                { "source": {}, "title": "T", "url": "https://e.com", "description": "x", "publishedAt": "2026-06-01T00:00:00Z" }
            ]
        }"#;
        let articles = parse_response(body).expect("parse ok");
        assert_eq!(articles[0].source, "Unknown");
    }

    #[tokio::test]
    async fn fetch_returns_missing_api_key_when_unset() {
        let client = reqwest::Client::new();
        let q = FetchQuery {
            query: "test".into(),
            language: None,
            page_size: 5,
        };
        let err = fetch(&client, None, q).await.expect_err("should fail");
        assert!(matches!(err, NewsApiError::MissingApiKey));
    }

    #[tokio::test]
    async fn fetch_handles_404_as_http_error() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let q = FetchQuery {
            query: "test".into(),
            language: None,
            page_size: 5,
        };
        let err = fetch_with_base(&client, &server.uri(), Some("test-key"), q)
            .await
            .expect_err("should fail");
        match err {
            NewsApiError::HttpError { status, .. } => assert_eq!(status, 404),
            _ => panic!("expected HttpError, got {:?}", err),
        }
    }

    #[tokio::test]
    async fn fetch_happy_path_parses_articles() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = r#"{
            "status": "ok",
            "articles": [
                {"source":{"name":"Reuters"},"title":"AAPL story","description":"earnings","url":"https://e.com/1","publishedAt":"2026-06-01T00:00:00Z"}
            ]
        }"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let q = FetchQuery {
            query: "apple".into(),
            language: Some("en".into()),
            page_size: 10,
        };
        let articles = fetch_with_base(&client, &server.uri(), Some("test-key"), q)
            .await
            .expect("ok");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "AAPL story");
    }
}
