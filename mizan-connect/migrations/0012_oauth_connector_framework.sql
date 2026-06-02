-- ============================================================================
-- Track J PR-J1 — OAuth Connector Framework
-- ----------------------------------------------------------------------------
-- Per spec §20 (OAuth Connector Framework) and `docs/plans/10-track-j.md`:
--
-- Beyond the dedicated Plaid / SnapTrade / Setu / Tink / Basiq / SGFinDex /
-- Lean / CCXT integrations, Mizan provides a generic OAuth 2.0 / OIDC
-- framework that lets users authorize Mizan to connect to *any* service
-- supporting OAuth. Silver+ entitlement.
--
-- 3 tables:
--   oauth_providers       — registry of pre-vetted services
--   user_oauth_connections — per-user active connections + encrypted tokens
--   oauth_suggestions     — queue of user-suggested services awaiting review
-- ============================================================================

CREATE TABLE oauth_providers (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                  TEXT NOT NULL UNIQUE,
    display_name          TEXT NOT NULL,
    -- OAuth endpoints
    authorize_url         TEXT NOT NULL,
    token_url             TEXT NOT NULL,
    refresh_url           TEXT NOT NULL,
    revoke_url            TEXT,
    -- Required OAuth scopes for read-only baseline. Comma-separated.
    required_scopes       TEXT NOT NULL,
    -- Optional write scopes (must be re-consented per-action by user).
    optional_write_scopes TEXT,
    -- Per spec §20.5: data residency, ToS compatibility, subprocessor disclosure
    compliance_status     TEXT NOT NULL DEFAULT 'pending_review' CHECK (
        compliance_status IN ('approved', 'pending_review', 'rejected', 'deprecated')
    ),
    -- Free-form notes for the team's review queue.
    review_notes          TEXT,
    -- The post-connect handler reference (e.g., the Rust function module path
    -- that processes a successful OAuth callback for this provider).
    post_connect_handler  TEXT NOT NULL,
    -- Per-provider encryption key reference (Fly secret name). Rotated quarterly
    -- per docs/runbooks/key-rotation-quarterly.md.
    encryption_key_env    TEXT NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_oauth_providers_compliance ON oauth_providers(compliance_status);

CREATE TABLE user_oauth_connections (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id               UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id           UUID NOT NULL REFERENCES oauth_providers(id) ON DELETE RESTRICT,
    -- AES-GCM-256 encrypted via SecretCipher::from_bytes(&{PROVIDER}_TOKEN_ENCRYPTION_KEY).
    -- Working agreement §3 hard rule 4: never plaintext on disk.
    encrypted_access_token  BYTEA NOT NULL,
    encrypted_refresh_token BYTEA,
    -- Encryption nonces (kept alongside ciphertext per AES-GCM spec).
    access_token_nonce      BYTEA NOT NULL,
    refresh_token_nonce     BYTEA,
    -- Granted scopes (subset of provider.required_scopes ∪ optional_write_scopes).
    scopes_granted          TEXT NOT NULL,
    -- Token expiry — the refresh worker checks this every hour.
    expires_at              TIMESTAMPTZ,
    -- Annual re-consent enforcement (working agreement §20.4): expired_at + 12mo
    -- triggers auto-disconnect unless user re-consents.
    last_reconsented_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    reconsent_due_at        TIMESTAMPTZ NOT NULL,
    -- Disconnect lifecycle.
    disconnected_at         TIMESTAMPTZ,
    disconnect_reason       TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, provider_id)
);

CREATE INDEX idx_user_oauth_connections_user
    ON user_oauth_connections(user_id)
    WHERE disconnected_at IS NULL;

CREATE INDEX idx_user_oauth_connections_refresh
    ON user_oauth_connections(expires_at)
    WHERE disconnected_at IS NULL AND expires_at IS NOT NULL;

CREATE INDEX idx_user_oauth_connections_reconsent
    ON user_oauth_connections(reconsent_due_at)
    WHERE disconnected_at IS NULL;

CREATE TABLE oauth_suggestions (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    suggested_by_user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    service_name          TEXT NOT NULL,
    rationale             TEXT,
    status                TEXT NOT NULL DEFAULT 'queued' CHECK (
        status IN ('queued', 'reviewing', 'approved', 'rejected')
    ),
    reviewed_by           UUID REFERENCES users(id),
    reviewed_at           TIMESTAMPTZ,
    review_notes          TEXT,
    -- Once approved + shipped, this links to the resulting oauth_providers row.
    resulting_provider_id UUID REFERENCES oauth_providers(id) ON DELETE SET NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_oauth_suggestions_status_queued
    ON oauth_suggestions(created_at)
    WHERE status = 'queued';
