-- The TOTAL portfolio is a synthetic account_id used by the snapshot
-- service to aggregate every individual account's keyframes into a
-- single time-series view. It's referenced by holdings_snapshots,
-- daily_account_valuation, and the dashboard's "All Portfolios"
-- headline number.
--
-- BUG (QA Pass 6, 2026-05-25)
-- Migration 2026-05-13-000002 added a FOREIGN KEY constraint
-- `fk_holdings_snapshots_account` to holdings_snapshots that points
-- at accounts(id). The TOTAL synthetic account_id had never been
-- inserted into the `accounts` table — it was treated as a virtual
-- name in the service layer. The new FK constraint started rejecting
-- every TOTAL snapshot insert with a "FOREIGN KEY constraint failed"
-- error, leaving the dashboard's headline number completely empty.
--
-- FIX
-- Insert a placeholder row for the TOTAL account so the FK passes.
-- The account is is_active=1 (otherwise it'd be filtered out of
-- compute paths) but is_default=0 and tracking_mode='NOT_SET' so it
-- behaves as a tombstone rather than an editable account. Several
-- repository queries already filter on `account_id != 'TOTAL'`, so
-- the rest of the codebase already understood the synthetic-account
-- pattern — we just need the FK target to physically exist.

INSERT OR IGNORE INTO accounts (
    id,
    name,
    account_type,
    "group",
    currency,
    is_default,
    is_active,
    tracking_mode,
    is_archived,
    created_at,
    updated_at
) VALUES (
    'TOTAL',
    'All Portfolios (synthetic aggregate — do not edit)',
    'SECURITIES',
    NULL,
    'USD',
    0,
    1,
    'NOT_SET',
    0,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
);
