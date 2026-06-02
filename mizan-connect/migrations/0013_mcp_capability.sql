-- ============================================================================
-- Track K PR-K1 — MCP Capability (Gold+)
-- ----------------------------------------------------------------------------
-- Per spec §21 (MCP Capability) and `docs/plans/11-track-k.md`:
--
-- Model Context Protocol support is a Gold tier and above exclusive.
-- Users register MCP servers; the Mizan AI agent calls those tools alongside
-- its native registry. **The sandboxing rules in §21.3 are non-negotiable**
-- (working agreement §3.4): MCP tools may NEVER write to truth_ledger,
-- holdings, activities, balances, or any core financial state.
--
-- 4 tables:
--   mcp_servers   — per-user registered servers
--   mcp_catalog   — Mizan-curated public catalog of vetted servers
--   mcp_call_log  — every call logged with digests (no raw payload retention)
--
-- Plus the scratchpad lives client-side per working-agreement §3.4 —
-- desktop migration to follow.
-- ============================================================================

CREATE TABLE mcp_catalog (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_name         TEXT NOT NULL UNIQUE,
    display_name        TEXT NOT NULL,
    description         TEXT NOT NULL,
    server_url          TEXT NOT NULL,
    -- Tool catalog refreshed on demand; cached 24h per spec §21.2.
    tools_discovered    TEXT,                                  -- JSON
    tools_refreshed_at  TIMESTAMPTZ,
    -- Per spec §21.5: security review status.
    security_review_status TEXT NOT NULL DEFAULT 'pending' CHECK (
        security_review_status IN ('approved', 'pending', 'delisted')
    ),
    last_reviewed_at    TIMESTAMPTZ,
    reviewer_id         UUID REFERENCES users(id),
    delist_reason       TEXT,
    -- Trust level per ADR 0048 (planned) — schema column ready, NEVER
    -- honoured by any code path until security review explicitly authorises.
    trust_level         TEXT NOT NULL DEFAULT 'untrusted' CHECK (
        trust_level IN ('untrusted', 'trusted')
    ),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_mcp_catalog_approved ON mcp_catalog(security_review_status)
    WHERE security_review_status = 'approved';

CREATE TABLE mcp_servers (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Either from the catalog OR self-registered. self-registered carries
    -- a warning badge + requires user acknowledgment per spec §21.5.
    source              TEXT NOT NULL CHECK (source IN ('catalog', 'self_registered')),
    catalog_entry_id    UUID REFERENCES mcp_catalog(id),
    -- For self-registered servers, the user-provided URL + auth method.
    custom_server_url   TEXT,
    auth_method         TEXT,
    display_name        TEXT NOT NULL,
    -- Encrypted user-supplied credentials (API keys, bearer tokens).
    -- Per working-agreement §3 + ADR rotation runbook, encrypted with
    -- MCP_CREDENTIAL_ENCRYPTION_KEY (rotated quarterly).
    encrypted_credentials BYTEA,
    credentials_nonce   BYTEA,
    -- trust_level NEVER honoured by code paths today; ADR 0048 prep for future.
    trust_level         TEXT NOT NULL DEFAULT 'untrusted' CHECK (
        trust_level IN ('untrusted', 'trusted')
    ),
    -- User acknowledged the self-registered warning per §21.5.
    user_acknowledged_at TIMESTAMPTZ,
    disconnected_at     TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_mcp_servers_user_active
    ON mcp_servers(user_id)
    WHERE disconnected_at IS NULL;

-- Per spec §21.3 audit log + working-agreement §15.6 visibility.
CREATE TABLE mcp_call_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id       UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    tool_name       TEXT NOT NULL,
    -- Digests only — never the raw payload, working-agreement §3.4 bright line.
    params_digest   TEXT NOT NULL,        -- blake3(params_json)
    response_digest TEXT NOT NULL,        -- blake3(response_json)
    duration_ms     INTEGER NOT NULL,
    -- For the DLP filter rejections (§21.3) — the call was rejected before
    -- send because the payload matched a sensitive identifier pattern.
    outcome         TEXT NOT NULL CHECK (
        outcome IN ('ok', 'error', 'rejected_by_dlp', 'rejected_by_sandbox_gate', 'timeout')
    ),
    error_message   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_mcp_call_log_user_created ON mcp_call_log(user_id, created_at DESC);
CREATE INDEX idx_mcp_call_log_server_created ON mcp_call_log(server_id, created_at DESC);
-- Hot-path: rate-limit window queries (60 calls/min per user per spec §21.4)
CREATE INDEX idx_mcp_call_log_rate_window ON mcp_call_log(user_id, created_at DESC);
