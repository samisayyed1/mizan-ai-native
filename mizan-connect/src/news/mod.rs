//! News module — Track D PR-D2 / Goal v3 §V Phase 6.
//!
//! Implements the cloud side of the Mizan news layer per Evolution
//! Spec §8 + the autonomous loop directive `Mizan_Continue_Autonomous.md`
//! lines 32-36. Three layers:
//!
//! 1. **Provider clients** — `providers/` contains one Rust client per
//!    external news source. PR-D2 ships NewsAPI.org; additional
//!    providers (Benzinga / Polygon / Refinitiv / Bondevalue / regional
//!    feeds) land in PR-D2.a + later. Every client returns the same
//!    `RawArticle` shape so the upstream is provider-agnostic.
//!
//! 2. **Personalization** — `personalization` ranks `RawArticle`s
//!    against a user's holdings + `user_memory` facts. The ranking is
//!    pure logic — no I/O, no randomness — so the same input
//!    deterministically produces the same ranking. Tracked separately
//!    as PR-D3, scaffolded here as a stub trait.
//!
//! 3. **Handlers** — `handlers` exposes `GET /v1/news/feed?tab=...`
//!    over the provider+personalization stack. Caching, rate limits,
//!    and the per-user materialized feed land in PR-D4 alongside the
//!    desktop sync layer.
//!
//! # Layering rules
//!
//! - Provider clients DO NOT read from the database. They are pure
//!   `async fn(&Client, &Query) -> Result<Vec<RawArticle>>`.
//! - Personalization DOES NOT call providers. It accepts pre-fetched
//!   `RawArticle`s as input.
//! - Handlers compose providers + personalization + caching.
//!
//! This keeps the 95%-coverage floor mechanical: provider clients
//! mock the HTTP layer with `mockito`, personalization runs against
//! fixed in-memory inputs, handlers exercise the composed surface.
//!
//! # Out of scope (deferred)
//!
//! - The `news_items` per-user materialized view (PR-D4)
//! - The desktop sync pipeline that pulls from this endpoint
//! - Per-region feeds (CNA / Mint / Khaleej Times / IFN / Salaam Gateway)
//! - Refinitiv / Benzinga / Polygon / Bondevalue providers
//! - The "Discuss with Mizan" handoff to the agent
//! - The `'mcp'`-style provider trust badge

pub mod handlers;
pub mod personalization;
pub mod pgvector_similarity;
pub mod providers;
pub mod types;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub use personalization::{rank_articles, RankedArticle, RankingInput};
pub use pgvector_similarity::{
    cosine_similarity, rank_with_memory_embeddings, ArticleEmbedding, UserMemoryEmbedding,
};
pub use types::{NewsCategory, NewsTab, RawArticle};

/// Subrouter for `/v1/news/*` endpoints — merged into the v1 tree.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/news/health", get(health_check))
        .route("/news/feed", post(handlers::feed_handler))
}

/// Trivial health endpoint so the news router is non-empty before
/// PR-D2.b adds the real `feed` handler. Returns 200 with the
/// provider catalogue so `/v1/news/health` can be probed by uptime
/// monitors.
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "providers": providers::available_providers(),
    }))
}
