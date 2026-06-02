# Track G — Enterprise + Advisor Tiers

**Status:** Pending. Depends on Track C (memory + tool registry for advisor context).
**Estimated sprints:** 3.
**Source:** `docs/plans/00-master-plan.md` → "Track G — Enterprise + Advisor".

## Scope

**In:** SSO / SAML / OIDC for Enterprise auth, multi-seat team membership extension, Advisor → Client linking model, `'advisor-reviewed'` badge surface (badge from Track E + write path here), per-client report generation, note-taking surface, separate billing model.

**Out:** the entitlement gating logic at the use sites (threaded throughout other tracks); the existing solo team / member infrastructure (already in place).

## PRs

| # | Status | Title |
|---|---|---|
| G1 | ⏸️ Pending | Multi-seat team extension + Enterprise billing entitlement |
| G2 | ⏸️ Pending | SAML auth path |
| G3 | ⏸️ Pending | OIDC auth path |
| G4 | ⏸️ Pending | SSO group-to-role mapping |
| G5.a | ✅ Done | Advisor-Client link model migration | `mizan-connect/migrations/0014_advisor_links.sql` — `advisor_links` + `advisor_sign_offs` + `advisor_access_log` with grant-token-hash, scope enum, time-limited expires_at, revoke trail. Drives the `'advisor-reviewed'` Mizan Badge modifier (Track E §8.2). |
| G5.b | ⏸️ Pending | Handlers + scope enforcement at endpoint level |
| G6 | ⏸️ Pending | Advisor clients list UI |
| G7 | ⏸️ Pending | Advisor client detail view |
| G8 | ⏸️ Pending | `'advisor-reviewed'` badge write path (consumes Track E modifier slot) |
| G9 | ⏸️ Pending | Notes panel attached to clients + individual holdings |
| G10 | ⏸️ Pending | Per-client report generation (existing report generator + advisor branding) |
| G11 | ⏸️ Pending | Audit log entries for advisor accesses |

## ADRs (planned)

- 0039 — SSO SAML/OIDC Enterprise
- 0040 — Advisor-Client linking model
- 0041 — Enterprise multi-seat billing

## Security checklist

- [ ] SSO tokens verified against IdP signing keys
- [ ] Advisor scope enforced at every endpoint (not just UI)
- [ ] Time-limited tokens for client access; revocation works immediately
- [ ] Audit log entry per advisor view / write of a client's data
- [ ] Client must explicitly grant + revoke; never granted by advisor unilaterally

## Definition of Done

- Enterprise tier: design-partner family office uses SSO, multiple seats, custom rate limits, audit log export
- Advisor tier: an advisor sees N clients who granted access, signs off on individual holdings, exports per-client reports with their name
