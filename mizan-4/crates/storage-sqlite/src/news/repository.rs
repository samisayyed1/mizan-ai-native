use async_trait::async_trait;
use diesel::prelude::*;
use std::sync::Arc;

use super::model::MarketNewsDB;
use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::market_news::dsl as news_dsl;
use mizan_core::errors::Result;
use mizan_core::news::{NewsArticle, NewsRepositoryTrait};

/// How many articles to retain in the cache (oldest pruned on each upsert).
const CACHE_CAP: i64 = 500;

pub struct NewsRepository {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl NewsRepository {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl NewsRepositoryTrait for NewsRepository {
    async fn upsert_news(&self, articles: &[NewsArticle]) -> Result<usize> {
        if articles.is_empty() {
            return Ok(0);
        }
        let rows: Vec<MarketNewsDB> = articles.iter().map(MarketNewsDB::from).collect();

        self.writer
            .exec(move |conn| -> Result<usize> {
                let mut inserted = 0usize;
                // Dedup by primary key (id_hash); existing rows are kept as-is.
                // SQLite can't batch-insert with on_conflict, so insert per row
                // (the mesh yields ~100 rows per refresh — negligible).
                for row in &rows {
                    inserted += diesel::insert_into(news_dsl::market_news)
                        .values(row)
                        .on_conflict(news_dsl::id_hash)
                        .do_nothing()
                        .execute(conn)
                        .map_err(StorageError::from)?;
                }
                // Bound cache growth: keep only the newest CACHE_CAP rows.
                diesel::sql_query(
                    "DELETE FROM market_news WHERE id_hash NOT IN \
                     (SELECT id_hash FROM market_news ORDER BY published_at DESC LIMIT ?)",
                )
                .bind::<diesel::sql_types::BigInt, _>(CACHE_CAP)
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(inserted)
            })
            .await
    }

    fn get_recent_news(&self, limit: i64) -> Result<Vec<NewsArticle>> {
        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<MarketNewsDB> = news_dsl::market_news
            .order(news_dsl::published_at.desc())
            .limit(limit)
            .select(MarketNewsDB::as_select())
            .load::<MarketNewsDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(NewsArticle::from).collect())
    }
}
