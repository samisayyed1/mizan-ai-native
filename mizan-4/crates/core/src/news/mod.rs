//! Financial news orchestration: aggregates the multi-source mesh (TradingView,
//! MarketWatch, Yahoo) and serves it from a durable SQLite cache so the feed
//! never goes empty during a transient source outage.

use std::sync::Arc;

use async_trait::async_trait;
use log::warn;
use mizan_market_data::NewsProvider;

use crate::errors::Result;

/// Re-exported so storage/consumers can name the article type without a direct
/// dependency on the market-data crate.
pub use mizan_market_data::NewsArticle;

/// How many recent articles to keep/serve.
const NEWS_LIMIT: i64 = 100;

/// Persistence for the local news cache.
#[async_trait]
pub trait NewsRepositoryTrait: Send + Sync {
    /// Insert articles, ignoring ones already cached (dedup by `id`). Returns
    /// the number newly inserted.
    async fn upsert_news(&self, articles: &[NewsArticle]) -> Result<usize>;

    /// Most recent articles, newest first, capped at `limit`.
    fn get_recent_news(&self, limit: i64) -> Result<Vec<NewsArticle>>;
}

/// Orchestrates fetching the mesh, caching it, and serving from the cache.
pub struct NewsService {
    provider: NewsProvider,
    repo: Arc<dyn NewsRepositoryTrait>,
}

impl NewsService {
    pub fn new(repo: Arc<dyn NewsRepositoryTrait>) -> Self {
        Self {
            provider: NewsProvider::new(),
            repo,
        }
    }

    /// Refresh the mesh, persist whatever succeeded, then return from the cache
    /// (so an all-sources-down moment still serves the last good feed). When
    /// `symbols` is provided, filters to articles relevant to those holdings.
    pub async fn get_news(&self, symbols: Option<Vec<String>>) -> Vec<NewsArticle> {
        let fresh = self.provider.fetch_global_news_mesh().await;
        if !fresh.is_empty() {
            if let Err(e) = self.repo.upsert_news(&fresh).await {
                warn!("News cache upsert failed: {e}");
            }
        }

        // The cache is the source of truth; fall back to the fresh batch only
        // if the cache is unavailable/empty (e.g. first run or a write error).
        let mut articles = match self.repo.get_recent_news(NEWS_LIMIT) {
            Ok(cached) if !cached.is_empty() => cached,
            _ => fresh,
        };

        if let Some(symbols) = symbols {
            if !symbols.is_empty() {
                articles.retain(|a| a.matches_user_portfolio(&symbols));
            }
        }
        articles
    }
}
