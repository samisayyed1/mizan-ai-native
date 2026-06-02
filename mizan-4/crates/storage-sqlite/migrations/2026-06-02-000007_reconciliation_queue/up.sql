-- ============================================================================
-- Track C — reconciliation_queue
-- ----------------------------------------------------------------------------
-- Per spec §5.2 (Brokerage Accounts) + §9.3 (ReconciliationRequired
-- notification) + spec §14.1:
--
-- > When sync detects a conflict, a reconciliation card in the notifications
-- > panel routes the user to resolve (keep manual / replace with sync / merge).
--
-- This table holds pending items; the agent helps the user clear them
-- conversationally per spec §15.3.
--
-- Durable state (not a cache) — items live here until the user resolves
-- them. The `'pending-reconciliation'` Mizan Badge modifier (spec §8.2)
-- renders for holdings that have an open queue entry.
--
-- caches-evicted: (none — durable state)
-- ============================================================================

CREATE TABLE reconciliation_queue (
    id                TEXT NOT NULL PRIMARY KEY,
    user_id           TEXT NOT NULL,
    account_id        TEXT NOT NULL,
    holding_symbol    TEXT,                            -- NULL for account-level conflicts
    -- The kind of conflict — drives the resolution UI affordance.
    conflict_kind     TEXT NOT NULL CHECK (
        conflict_kind IN (
            'manual_vs_sync_value',          -- user-entered value differs from sync
            'manual_vs_sync_quantity',       -- user-entered qty differs from sync
            'duplicate_transaction',         -- same activity from two providers
            'orphaned_position',             -- sync removed a holding the user has activities for
            'currency_mismatch'              -- sync sent a different currency than expected
        )
    ),
    -- The two competing values, stored as JSON for shape flexibility.
    manual_value_json TEXT,
    sync_value_json   TEXT,
    -- The provider that produced the sync side (plaid, snaptrade, etc.)
    sync_provider     TEXT NOT NULL,
    -- Status drives whether this still surfaces in the notifications panel.
    status            TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'resolved_keep_manual', 'resolved_replace_with_sync', 'resolved_merged', 'dismissed')
    ),
    resolved_at       DATETIME,
    resolved_by       TEXT,                            -- user_id of the human who resolved
    created_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- Pending-items lookup (the notification badge + reconciliation panel)
CREATE INDEX idx_reconciliation_queue_pending
    ON reconciliation_queue(user_id, created_at DESC)
    WHERE status = 'pending';

-- Per-holding lookup (powers the 'pending-reconciliation' Mizan Badge
-- modifier render)
CREATE INDEX idx_reconciliation_queue_holding_pending
    ON reconciliation_queue(account_id, holding_symbol)
    WHERE status = 'pending' AND holding_symbol IS NOT NULL;
