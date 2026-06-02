# ADR 0013 — Mizan Connect API versioning + deprecation policy

| Status | Accepted |
|---|---|
| Date | 2026-06-03 |
| Deciders | sami (autonomous-execution directive); ai (auditor; documents the decision) |
| Replaces | n/a |
| Supersedes | n/a |
| Related | [0001-adopt-working-agreement-v1](0001-adopt-working-agreement-v1.md), [0010-ipc-schema-versioning](0010-ipc-schema-versioning.md) (sibling — IPC = desktop side; this ADR = HTTP API side) |

## Context

The Mizan evolution spec adds many new HTTP endpoints under Mizan Connect across Tracks B (asset class sync providers), D (news), G (advisor multi-client), J (OAuth framework), K (MCP gateway). The original master-plan had "API deprecation policy" as one of the four required user-gates before Track H could close.

Per the autonomous-execution directive of 2026-06-03, the API-deprecation gate is **removed** in favour of a default policy: **v2 with a 6-month deprecation window**. This ADR documents that default.

## Decision

**Versioning scheme:** path-prefix `/v1/`, `/v2/`, ... per endpoint family. Bumped only on breaking changes; additive changes keep the same version.

**Definition of a breaking change** (anything below bumps the version):
- Removed field in a response shape
- Renamed field
- Type-changed field (e.g. `string` → `integer`, nullable → non-nullable, optional → required)
- Removed endpoint
- Stricter request validation (request that previously succeeded now 400s)
- Changed default behaviour of a query parameter
- Changed pagination semantics (cursor format change, page size limits)
- Changed error response shape

**Not a breaking change** (do NOT bump version for):
- Added optional response field
- Added endpoint
- Added optional request field
- Looser request validation
- New error variant in an existing error enum (clients must handle unknown errors)

**Deprecation window:** **6 months** from the public announcement of a vN successor. During that window:
- Both vN and vN+1 endpoints respond
- vN responses carry a `Deprecation:` header (per RFC 8594) with the sunset date in `Sunset:` (RFC 8594)
- vN responses also carry `Link: </v2/...>; rel="successor-version"`
- The desktop's release notes name the affected endpoints + the sunset date
- The admin monitoring dashboard (working-agreement §15) shows per-day request counts split by version — the team can see clients that have not migrated and reach out

**Sunset behaviour:** at sunset, vN endpoints return `410 Gone` with a body pointing to vN+1. Logs record sunset-410 calls separately so we can quantify breakage.

**Telemetry:** every request emits `X-Mizan-API-Version` (request) + `X-Mizan-API-Served-Version` (response). The dashboard's API-version panel reads from these.

## Consequences

**Positive:**
- Predictable client lifecycle. Six months is enough for the desktop's auto-update + the user base's mobile bandwidth realities.
- No silent breakage. Headers + dashboard + release-notes are the three surfaces clients learn about a deprecation from.
- Tracks B/D/G/J/K can independently version their endpoint families without coordinating.

**Negative / accepted:**
- Server code carries two versions for 6 months per breaking change. Acceptable cost.
- Migration discipline required: every breaking change ADR must include the deprecation calendar entry (`docs/api-versioning.md` — to be created when the first deprecation lands).

**Risks:**
- Forgetting to set the `Deprecation` / `Sunset` headers. Mitigation: an axum middleware adds them centrally based on a route-registry annotation.
- The desktop refusing to auto-update during the deprecation window leaves the user on a stale client. Mitigation: the Tauri auto-updater is mandatory above a min-version sentinel; updater snapshot + rollback (ADR 0009) ensures the update is safe.

## Alternatives considered

- **No versioning, just add fields.** Rejected: doesn't survive type changes or removed endpoints.
- **Header-based versioning (`Accept: application/vnd.mizan.v2+json`).** Rejected: harder to test from a browser; path-prefix is the convention for public Rust+axum APIs.
- **3-month window.** Rejected: too short for users on monthly desktop release cadence + mobile-bandwidth constraints in markets like India + UAE.
- **12-month window.** Rejected: doubles server-side carry cost without proportional benefit.

## Implementation

- **PR-API-V1:** create `docs/api-versioning.md` calendar (when the first deprecation lands)
- **PR-API-V2:** add the axum middleware that injects `Deprecation` / `Sunset` / `Link` headers from a route-registry annotation
- **PR-API-V3:** wire the dashboard's API-version panel to read `X-Mizan-API-Version` from request logs
- Future deprecations: each one is a small PR that adds the version-bumped endpoint, marks the old endpoint deprecated in the route registry, and updates the calendar
