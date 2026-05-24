-- Mizan Connect — Plaid-first Gold live sync.
-- Forward-only. SnapTrade-era broker tables remain only as migration
-- compatibility; active sync uses the Plaid tables below.

ALTER TABLE subscriptions DROP CONSTRAINT IF EXISTS subscriptions_tier_check;
ALTER TABLE subscriptions ADD CONSTRAINT subscriptions_tier_check
    CHECK (tier IN (
        'silver', 'gold',
        'basic', 'essentials', 'duo', 'plus', 'pro', 'enterprise', 'advisor'
    ));

CREATE TABLE plaid_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    access_token_encrypted BYTEA NOT NULL,
    institution_id TEXT,
    institution_name TEXT,
    status TEXT NOT NULL DEFAULT 'connected',
    transactions_cursor TEXT,
    last_successful_sync_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, item_id)
);
CREATE INDEX idx_plaid_items_user_status
    ON plaid_items(user_id, status, updated_at DESC);

CREATE TABLE plaid_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    name TEXT,
    official_name TEXT,
    account_type TEXT,
    subtype TEXT,
    mask TEXT,
    balances_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, account_id)
);
CREATE INDEX idx_plaid_accounts_item ON plaid_accounts(user_id, item_id);

CREATE TABLE plaid_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    transaction_id TEXT NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    iso_currency_code TEXT,
    merchant_name TEXT,
    name TEXT,
    category_json JSONB,
    date DATE NOT NULL,
    pending BOOLEAN NOT NULL DEFAULT FALSE,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    removed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, transaction_id)
);
CREATE INDEX idx_plaid_transactions_account_date
    ON plaid_transactions(user_id, account_id, date DESC);
CREATE INDEX idx_plaid_transactions_item_date
    ON plaid_transactions(user_id, item_id, date DESC);

CREATE TABLE plaid_liabilities (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    liabilities_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, item_id)
);

CREATE TABLE plaid_investment_holdings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    security_id TEXT,
    holding_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    securities_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_plaid_holdings_item
    ON plaid_investment_holdings(user_id, item_id, account_id);

CREATE TABLE plaid_webhook_events (
    id BIGSERIAL PRIMARY KEY,
    item_id TEXT,
    webhook_type TEXT,
    webhook_code TEXT,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_plaid_webhook_events_item
    ON plaid_webhook_events(item_id, received_at DESC);

CREATE TRIGGER trg_plaid_items_updated_at
    BEFORE UPDATE ON plaid_items
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_plaid_accounts_updated_at
    BEFORE UPDATE ON plaid_accounts
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_plaid_transactions_updated_at
    BEFORE UPDATE ON plaid_transactions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_plaid_liabilities_updated_at
    BEFORE UPDATE ON plaid_liabilities
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
