# ADR 0025 — OAuth connector framework

| Status | ✅ Accepted (autonomous-execution authority — Track J foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/10-track-j.md](../plans/10-track-j.md), [ADR 0013 — API deprecation default](0013-api-deprecation-default.md), [ADR 0020 — AI Tool Registry](0020-ai-tool-registry-expansion.md), Working-agreement §6 (sync providers), §11 (token lifecycle) |

## Context

Beyond the sync providers (Plaid, Setu, SnapTrade, etc.) covered by Track B's ADR 0021, the Mizan Evolution Spec §15 describes a **generic OAuth connector framework** for productivity / lifestyle integrations: Google Drive (statement upload), Notion (goal tracking), Slack (notification channel), and more.

This ADR locks the framework shape, the initial 3 providers, the background-refresh worker, the annual re-consent workflow, and the user-suggestion queue so Track J's 10 PRs ship mechanically.

## Decision

### Framework shape

The OAuth framework lives in `mizan-connect/src/oauth/`:

```
mizan-connect/src/oauth/
├── mod.rs             # public surface
├── registry.rs        # OauthProvider enum + per-provider config
├── refresh_worker.rs  # background-refresh worker
├── reconsent_worker.rs # annual re-consent + 14-day-pre-expiry notification
├── handlers.rs        # connect / callback / disconnect / list endpoints
└── providers/
    ├── google_drive.rs
    ├── notion.rs
    └── slack.rs
```

### Endpoint set

| Endpoint | Purpose |
|---|---|
| `POST /v1/oauth/connect/{provider}` | Kicks off the OAuth dance; returns the authorization URL |
| `GET /v1/oauth/callback/{provider}` | OAuth redirect target; exchanges code for tokens; persists encrypted refresh token |
| `POST /v1/oauth/disconnect/{provider}` | Revokes server-side + deletes local row |
| `GET /v1/oauth/connections` | Lists user's connected services |
| `POST /v1/oauth/suggest` | User-suggested provider queue |

### Data model (foundation migration already shipped — task tracker #35)

| Table | Purpose |
|---|---|
| `oauth_providers` | Provider registry (name, endpoints, scopes, handler_ref, compliance_status) |
| `user_oauth_connections` | Per-user connected services (user_id, provider, encrypted_token, scopes_granted, granted_at, last_reconsented_at, expires_at) |
| `oauth_suggestions` | User-suggested providers (user_id, suggested_service, status, reviewed_at) |

### Initial 3 providers (recommended per master plan)

| Provider | Scope | Use case |
|---|---|---|
| **Google Drive** | `drive.readonly` (limited to a Mizan-watched folder) | Statement upload — drop a PDF in the folder; agent ingests via `summarize_document` tool |
| **Notion** | `read` + `update` on a designated database only | Goal tracking — sync Mizan goals to a user's Notion database |
| **Slack** | `chat:write` + `users:read` | Today's Signal delivery as a Slack DM (Gold-tier polish) |

### Token lifecycle

- **At-rest encryption:** all refresh tokens encrypted with `AES-GCM-256` using a per-provider encryption key (matches the existing token-encryption pattern in `crates/connect/src/token_lifecycle.rs`)
- **Refresh worker:** runs hourly; pre-emptively refreshes tokens that expire within 24h
- **Annual re-consent:** at 14 days before the 12-month re-consent due date, fire an in-app notification "Mizan needs you to re-confirm Google Drive access". User accepts via the OAuth flow; otherwise the connection auto-disconnects at the 12-month mark.
- **Disconnect:** calls the provider's server-side revocation endpoint FIRST, then deletes the local row. If revocation fails, the local row is marked `disconnect_pending` for retry — never deleted ahead of provider revocation.

### Scope discipline

Default scopes are READ-ONLY per provider. Write scopes (e.g. Notion `update`) require explicit per-action user confirmation via the in-app modal — the OAuth flow grants the scope; the per-action gate enforces "you authorize, you action."

### User-suggested services

The `POST /v1/oauth/suggest` endpoint feeds into a manual review queue. Suggestions are not auto-activated; the Mizan team reviews quarterly + ships approved entries behind a per-provider feature flag.

### Entitlement gating

Per working-agreement §A11: OAuth connectors are a Silver+ capability. The capability is checked at the `/connect/{provider}` endpoint via the existing entitlement matrix.

## Rationale

**Why a generic framework (not per-provider one-off integrations)?**
Each provider integration would otherwise duplicate the refresh worker + re-consent flow + revocation handling. The framework amortizes that work once + lets providers ship as ~100-line modules.

**Why Google Drive / Notion / Slack as the initial 3?**
- **Google Drive** has the highest user value (statement upload covers the "I have a PDF brokerage statement" use case for many users)
- **Notion** is the conventional goal-tracking surface for the spec's reference users
- **Slack** is the most-requested notification channel for Gold-tier users per beta-feedback

**Why server-side revocation FIRST (not just delete local row)?**
- **Privacy contract** — the user expects the provider relationship to actually end. Deleting only the local row leaves a stale grant on the provider's side that could be reactivated if Mizan's keys leak.
- Working-agreement §3.1 (key-rotation discipline) generalizes to "tokens are revoked at the source, not just locally."

**Why annual re-consent (not 90-day or perpetual)?**
- 90-day is too frequent — users notification-fatigue and ignore the re-consent prompts
- Perpetual is too permissive — a once-connected service has perpetual scope until manually disconnected
- Annual aligns with most providers' default token lifetimes + matches the "review my permissions annually" pattern most security-conscious users follow

**Why a manual review queue for user-suggested providers?**
Compliance / security review per provider takes 1-2 days; queue-based review lets the team batch the work + lets users see "your request is being reviewed" status without a per-request response.

## Consequences

**Positive:**
- New providers ship as ~100-line modules against the framework
- Refresh worker + re-consent + revocation logic written once, reused by every provider
- Annual re-consent + 14-day pre-expiry notification surfaces upcoming disconnections to users → no surprise "why can't Mizan see my Google Drive?"

**Negative / accepted:**
- Per-provider OAuth quirks (some providers don't support refresh tokens, some require state parameter validation, etc.). Mitigation: per-provider module overrides where needed.
- User-suggestion queue creates ongoing review burden. Mitigation: queue review is monthly cadence + canned responses for common rejections.

**Risks:**
- Per-provider compliance review surface (e.g. Notion's enterprise customers' data-processing requirements). Mitigation: per-provider compliance ADR (one per provider) reviewed before activation.
- Token rotation failures could leave a user in a disconnected state without obvious recovery. Mitigation: refresh-worker failure fires an in-app `ReconnectionRequired` notification per Spec §9.3.

## Alternatives considered

- **One mega-OAuth handler with provider switches** — rejected; the per-provider quirks (Google's offline_access scope, Notion's workspace_id state, Slack's bot vs user token model) make a single handler quickly become unmaintainable.
- **No refresh worker — re-prompt on token expiry** — rejected; would interrupt user flow at the worst moment (mid-task).
- **Quarterly re-consent** — rejected per "Why annual" above.

## Implementation map

| PR | What lands |
|---|---|
| PR-J1 | `oauth_providers` registry + `user_oauth_connections` table consumers (foundation migration already shipped) |
| PR-J2 | Generic OAuth endpoints (`/connect/:provider` etc.) |
| PR-J3 | Google Drive provider — read-only Drive scope + Mizan-watched folder |
| PR-J4 | Notion provider — designated database for goals/notes |
| PR-J5 | Slack provider — Today's Signal delivery channel |
| PR-J6 | Background refresh worker |
| PR-J7 | Annual re-consent worker + 14-day pre-expiry notification |
| PR-J8 | Settings UI — Connections list + scope display + revoke buttons |
| PR-J9 | Suggest-a-service form + admin review surface |
| PR-J10+ | Additional providers (Apple Health / GitHub / Spotify / Calendar / Dropbox / OneDrive / iCloud / Zapier) — one PR per provider |

Each PR ≤ 500 lines per working-agreement §A21.
