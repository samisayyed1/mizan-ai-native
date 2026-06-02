-- ============================================================================
-- Track C PR-C14 (foundation) — agent_audit_log
-- ----------------------------------------------------------------------------
-- Per spec §7.1 and working-agreement §11 (AI Agent Development):
--
-- > Every tool call emits a Truth Ledger entry where the operation is
-- > financial (any mutation to balances or holdings). The chain integrity
-- > property — one tampered row breaks the chain → instant detection —
-- > must hold for AI-initiated writes exactly as it does for user-initiated
-- > writes.
--
-- This table is the per-tool-call audit beyond what's in the AI Safety
-- Runtime (§A6) — useful for end-user transparency ("which agent action
-- produced this number"), debug bundles, and reasoning-trail surfaces.
--
-- 12-month full-resolution retention per working-agreement §18.3, then
-- archived to Mizan Connect cold storage via the ArchiveThenDelete
-- eviction strategy (cache_policy.rs entry).
--
-- caches-evicted: agent_audit_log
-- ============================================================================

CREATE TABLE agent_audit_log (
    id                TEXT NOT NULL PRIMARY KEY,
    user_id           TEXT NOT NULL,
    -- The conversation/session this tool call belongs to. Lets us link
    -- many tool calls to one user message.
    session_id        TEXT,
    -- The specific tool that was invoked (registered name from dispatcher).
    tool_name         TEXT NOT NULL,
    -- Sequence number within the session for ordering.
    sequence_in_session INTEGER NOT NULL DEFAULT 0,
    -- Per-turn-cap weight consumed by this call (AI Safety Runtime).
    cap_weight        INTEGER NOT NULL DEFAULT 1,
    -- Input parameters as JSON. May contain PII; redacted on diagnostic
    -- bundle export (§A17).
    params_json       TEXT NOT NULL,
    -- Result digest (blake3 of result JSON) — used to cross-reference
    -- Truth Ledger entries for financial-mutating tools.
    result_digest     TEXT NOT NULL,
    -- Did this call write to truth_ledger? Lets us audit "every financial-
    -- mutating tool emitted a ledger entry" trivially.
    emitted_truth_ledger INTEGER NOT NULL DEFAULT 0 CHECK (
        emitted_truth_ledger IN (0, 1)
    ),
    -- Wall-clock duration of the tool call (ms).
    duration_ms       INTEGER NOT NULL,
    -- Outcome: 'ok' | 'error' | 'safety_blocked'
    outcome           TEXT NOT NULL CHECK (
        outcome IN ('ok', 'error', 'safety_blocked')
    ),
    error_message     TEXT,
    created_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Per-user read path (memory editor + diagnostic bundle)
CREATE INDEX idx_agent_audit_log_user
    ON agent_audit_log(user_id, created_at DESC);

-- Per-session read path (reasoning-trail surface)
CREATE INDEX idx_agent_audit_log_session
    ON agent_audit_log(session_id, sequence_in_session)
    WHERE session_id IS NOT NULL;

-- Per-tool analytics (which tools are hot, which fail often)
CREATE INDEX idx_agent_audit_log_tool_outcome
    ON agent_audit_log(tool_name, outcome);

-- The ArchiveThenDelete worker uses this to find rows past 12mo
CREATE INDEX idx_agent_audit_log_created_at
    ON agent_audit_log(created_at);
