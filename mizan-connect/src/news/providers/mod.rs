//! News providers — Track D PR-D2 / Goal v3 §V Phase 6.
//!
//! Each provider implements `async fn fetch(...) -> Result<Vec<RawArticle>>`
//! against its external API. Provider clients do NOT touch the
//! database — they're pure HTTP wrappers. The personalization layer
//! and the per-user materialization (PR-D4) consume their outputs.
//!
//! PR-D2 ships only NewsAPI. Subsequent PRs add:
//!   - PR-D2.a: Benzinga
//!   - PR-D2.b: Polygon
//!   - PR-D2.c: Refinitiv
//!   - PR-D2.d: Bondevalue (Sukuks specialist, critical for §23)
//!   - PR-D2.e: Regional feeds (CNA / Mint / Khaleej Times / IFN /
//!     Salaam Gateway)

pub mod newsapi;

pub use newsapi::{fetch as fetch_newsapi, NewsApiError};

/// List the providers shipped in the current build. Surfaced by
/// `/v1/news/health` so monitoring can confirm a fresh deploy
/// picked up new clients.
pub fn available_providers() -> Vec<&'static str> {
    vec!["newsapi"]
}
