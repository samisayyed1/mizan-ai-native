-- Mizan Connect — denormalize Plaid security fields onto
-- plaid_investment_transactions so the desktop can map a Plaid trade
-- straight to a local asset without an extra round-trip.
--
-- The Plaid /investments/transactions/get response includes a sibling
-- `securities` array; we previously stored only `security_id` on the
-- transaction, leaving the desktop unable to resolve the ticker/name
-- without an extra lookup we never exposed.
--
-- These three columns are denormalized intentionally: securities don't
-- mutate often, and a JOIN in the hot read path (every desktop sync)
-- would only buy us a small storage win at the cost of extra IO. If
-- the security's symbol/name/type change later, the next investment
-- transaction we receive for that security will refresh the columns
-- via the ON CONFLICT upsert.
--
-- Forward-only. Existing rows get NULL until next sync — first sync
-- after deploy backfills.

ALTER TABLE plaid_investment_transactions
    ADD COLUMN security_ticker_symbol TEXT,
    ADD COLUMN security_name          TEXT,
    -- Plaid security types: "cash", "cryptocurrency", "derivative",
    -- "equity", "etf", "fixed income", "loan", "mutual fund", "other".
    -- Stored verbatim; downstream maps to Mizan AssetKind.
    ADD COLUMN security_type          TEXT,
    -- ISO codes when Plaid has them — useful for cross-listing the
    -- same instrument across data providers (e.g. AAPL CUSIP vs ISIN).
    ADD COLUMN security_cusip         TEXT,
    ADD COLUMN security_isin          TEXT;

-- Index for symbol-based lookups (desktop ingestion's resolve-by-symbol
-- fallback when CUSIP/ISIN aren't available).
CREATE INDEX idx_plaid_investment_transactions_symbol
    ON plaid_investment_transactions(user_id, security_ticker_symbol)
    WHERE security_ticker_symbol IS NOT NULL;
