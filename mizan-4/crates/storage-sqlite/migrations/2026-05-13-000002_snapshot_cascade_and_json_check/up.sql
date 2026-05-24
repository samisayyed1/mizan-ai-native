-- Production hardening: add ON DELETE CASCADE foreign keys + JSON
-- validity CHECK constraints to holdings_snapshots and
-- daily_account_valuation.
--
-- WHY
-- The 2025-04-21 init created both tables WITHOUT FK constraints,
-- so deleting an account left orphan snapshot rows behind. PR #18
-- shipped a defensive cleanup at the repository layer, so user
-- data isn't actively leaking — but the schema is structurally
-- weaker than it should be for soft-launch. This migration closes
-- it at the schema layer so anyone bypassing the repo (manual SQL,
-- future ingestion paths, broker sync edge cases) physically
-- cannot orphan rows.
--
-- The JSON blob columns (`positions`, `cash_balances`,
-- `realized_gains`) had no validity guarantee. Corrupted JSON
-- silently parsed as the default empty object downstream, hiding
-- data integrity issues. CHECK constraints reject malformed JSON
-- at write time so corruption is loud, not silent.
--
-- HOW (SQLite table-swap pattern)
-- SQLite can't ALTER TABLE to add a foreign key or a CHECK; we
-- have to:
--   1. Create a new table with the constraints.
--   2. Copy data over.
--   3. Drop the old table.
--   4. Rename new → original name.
--   5. Recreate the indexes (drop_table also drops its indexes).
--
-- This runs inside Diesel's per-migration transaction, so either
-- the entire swap commits or nothing changes. PRAGMA foreign_keys
-- is left in its default state (Diesel enables it per-connection).

-- ─────────────────────────────────────────────────────────────────────
-- holdings_snapshots
-- ─────────────────────────────────────────────────────────────────────

CREATE TABLE holdings_snapshots_new (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    snapshot_date DATE NOT NULL,
    currency TEXT NOT NULL,

    -- Store complex data as JSON strings, validated at write time.
    -- A NULL `realized_gains` is tolerated for compatibility with
    -- older rows that pre-date the column, but anything else must
    -- be a valid JSON object/array string.
    positions TEXT NOT NULL DEFAULT '{}',
    cash_balances TEXT NOT NULL DEFAULT '{}',
    realized_gains TEXT NOT NULL DEFAULT '{}',

    -- Store Decimals as TEXT.
    cost_basis TEXT NOT NULL DEFAULT '0.0',
    net_contribution TEXT NOT NULL DEFAULT '0.0',
    net_contribution_base TEXT NOT NULL DEFAULT '0',
    cash_total_account_currency TEXT NOT NULL DEFAULT '0',
    cash_total_base_currency TEXT NOT NULL DEFAULT '0',

    source TEXT NOT NULL DEFAULT 'CALCULATED',

    calculated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    -- Production hardening: orphan rows on account deletion are now
    -- impossible at the schema layer.
    CONSTRAINT fk_holdings_snapshots_account
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,

    -- Production hardening: reject malformed JSON at write time so
    -- corruption is loud, not silently defaulted to {}.
    CONSTRAINT chk_positions_json CHECK (json_valid(positions)),
    CONSTRAINT chk_cash_balances_json CHECK (json_valid(cash_balances)),
    CONSTRAINT chk_realized_gains_json CHECK (json_valid(realized_gains))
);

-- Copy every row from the old table. Column order matches the
-- create above. Source column may be missing if the data predates
-- the 2026-01-26 tracking_mode migration on some installs — that
-- migration sets a default 'CALCULATED' so this should be safe.
INSERT INTO holdings_snapshots_new (
    id, account_id, snapshot_date, currency,
    positions, cash_balances, realized_gains,
    cost_basis, net_contribution, net_contribution_base,
    cash_total_account_currency, cash_total_base_currency,
    source, calculated_at
)
SELECT
    id, account_id, snapshot_date, currency,
    positions, cash_balances, realized_gains,
    cost_basis, net_contribution, net_contribution_base,
    cash_total_account_currency, cash_total_base_currency,
    source, calculated_at
FROM holdings_snapshots;

DROP TABLE holdings_snapshots;
ALTER TABLE holdings_snapshots_new RENAME TO holdings_snapshots;

-- Recreate the indexes; DROP TABLE removed them.
CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_account_date
  ON holdings_snapshots (account_id, snapshot_date);
CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_date
  ON holdings_snapshots (snapshot_date);
CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_account_id
  ON holdings_snapshots (account_id);

-- ─────────────────────────────────────────────────────────────────────
-- daily_account_valuation
-- ─────────────────────────────────────────────────────────────────────

CREATE TABLE daily_account_valuation_new (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    valuation_date DATE NOT NULL,
    account_currency TEXT NOT NULL,
    base_currency TEXT NOT NULL,
    fx_rate_to_base TEXT NOT NULL,
    cash_balance TEXT NOT NULL,
    investment_market_value TEXT NOT NULL,
    total_value TEXT NOT NULL,
    cost_basis TEXT NOT NULL,
    net_contribution TEXT NOT NULL,
    calculated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CONSTRAINT fk_daily_account_valuation_account
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

INSERT INTO daily_account_valuation_new (
    id, account_id, valuation_date, account_currency, base_currency,
    fx_rate_to_base, cash_balance, investment_market_value, total_value,
    cost_basis, net_contribution, calculated_at
)
SELECT
    id, account_id, valuation_date, account_currency, base_currency,
    fx_rate_to_base, cash_balance, investment_market_value, total_value,
    cost_basis, net_contribution, calculated_at
FROM daily_account_valuation;

DROP TABLE daily_account_valuation;
ALTER TABLE daily_account_valuation_new RENAME TO daily_account_valuation;

CREATE INDEX IF NOT EXISTS idx_daily_account_valuation_account_date
  ON daily_account_valuation(account_id, valuation_date);
-- Standalone date index added in 2026-05-13-000001 (production hot
-- path indexes) is dropped along with the old table; recreate.
CREATE INDEX IF NOT EXISTS ix_daily_account_valuation_date
  ON daily_account_valuation(valuation_date);
