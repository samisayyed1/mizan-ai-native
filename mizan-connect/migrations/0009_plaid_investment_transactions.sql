-- Mizan Connect — Plaid investment transactions sync.
--
-- The 0007 baseline shipped holdings snapshots but no traceability into
-- the trades that produced them. Per the production-readiness audit
-- (Plaid GAP 1, "valued but untraceable to trades"), we now persist the
-- full Plaid /investments/transactions/get feed — buys, sells, dividends,
-- fees, splits — keyed by Plaid's stable investment_transaction_id.
--
-- Unlike /transactions/sync which is cursor-based, /investments/transactions/get
-- is date-range-based. We track the last successful sync window so
-- incremental syncs only need to pull recent activity (with a 7-day
-- safety overlap to catch any broker-side amendments Plaid backdates).
--
-- Forward-only. Pre-Plaid-investment-transaction installs are migrated
-- by simply running this; first sync after deploy will backfill up to
-- 24 months from Plaid.

CREATE TABLE plaid_investment_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    investment_transaction_id TEXT NOT NULL,
    -- Plaid's classification of the trade. Common values:
    --   buy, sell, dividend, fee, cash, transfer, cancel.
    -- We store the raw string and let downstream layers map it to
    -- Mizan's activity types (BUY / SELL / DIVIDEND / FEE / etc.).
    type TEXT,
    -- More granular than `type`, e.g. "buy", "sell short", "qualified
    -- dividend", "long-term capital gain". May be NULL for some types.
    subtype TEXT,
    -- Plaid's `name` field — the human-readable description, e.g.
    -- "BUY APPLE INC", "DIVIDEND CASH".
    name TEXT,
    -- The Plaid security_id this transaction relates to. May be NULL
    -- for pure-cash events (deposit, fee, withdrawal).
    security_id TEXT,
    -- Monetary fields stored as NUMERIC to avoid FP drift. Plaid returns
    -- these as JSON numbers but we want exact arithmetic at the storage
    -- boundary.
    amount NUMERIC(18, 6),
    price NUMERIC(18, 6),
    quantity NUMERIC(18, 6),
    fees NUMERIC(18, 6),
    iso_currency_code TEXT,
    unofficial_currency_code TEXT,
    transaction_date DATE NOT NULL,
    -- Plaid marks some events as "cancelled" later. We treat cancel
    -- like the transactions/sync `removed` list — keep the row, flip
    -- the flag, let the desktop reconcile.
    cancelled BOOLEAN NOT NULL DEFAULT FALSE,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, investment_transaction_id)
);
CREATE INDEX idx_plaid_investment_transactions_account_date
    ON plaid_investment_transactions(user_id, account_id, transaction_date DESC);
CREATE INDEX idx_plaid_investment_transactions_item_date
    ON plaid_investment_transactions(user_id, item_id, transaction_date DESC);
CREATE INDEX idx_plaid_investment_transactions_security
    ON plaid_investment_transactions(user_id, security_id)
    WHERE security_id IS NOT NULL;

CREATE TRIGGER trg_plaid_investment_transactions_updated_at
    BEFORE UPDATE ON plaid_investment_transactions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Track the most-recent date we successfully pulled investment
-- transactions through. Incremental syncs use this minus a 7-day
-- safety overlap as the start_date. NULL means "this item has never
-- successfully synced investment transactions" → full 24-month
-- backfill on next sync.
ALTER TABLE plaid_items
    ADD COLUMN investment_transactions_sync_through TIMESTAMPTZ;
