-- §v3.1 foundation persistence: §A4 sync run ledger, §A12 NW snapshots,
-- §A22 daily briefs, §A1/§A2 truth ledger. Each table is append-only at
-- the application layer; we still expose UPDATE for upsert paths (e.g.
-- close a SyncRunEntry).

-- §A4 — every sync attempt across providers (Plaid, SnapTrade, Yahoo,
-- TradingView, MarketData, CsvImport, AiTool, FxRefresh, Manual).
CREATE TABLE IF NOT EXISTS sync_run_ledger (
    run_id        TEXT PRIMARY KEY NOT NULL,
    provider      TEXT NOT NULL,
    mode          TEXT NOT NULL,
    account_id    TEXT,
    thread_id     TEXT,
    started_at    INTEGER NOT NULL,           -- unix milliseconds (UTC)
    finished_at   INTEGER,                    -- NULL until the run completes
    outcome       TEXT NOT NULL,              -- applied|applied_with_warnings|cancelled|failed
    summary_json  TEXT NOT NULL DEFAULT '{}', -- SyncRunSummary serialised
    error_json    TEXT,                       -- §A24 envelope when outcome=failed
    created_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_sync_run_ledger_started_at
    ON sync_run_ledger (started_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_run_ledger_provider_started
    ON sync_run_ledger (provider, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_run_ledger_account
    ON sync_run_ledger (account_id, started_at DESC);

-- §A12 — daily net-worth snapshots. One row per (snapshot_date, base_currency).
-- breakdown_json holds the per-category breakdown (NetWorthBreakdownEntry).
CREATE TABLE IF NOT EXISTS net_worth_snapshots (
    snapshot_date     TEXT NOT NULL,           -- ISO date 'YYYY-MM-DD'
    base_currency     TEXT NOT NULL,
    total_assets      TEXT NOT NULL,           -- Decimal as string
    total_liabilities TEXT NOT NULL,           -- Decimal as string
    net_worth         TEXT NOT NULL,           -- assets - liabilities
    breakdown_json    TEXT NOT NULL DEFAULT '[]',
    source            TEXT NOT NULL,           -- app_open | manual | scheduled
    captured_at       INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    PRIMARY KEY (snapshot_date, base_currency)
);

CREATE INDEX IF NOT EXISTS idx_nw_snapshots_date
    ON net_worth_snapshots (snapshot_date DESC);

-- §A22 — daily Investor Brief. One row per (brief_date, base_currency).
CREATE TABLE IF NOT EXISTS daily_briefs (
    brief_date     TEXT NOT NULL,              -- ISO date
    base_currency  TEXT NOT NULL,
    payload_json   TEXT NOT NULL,              -- DailyBrief serialised
    delivered_at   INTEGER,                    -- NULL = not yet sent
    created_at     INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    PRIMARY KEY (brief_date, base_currency)
);

CREATE INDEX IF NOT EXISTS idx_daily_briefs_date
    ON daily_briefs (brief_date DESC);

-- §A1/§A2 — immutable hash-chained truth ledger. Append-only at the
-- application layer; the (id) primary key + sequence uniqueness make
-- ledger tampering detectable by replay-verify.
CREATE TABLE IF NOT EXISTS truth_ledger_entries (
    id           TEXT PRIMARY KEY NOT NULL,
    sequence     INTEGER NOT NULL UNIQUE,
    kind         TEXT NOT NULL,
    recorded_at  INTEGER NOT NULL,              -- unix milliseconds (UTC)
    account_id   TEXT,
    asset_id     TEXT,
    amount       TEXT,                          -- Decimal as string (None = no amount)
    currency     TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    prev_hash    TEXT NOT NULL,                 -- 64 lowercase hex chars
    entry_hash   TEXT NOT NULL                  -- 64 lowercase hex chars
);

CREATE INDEX IF NOT EXISTS idx_truth_ledger_sequence
    ON truth_ledger_entries (sequence);
CREATE INDEX IF NOT EXISTS idx_truth_ledger_account
    ON truth_ledger_entries (account_id, sequence);
CREATE INDEX IF NOT EXISTS idx_truth_ledger_kind
    ON truth_ledger_entries (kind, sequence);

-- §A1/§A2 — durable retry queue for ledger appends that failed on the
-- write path (e.g. ledger backend transient error after the activity
-- row already committed). A startup task drains the queue.
CREATE TABLE IF NOT EXISTS truth_ledger_retry_queue (
    id            TEXT PRIMARY KEY NOT NULL,    -- AppendInput.id (one retry per source)
    payload_json  TEXT NOT NULL,                -- serialised AppendInput
    queued_at     INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    attempts      INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT
);
