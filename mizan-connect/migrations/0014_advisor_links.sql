-- ============================================================================
-- Track G PR-G5 — Advisor → Client linking model
-- ----------------------------------------------------------------------------
-- Per spec §13.5 (Advisor tier) and `docs/plans/07-track-g.md`:
--
-- > Multi-client view: an Advisor sees the balance sheets of clients who
-- > have granted access. 'advisor-reviewed' badge surface — Advisor can
-- > sign off on individual holdings. Per-client report generation with
-- > the Advisor's name on every output.
--
-- The link must be EXPLICITLY granted by the client (not unilaterally by
-- the advisor), with scope (read / read-write) and a time-limited token.
-- ============================================================================

CREATE TABLE advisor_links (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    advisor_user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Scope drives what the advisor can do.
    scope               TEXT NOT NULL CHECK (
        scope IN ('read_only', 'read_write')
    ),
    -- Client-issued grant token (opaque, time-limited, revocable).
    -- Hashed; raw token only shown to client on creation, never again.
    grant_token_hash    TEXT NOT NULL UNIQUE,
    -- Granted-at + expiry. Default 1 year; client can extend or revoke.
    granted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL,
    revoked_at          TIMESTAMPTZ,
    revoked_by_user_id  UUID REFERENCES users(id),
    revoke_reason       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (advisor_user_id, client_user_id)
);

CREATE INDEX idx_advisor_links_active
    ON advisor_links(advisor_user_id)
    WHERE revoked_at IS NULL AND expires_at > now();

CREATE INDEX idx_advisor_links_client
    ON advisor_links(client_user_id)
    WHERE revoked_at IS NULL;

CREATE INDEX idx_advisor_links_expiry
    ON advisor_links(expires_at)
    WHERE revoked_at IS NULL;

-- Advisor sign-off audit on individual holdings — drives the
-- 'advisor-reviewed' Mizan Badge modifier (Track E §8.2).
CREATE TABLE advisor_sign_offs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    advisor_user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Holding identity matches the desktop's holdings_metadata composite key.
    holding_account_id  TEXT NOT NULL,
    holding_symbol      TEXT NOT NULL,
    holding_as_of_date  DATE NOT NULL,
    -- The sign-off note (advisor's commentary on this holding).
    note                TEXT,
    signed_off_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Sign-offs can be retracted if the advisor's view changes.
    retracted_at        TIMESTAMPTZ,
    retract_reason      TEXT
);

CREATE INDEX idx_advisor_sign_offs_holding
    ON advisor_sign_offs(client_user_id, holding_account_id, holding_symbol)
    WHERE retracted_at IS NULL;

CREATE INDEX idx_advisor_sign_offs_advisor
    ON advisor_sign_offs(advisor_user_id, signed_off_at DESC);

-- Per spec + working-agreement §15.5 audit: every advisor access logged.
CREATE TABLE advisor_access_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    advisor_user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action              TEXT NOT NULL CHECK (
        action IN ('view_dashboard', 'view_holding', 'sign_off', 'retract_sign_off', 'add_note', 'generate_report')
    ),
    -- Action-specific context as JSON (e.g., the holding_id for view_holding).
    context_json        TEXT,
    ip_address          INET,
    user_agent          TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_advisor_access_log_advisor_created
    ON advisor_access_log(advisor_user_id, created_at DESC);

CREATE INDEX idx_advisor_access_log_client_created
    ON advisor_access_log(client_user_id, created_at DESC);
