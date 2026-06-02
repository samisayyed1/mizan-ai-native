-- ============================================================================
-- Track D PR-D1 — news_items cache (multi-source curated news mesh)
-- ----------------------------------------------------------------------------
-- Per spec §10 (News Module) and `docs/plans/04-track-d.md`:
--
-- A two-tab News surface (Relevant / Global). Relevant items are ranked
-- per-user via cloud-side personalization worker (vector similarity over
-- user_memory + holdings); items are pulled to the desktop into this
-- cached table on app open and periodically.
--
-- "Relevant" vs "Global" distinguished by the `is_personalized` flag —
-- the relevance_score is meaningful only for is_personalized = 1 rows.
--
-- caches-evicted: news_items
-- ============================================================================

CREATE TABLE news_items (
    id                  TEXT NOT NULL PRIMARY KEY,
    user_id             TEXT,                          -- NULL for global feed items
    headline            TEXT NOT NULL,
    summary             TEXT,                          -- agent-generated, not publisher's stock dek
    source              TEXT NOT NULL,                 -- 'newsapi' | 'benzinga' | 'polygon' | 'refinitiv' | 'bondevalue' | regional codes
    source_url          TEXT NOT NULL,
    published_at        INTEGER NOT NULL,              -- unix seconds (matches market_news pattern)
    is_personalized     INTEGER NOT NULL DEFAULT 0 CHECK (is_personalized IN (0, 1)),
    relevance_score     REAL,                          -- only meaningful when is_personalized = 1 (0.0-1.0)
    why_matters         TEXT,                          -- agent reasoning for "why this matters to you" (relevant tab only)
    related_holding_ids TEXT NOT NULL DEFAULT '[]',    -- JSON array of holding identifiers
    read_state          TEXT NOT NULL DEFAULT 'unread' CHECK (
        read_state IN ('unread', 'read', 'saved', 'dismissed')
    ),
    created_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_news_items_user_personalized
    ON news_items(user_id, is_personalized, relevance_score DESC)
    WHERE is_personalized = 1;

CREATE INDEX idx_news_items_published
    ON news_items(published_at DESC);

CREATE INDEX idx_news_items_read_state
    ON news_items(user_id, read_state)
    WHERE user_id IS NOT NULL;

-- For the saved-list reading surface (long-press → save → view in "Saved")
CREATE INDEX idx_news_items_saved
    ON news_items(user_id, created_at DESC)
    WHERE read_state = 'saved';
