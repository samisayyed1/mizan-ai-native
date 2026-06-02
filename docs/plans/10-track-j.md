# Track J — OAuth Connector Framework

**Status:** Pending. Depends on Track C (memory + tool registry).
**Estimated sprints:** 2.5.
**Source:** `docs/plans/00-master-plan.md` → "Track J — OAuth Connector Framework".

## Scope

**In:** `oauth_providers` registry + endpoint set in Mizan Connect, initial 3 providers (Google Drive, Notion, Slack — recommended), background refresh worker, user-suggested service queue, annual re-consent worker, Silver+ entitlement gating.

**Out:** sync providers (Plaid / SnapTrade / Setu — Track B); MCP (Track K).

## PRs

| # | Status | Title |
|---|---|---|
| J1 | ✅ Done | `oauth_providers` registry + `user_oauth_connections` + `oauth_suggestions` tables | `mizan-connect/migrations/0012_oauth_connector_framework.sql` — 3 tables, AES-GCM-256 encrypted tokens with nonces, annual re-consent column, suggestion review workflow |
| J2 | ⏸️ Pending | Generic OAuth endpoints (`POST /v1/oauth/connect/:provider`, `GET /v1/oauth/callback/:provider`, `POST /v1/oauth/disconnect/:provider`, `GET /v1/oauth/connections`) |
| J3 | ⏸️ Pending | Google Drive provider — read-only Drive scope + Mizan-watched folder for statement ingestion |
| J4 | ⏸️ Pending | Notion provider — designated database for goals/notes |
| J5 | ⏸️ Pending | Slack provider — Today's Signal delivery channel option |
| J6 | ⏸️ Pending | Background refresh worker (hourly token refresh) |
| J7 | ⏸️ Pending | Annual re-consent worker + 14-day pre-expiry notification (extends Track C insights rule set) |
| J8 | ⏸️ Pending | Settings UI — Connections list + scope display + revoke buttons |
| J9 | ⏸️ Pending | Suggest-a-service form + admin review surface |
| J10..N | ⏸️ Pending | Additional providers: Apple Health, GitHub, Spotify, Calendar (Google/Outlook/Calendly), Dropbox/OneDrive/iCloud, Zapier |

## ADRs (planned)

- 0042 — OAuth connector framework
- 0043 — Initial OAuth provider selection (Google Drive, Notion, Slack)

## Security checklist

- [ ] Tokens encrypted at rest with provider-specific encryption key (AES-GCM-256)
- [ ] Disconnect calls provider revocation endpoint server-side first
- [ ] Read-only scopes by default; write requires explicit re-consent
- [ ] Annual re-consent enforced; expired = auto-disconnect
- [ ] Privacy notice surfaces at connect with scope list
- [ ] User-suggested services queued for compliance review before activation

## Definition of Done

- User in Settings → Connections sees 3+ connected services; can revoke any; can suggest new ones
- Google Drive statement-ingestion path proven end-to-end (statement drops in watched folder → agent ingests → activities written)
- Annual re-consent fires + user re-grants successfully
