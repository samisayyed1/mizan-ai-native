-- Reverts FK + JSON CHECK by re-creating both tables without those
-- constraints and copying data back. Index list matches the original
-- pre-migration state from 2025-04-21.

CREATE TABLE holdings_snapshots_old (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    snapshot_date DATE NOT NULL,
    currency TEXT NOT NULL,
    positions TEXT NOT NULL DEFAULT '{}',
    cash_balances TEXT NOT NULL DEFAULT '{}',
    realized_gains TEXT NOT NULL DEFAULT '{}',
    cost_basis TEXT NOT NULL DEFAULT '0.0',
    net_contribution TEXT NOT NULL DEFAULT '0.0',
    net_contribution_base TEXT NOT NULL DEFAULT '0',
    cash_total_account_currency TEXT NOT NULL DEFAULT '0',
    cash_total_base_currency TEXT NOT NULL DEFAULT '0',
    source TEXT NOT NULL DEFAULT 'CALCULATED',
    calculated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO holdings_snapshots_old SELECT * FROM holdings_snapshots;
DROP TABLE holdings_snapshots;
ALTER TABLE holdings_snapshots_old RENAME TO holdings_snapshots;

CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_account_date ON holdings_snapshots (account_id, snapshot_date);
CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_date ON holdings_snapshots (snapshot_date);
CREATE INDEX IF NOT EXISTS idx_holdings_snapshots_account_id ON holdings_snapshots (account_id);

CREATE TABLE daily_account_valuation_old (
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
    calculated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO daily_account_valuation_old SELECT * FROM daily_account_valuation;
DROP TABLE daily_account_valuation;
ALTER TABLE daily_account_valuation_old RENAME TO daily_account_valuation;

CREATE INDEX IF NOT EXISTS idx_daily_account_valuation_account_date ON daily_account_valuation(account_id, valuation_date);
