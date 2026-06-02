# ADR 0024 — Enterprise + Advisor tier

| Status | ✅ Accepted (autonomous-execution authority — Track G foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/07-track-g.md](../plans/07-track-g.md), Working-agreement §16.2 (AML/KYC) + §A11 (entitlements), [ADR 0023 — Mizan Badge expansion](0023-badge-expansion.md) (the `'advisor-reviewed'` modifier is populated by this surface) |

## Context

Mizan today ships Free / Silver / Gold consumer tiers. The Mizan Evolution Spec §14 prescribes two additional surfaces:

1. **Enterprise tier** — multi-seat orgs (family offices, small wealth-management firms), SSO via SAML/OIDC, custom rate limits, audit-log export
2. **Advisor tier** — independent financial advisors linking to N client users with scoped read-write access, sign-off workflow, per-client reports

This ADR locks the data model, SSO design, scope enforcement, and billing model so Track G's 11 PRs (per `docs/plans/07-track-g.md`) land mechanically.

## Decision

### Enterprise tier

**Auth:**
- **SAML 2.0** via the `samael` crate (mature Rust SAML impl) for SSO providers (Okta, OneLogin, Azure AD)
- **OIDC** via the existing `jsonwebtoken` infrastructure with provider-specific JWKS endpoints (Google Workspace, Microsoft Entra)
- Existing `auth/supabase_jwt.rs` extends to support both SAML + OIDC tokens with org-group claims

**Data model (cloud-side migrations already shipped — see task tracker #35):**

| Table | Purpose |
|---|---|
| `team_memberships_extended` | Extends existing `team_memberships` with `sso_group_to_role` mapping |
| `org_audit_log_export` | Per-org audit-log export tracking (compliance use) |

**Custom rate limits:** per-org overrides in `mizan-connect/src/auth/rate_limit_overrides.rs`, configured by Mizan admin via the existing admin dashboard.

**Billing:** per-seat pricing via Stripe (existing infrastructure) + custom invoice line items for compliance add-ons.

### Advisor tier

**Data model (foundation already shipped — `advisor_links` migration per task tracker #35):**

| Table | Purpose |
|---|---|
| `advisor_links` | (advisor_user_id, client_user_id, scope enum, time_limited_token, granted_at, revoked_at) |
| `holding_signoff` | Per-holding advisor sign-off records (populates the `'advisor-reviewed'` Mizan Badge modifier per ADR 0023) |
| `advisor_notes` | Per-client + per-holding notes (encrypted at rest per cloud-side AES-GCM-256) |

**Scope enforcement:** the `advisor_links.scope` enum gates advisor access:

| Scope | Read | Write | Notes |
|---|---|---|---|
| `read_only` | ✅ all client holdings | ❌ | Default — advisor sees but can't change anything |
| `notes_only` | ✅ all client holdings | ✅ `advisor_notes` only | Most common — sign off + leave notes |
| `read_write_full` | ✅ all | ✅ holdings + activities | Rare; requires explicit client opt-in per holding |

**Time-limited tokens:** every advisor link has a `granted_until` timestamp. Expiry triggers automatic revocation; renewal requires the client to re-grant via the in-app accept flow.

**Per-client reports:** PDF generation reuses the existing `generate_report` AI tool (per ADR 0020 entry 11) with the advisor's name embedded in the report header + footer. Report retention 7 years per AML/KYC working-agreement §16.2.

### `'advisor-reviewed'` badge population

The advisor sign-off flow (PR-G8) writes a `holding_signoff` row when an advisor reviews a holding. The badge layer (per ADR 0023) reads `holding_signoff.exists_for(holding_id, advisor_id)` at the row's render time and surfaces the modifier accordingly.

### Audit-log entry per advisor action

Every advisor read/write/sign-off action emits an entry to `agent_audit_log` (the existing audit trail) so:
- Clients can see "Your advisor accessed your holdings on 2026-06-15 at 14:23" via the privacy-disclosure surface
- Mizan admin can verify scope-enforcement integrity post-incident

## Rationale

**Why both SAML and OIDC?**
SAML is the de-facto standard for enterprise SSO (Okta, OneLogin, Azure AD via SAML); OIDC is the modern Google Workspace + Microsoft Entra path. Supporting both lets Enterprise customers pick their provider without us building bespoke connectors per provider.

**Why scoped advisor access (not "advisor can see everything")?**
- **AML/KYC compliance (working-agreement §16.2)** — advisor over-reach exposes Mizan to fiduciary liability. Scoped access + audit trail keeps each access traceable.
- **Client trust** — most users won't grant an advisor `read_write_full`. The notes-only + read-only tiers cover 95% of real advisor-client relationships.

**Why time-limited tokens (not perpetual links)?**
Conventional advisor-client engagements have natural review cycles (quarterly, annually). Forcing renewal aligns the access lifecycle with the engagement lifecycle. Per working-agreement §11.4 — token rotation cadence.

**Why per-seat Enterprise pricing (not per-AUM)?**
Per-seat is auditable (count of active SSO logins per month); per-AUM requires us to verify AUM claims which is out of scope. Enterprise contract can layer custom invoice items for high-AUM customers if needed.

## Consequences

**Positive:**
- Enterprise SSO unlocks family-office + small-firm market segment
- Advisor tier creates a new revenue channel (per-advisor billing) + adds the `'advisor-reviewed'` badge for high-touch clients
- Scope enforcement at the data layer (not just UI) means even a compromised advisor token can't exceed granted scope

**Negative / accepted:**
- SAML 2.0 has historical security CVEs (signature wrapping, XSW attacks). Mitigation: `samael` crate is actively maintained + per-IdP testing in CI per PR-G2.
- Per-seat billing requires a "seat is active" definition. Choosing "any SSO login in the last 30d" — documented in PR-G1.

**Risks:**
- Advisor sign-off creates a soft fiduciary expectation (advisor signed off → "they vouch for this"). Mitigation: explicit disclaimer in the UI + working-agreement §16 "no financial advice" still applies (the badge surfaces review status, not advisor recommendation).
- Multi-seat invitation flow has historical phishing attack surface. Mitigation: invitations require email confirmation + the recipient must match a verified domain per Enterprise's SSO domain claim.

## Alternatives considered

- **OIDC only (drop SAML)** — rejected; many existing enterprise customers run Okta as SAML-only.
- **No Advisor tier (only Enterprise)** — rejected; the spec's Sharia-aware user persona explicitly cites "I want my Islamic-finance advisor to review my Zakat strategy" as a primary use case.
- **Per-AUM Enterprise pricing** — rejected per "Why per-seat" above.

## Implementation map

| PR | What lands |
|---|---|
| PR-G1 | Multi-seat team extension + Enterprise billing entitlement |
| PR-G2 | SAML auth path (`samael` integration) |
| PR-G3 | OIDC auth path (extends existing JWT verifier) |
| PR-G4 | SSO group-to-role mapping (`team_memberships_extended` consumer) |
| PR-G5 | Advisor-Client link model + scopes + time-limited tokens |
| PR-G6 | Advisor clients list UI |
| PR-G7 | Advisor client detail view |
| PR-G8 | `'advisor-reviewed'` badge write path (populates badge per ADR 0023) |
| PR-G9 | Notes panel (per-client + per-holding) |
| PR-G10 | Per-client report generation (existing report generator + advisor branding) |
| PR-G11 | Audit log entries for advisor accesses |

Each PR ≤ 500 lines per working-agreement §A21.
