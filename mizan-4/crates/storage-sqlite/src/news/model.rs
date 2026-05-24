//! DB model for the local news cache (`market_news`).

use chrono::Utc;
use diesel::prelude::*;
use mizan_core::news::NewsArticle;

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::market_news)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MarketNewsDB {
    pub id_hash: String,
    pub source: String,
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    /// JSON array of "EXCHANGE:TICKER".
    pub related_symbols: String,
    pub urgency: Option<i64>,
    pub published_at: i64,
    pub created_at: i64,
}

impl From<&NewsArticle> for MarketNewsDB {
    fn from(a: &NewsArticle) -> Self {
        MarketNewsDB {
            id_hash: a.id.clone(),
            source: a.source.clone(),
            title: a.title.clone(),
            url: a.url.clone(),
            summary: a.summary.clone(),
            related_symbols: serde_json::to_string(&a.related_symbols)
                .unwrap_or_else(|_| "[]".to_string()),
            urgency: a.urgency,
            published_at: a.published,
            created_at: Utc::now().timestamp(),
        }
    }
}

impl From<MarketNewsDB> for NewsArticle {
    fn from(db: MarketNewsDB) -> Self {
        NewsArticle {
            id: db.id_hash,
            title: db.title,
            published: db.published_at,
            source: db.source,
            url: db.url,
            summary: db.summary,
            related_symbols: serde_json::from_str(&db.related_symbols).unwrap_or_default(),
            urgency: db.urgency,
        }
    }
}
