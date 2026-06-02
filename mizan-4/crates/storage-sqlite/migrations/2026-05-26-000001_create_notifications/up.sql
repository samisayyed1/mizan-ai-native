-- Personalized AI wealth notifications (Notify-3).
--
-- Each row is one user-visible insight (e.g. "PLTR up 8% today",
-- "International allocation drifted 12pp under target", "You hit
-- 75% of the House goal"). The insights engine in crates/core
-- writes here; the notification-center UI reads here; the
-- scheduler dispatches new rows to the OS notification system on
-- emit.
--
-- Why a normalized table and not JSON-blob-per-day like
-- `daily_briefs`? Notifications need per-row read/dismiss state,
-- per-row deep-link, and a queryable severity index for the
-- unread-badge count. Storing as one row per insight gives us
-- that for free.
--
-- IDEMPOTENCY
-- `dedupe_key` is what the insights engine emits to prevent
-- double-firing across runs. E.g. for BigDailyMove on PLTR
-- 2026-05-26 it's `big_move:PLTR:2026-05-26`. Re-running the same
-- rule on the same day must not produce a second row. Enforced
-- via UNIQUE on dedupe_key.

CREATE TABLE IF NOT EXISTS notifications (
    id           TEXT PRIMARY KEY,
    -- Stable category slug: 'big_move' | 'allocation_drift' |
    -- 'goal_milestone' | 'cash_drag' | 'dividend_posted' |
    -- 'new_ath' | 'net_worth_dip' | 'sync_failure' | 'ai_digest'.
    -- Frontend uses this for icon + grouping.
    kind         TEXT    NOT NULL,
    -- 'info' (default), 'success', 'warning', 'critical'. Drives
    -- color + whether we push to OS notification center (only
    -- warning + critical do, to respect the user's attention).
    severity     TEXT    NOT NULL DEFAULT 'info',
    title        TEXT    NOT NULL,
    body         TEXT    NOT NULL,
    -- Optional deep-link target the bell-panel item navigates to
    -- on tap. Format: `mizan://account/<id>` | `mizan://holding/<symbol>` |
    -- `mizan://goal/<id>` | `mizan://dashboard`. Resolved by the
    -- frontend NotificationItem.onClick handler. NULL = no nav.
    deep_link    TEXT,
    -- Free-form JSON for the rule that produced this row. The
    -- AI-digest synthesizer reads this to humanise the line, the
    -- UI may render structured chips from it (e.g. % change).
    payload_json TEXT    NOT NULL DEFAULT '{}',
    -- Idempotency key. UNIQUE — see header comment.
    dedupe_key   TEXT    NOT NULL,
    created_at   BIGINT  NOT NULL,
    read_at      BIGINT,
    dismissed_at BIGINT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedupe
    ON notifications(dedupe_key);

-- Bell-panel reads: "all not-dismissed, newest first" + unread count.
-- Composite on (dismissed_at IS NULL, created_at DESC) — SQLite
-- can't do partial+covering in one shot cleanly, so two:
CREATE INDEX IF NOT EXISTS idx_notifications_active_created
    ON notifications(dismissed_at, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_unread
    ON notifications(read_at, dismissed_at)
    WHERE read_at IS NULL AND dismissed_at IS NULL;
