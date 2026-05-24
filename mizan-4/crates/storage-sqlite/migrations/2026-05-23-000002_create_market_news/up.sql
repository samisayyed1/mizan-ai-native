-- Local cache for the multi-source news mesh. id_hash is the deterministic
-- SHA-256 of the normalized headline, so the same story from different sources
-- collapses to one row (INSERT OR IGNORE on the primary key).
CREATE TABLE IF NOT EXISTS market_news (
    id_hash TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    summary TEXT,
    related_symbols TEXT NOT NULL DEFAULT '[]', -- JSON array of "EXCHANGE:TICKER"
    urgency INTEGER,
    published_at INTEGER NOT NULL,              -- unix seconds
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_market_news_published ON market_news (published_at DESC);
