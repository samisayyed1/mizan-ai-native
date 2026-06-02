-- ============================================================================
-- Track C PR-C1.a — user_memory long-term memory layer
-- ----------------------------------------------------------------------------
-- Per spec §7.3 (Memory Layer): the AI agent maintains a per-user
-- persistent store of facts across sessions. The agent's memory writer
-- subroutine is the ONLY path that writes here — implicit memory writes
-- from tool handlers are forbidden (working agreement §11).
--
-- Facts examples (spec §7.3):
--   "User's risk tolerance is moderate-aggressive."
--   "User prefers Sharia-compliant investments following AAOIFI standard."
--   "User holds Hanafi school for fiqh-related decisions."
--   "User pays Zakat in Ramadan each year."
--   "User dislikes ULIPs and is considering surrendering Bajaj Pure Stock."
--
-- Memory has GDPR / DPDP / CCPA implications: users have the right to view,
-- edit, and delete every fact. Track C PR-C2 ships the editor UI.
--
-- Per the working agreement §11: "The memory writer is the only function
-- allowed to write to `user_memory`. Don't bypass it from tool handlers —
-- implicit memory writes pollute the store."
--
-- caches-evicted: (none — user_memory is per-user durable state, not a cache;
--                  per-row TTL handled via the expires_at column managed by
--                  the writer subroutine: 'agent-inferred' source → 12mo TTL,
--                  'user-stated' source → never expires)
-- ============================================================================

CREATE TABLE user_memory (
    -- ── Identity ─────────────────────────────────────────────────────
    id              TEXT NOT NULL PRIMARY KEY,
    user_id         TEXT NOT NULL,

    -- ── Content ──────────────────────────────────────────────────────
    fact_text       TEXT NOT NULL,
    -- Vector embedding for semantic search (sqlite-vec BLOB or JSON
    -- depending on the chosen vec backend at Track C PR-C1.b wiring).
    -- Nullable so a fact can be written before its embedding is computed;
    -- a background worker fills missing embeddings asynchronously.
    embedding       BLOB,

    -- ── Categorisation ──────────────────────────────────────────────
    category        TEXT NOT NULL CHECK (
        category IN ('preference', 'goal', 'context', 'constraint')
    ),

    -- ── Confidence (per spec §7.3) ──────────────────────────────────
    -- 0.0–1.0 confidence the agent has in this fact's correctness.
    -- User-stated facts default to 1.0; agent-inferred facts default
    -- to the agent's own confidence at write time.
    confidence      REAL NOT NULL DEFAULT 1.0 CHECK (
        confidence >= 0.0 AND confidence <= 1.0
    ),

    -- ── Provenance ──────────────────────────────────────────────────
    -- 'user-stated': the user said this in a conversation
    -- 'agent-inferred': the agent deduced this from observed behaviour
    -- 'system-derived': computed from app state (e.g. user's base currency)
    source          TEXT NOT NULL CHECK (
        source IN ('user-stated', 'agent-inferred', 'system-derived')
    ),

    -- ── Lifecycle ───────────────────────────────────────────────────
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at    DATETIME,
    expires_at      DATETIME,  -- NULL = never; agent-inferred default to created_at + 12mo
    -- Soft-delete column for GDPR / DPDP right-to-rectification before
    -- the cryptographic shredding window (working agreement §16.4 + §A20).
    deleted_at      DATETIME
);

-- ── Indexes ──────────────────────────────────────────────────────────

-- Per-user reads (the dominant query — every memory pull at agent turn)
CREATE INDEX idx_user_memory_user
    ON user_memory(user_id)
    WHERE deleted_at IS NULL;

-- Expiry sweep — finds rows past their expires_at for cleanup
-- (the cache_policy eviction worker doesn't touch user_memory because
--  per-row TTLs are heterogeneous; a dedicated cleanup runs in the
--  memory writer module at agent-turn cadence)
CREATE INDEX idx_user_memory_expires_at
    ON user_memory(expires_at)
    WHERE expires_at IS NOT NULL AND deleted_at IS NULL;

-- Last-used ordering for LRU eviction when a user's memory exceeds budget
CREATE INDEX idx_user_memory_last_used_at
    ON user_memory(user_id, last_used_at DESC NULLS LAST)
    WHERE deleted_at IS NULL;

-- Category filter (the memory editor UI filters by preference/goal/context/constraint)
CREATE INDEX idx_user_memory_category
    ON user_memory(user_id, category)
    WHERE deleted_at IS NULL;
