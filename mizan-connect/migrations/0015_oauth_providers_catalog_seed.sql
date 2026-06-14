-- ============================================================================
-- Track J PR-J1.b — Seed the oauth_providers registry from the in-code catalog
-- ----------------------------------------------------------------------------
-- Migration 0012 created the empty `oauth_providers` table. Until it has rows
-- a `user_oauth_connections.provider_id` FK lookup fails — every connect flow
-- depends on the row existing.
--
-- This migration seeds the 8 providers from `src/oauth/catalog.rs` (the source
-- of truth at compile time). Per ADR 0025, adding a 9th provider requires:
--   1. Add it to catalog.rs
--   2. Add a follow-up migration that INSERTs the new row (forward-only
--      discipline per CLAUDE.md §0 rule 4)
-- so the catalog and the DB never drift.
--
-- Idempotent via ON CONFLICT (name) DO NOTHING — safe to replay during the
-- annual reset drills (docs/runbooks/rollback-drill.md).
-- ============================================================================

INSERT INTO oauth_providers (
    name, display_name, authorize_url, token_url, refresh_url, revoke_url,
    required_scopes, optional_write_scopes, compliance_status,
    post_connect_handler, encryption_key_env
) VALUES
    (
        'google-drive', 'Google Drive',
        'https://accounts.google.com/o/oauth2/v2/auth',
        'https://oauth2.googleapis.com/token',
        'https://oauth2.googleapis.com/token',
        'https://oauth2.googleapis.com/revoke',
        'https://www.googleapis.com/auth/drive.readonly', NULL,
        'approved',
        'google_drive::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'notion', 'Notion',
        'https://api.notion.com/v1/oauth/authorize',
        'https://api.notion.com/v1/oauth/token',
        'https://api.notion.com/v1/oauth/token',
        NULL,
        'read_content,update_content', 'update_content',
        'approved',
        'notion::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'slack', 'Slack',
        'https://slack.com/oauth/v2/authorize',
        'https://slack.com/api/oauth.v2.access',
        'https://slack.com/api/oauth.v2.access',
        'https://slack.com/api/auth.revoke',
        'chat:write', NULL,
        'approved',
        'slack::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'github', 'GitHub',
        'https://github.com/login/oauth/authorize',
        'https://github.com/login/oauth/access_token',
        'https://github.com/login/oauth/access_token',
        NULL,
        'repo:read', NULL,
        'approved',
        'github::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'google-calendar', 'Google Calendar',
        'https://accounts.google.com/o/oauth2/v2/auth',
        'https://oauth2.googleapis.com/token',
        'https://oauth2.googleapis.com/token',
        'https://oauth2.googleapis.com/revoke',
        'https://www.googleapis.com/auth/calendar.events.readonly', NULL,
        'approved',
        'google_calendar::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'outlook-calendar', 'Outlook Calendar',
        'https://login.microsoftonline.com/common/oauth2/v2.0/authorize',
        'https://login.microsoftonline.com/common/oauth2/v2.0/token',
        'https://login.microsoftonline.com/common/oauth2/v2.0/token',
        NULL,
        'Calendars.Read', NULL,
        'approved',
        'outlook_calendar::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'apple-calendar', 'Apple Calendar',
        'eventkit://local',
        'eventkit://local',
        'eventkit://local',
        NULL,
        'eventkit-local', NULL,
        'approved',
        'apple_calendar::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    ),
    (
        'zapier', 'Zapier',
        'https://zapier.com/app/connections',
        'https://hooks.zapier.com/hooks/catch/',
        'https://hooks.zapier.com/hooks/catch/',
        NULL,
        'webhook', NULL,
        'approved',
        'zapier::on_connect',
        'OAUTH_TOKEN_ENCRYPTION_KEY'
    )
ON CONFLICT (name) DO NOTHING;
