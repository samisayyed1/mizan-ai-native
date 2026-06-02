-- ============================================================================
-- Track E PR-E1.a — holdings_metadata side table
-- ----------------------------------------------------------------------------
-- Per ADR 0011 (docs/adr/0011-holdings-metadata-design.md), the desktop's
-- holdings live as JSON inside portfolio_history.holdings (TEXT column),
-- not as relational rows. The Mizan Badge expansion (Track E) needs per-
-- holding provenance + AI-estimation + Sharia-screening + advisor-sign-off
-- metadata that doesn't fit cleanly into that JSON shape (indexing, schema
-- discoverability, foreign-key integrity, migration-friendliness).
--
-- This migration creates a side table keyed by the natural identity of
-- a holding row: (account_id, holding_symbol, as_of_date) matching how
-- holdings appear inside portfolio_history.holdings.
--
-- Reads do a LEFT JOIN against holdings_metadata; missing rows render
-- with badge defaults ('manual' origin, no modifiers). Sync writes (Plaid,
-- SnapTrade, Setu, etc.) UPSERT into this table with `origin` set from
-- the provider. The AAOIFI screening worker (Track E PR-E4) writes
-- sharia_status + last_screened_at. AI estimation pipelines (Track B
-- PR-B12/B15) write ai_estimated + range. Agent mutations (Track C) bump
-- agent_modified_at. Advisor sign-off (Track G) writes advisor_reviewed_*.
--
-- Modifier states ('stale', 'pending-reconciliation', 'audit-trail',
-- 'agent-modified' badge, 'mixed-compliance') are NOT stored — they are
-- computed at read time from this metadata + cache_policy TTLs +
-- reconciliation_queue presence + truth_ledger head + sharia_status enum.
--
-- caches-evicted: (none — holdings_metadata is durable state, not a cache)
-- ============================================================================

CREATE TABLE holdings_metadata (
    -- ── Composite natural key ─────────────────────────────────────────
    -- (account_id, holding_symbol, as_of_date) is the natural identity
    -- matching portfolio_history.holdings JSON entries on a given date.
    -- The composite-key approach avoids requiring a stable UUID per
    -- holding (which would require touching the JSON producers).
    account_id          TEXT NOT NULL,
    holding_symbol      TEXT NOT NULL,
    as_of_date          DATE NOT NULL,

    -- ── Origin / provenance ──────────────────────────────────────────
    -- Extends the existing sync_provider concept. New variants land here
    -- as Track B integrates each provider. The check constraint encodes
    -- the full allowed set today; new providers ALTER the constraint.
    origin              TEXT NOT NULL CHECK (origin IN (
        'manual',
        'plaid',
        'snaptrade',
        'csv',
        'example',
        'setu',
        'sgfindex',
        'tink',
        'basiq',
        'lean',
        'ccxt',
        'chain_reader',
        'twelve_data',
        'metalprice_api',
        'bondevalue'
    )),

    -- ── Sharia screening (Track E PR-E4) ─────────────────────────────
    -- NULL = not yet screened; 'unrated' = screening attempted but no
    -- definitive verdict. Distinguishing is important for the badge
    -- renderer: NULL renders no compliance badge; 'unrated' renders an
    -- explicit "screening attempted, no verdict" affordance.
    sharia_status       TEXT CHECK (
        sharia_status IS NULL
        OR sharia_status IN ('compliant', 'non_compliant', 'mixed', 'unrated')
    ),
    last_screened_at    DATETIME,

    -- ── AI estimation (Track B real estate + collectibles) ──────────
    -- ai_estimated is a boolean (0/1). Confidence is 'low' | 'mid' | 'high'
    -- per spec §5.8 (real estate range output) — these are intentionally
    -- discrete bins not a numeric score, to keep the UI surface honest.
    ai_estimated        INTEGER NOT NULL DEFAULT 0 CHECK (ai_estimated IN (0, 1)),
    ai_confidence       TEXT CHECK (
        ai_confidence IS NULL
        OR ai_confidence IN ('low', 'mid', 'high')
    ),
    ai_value_range_low  NUMERIC,
    ai_value_range_high NUMERIC,

    -- ── User tags ────────────────────────────────────────────────────
    -- JSON array (TEXT). Empty array represented as '[]'.
    tags                TEXT NOT NULL DEFAULT '[]',

    -- ── Advisor sign-off (Track G) ───────────────────────────────────
    advisor_reviewed_by TEXT,
    advisor_reviewed_at DATETIME,

    -- ── Agent mutation tracking (Track C) ───────────────────────────
    -- When the AI agent last mutated this holding's value/quantity/etc.
    -- The Mizan Badge 'agent-modified' modifier renders for 24h after.
    agent_modified_at   DATETIME,

    -- ── Audit columns ────────────────────────────────────────────────
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (account_id, holding_symbol, as_of_date),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- ── Indexes ──────────────────────────────────────────────────────────
-- The badge render-time lookup is by (account_id, holding_symbol,
-- latest as_of_date) — the primary key serves it directly.
--
-- The Sharia-screen worker scans by sharia_status + last_screened_at
-- to find rows due for re-screening. Partial index keeps it tiny.
CREATE INDEX idx_holdings_metadata_origin
    ON holdings_metadata(origin);

CREATE INDEX idx_holdings_metadata_sharia
    ON holdings_metadata(sharia_status)
    WHERE sharia_status IS NOT NULL;

CREATE INDEX idx_holdings_metadata_screened_at
    ON holdings_metadata(last_screened_at)
    WHERE last_screened_at IS NOT NULL;

-- AI-estimated rows lookup (real estate + collectibles dashboard surfaces)
CREATE INDEX idx_holdings_metadata_ai_estimated
    ON holdings_metadata(ai_estimated)
    WHERE ai_estimated = 1;

-- Agent-modified-recently lookup (powers the 'agent-modified' Mizan
-- Badge modifier — shown for 24h after mutation)
CREATE INDEX idx_holdings_metadata_agent_modified_at
    ON holdings_metadata(agent_modified_at)
    WHERE agent_modified_at IS NOT NULL;

-- Advisor-reviewed rows (Track G surfaces per-advisor reports)
CREATE INDEX idx_holdings_metadata_advisor_reviewed_by
    ON holdings_metadata(advisor_reviewed_by)
    WHERE advisor_reviewed_by IS NOT NULL;
