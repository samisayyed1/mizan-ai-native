# Mizan Reviewers Registry

Named external + internal reviewers, scoped to the surface they're authorised over. This file is the single source of truth — PRs that touch a scoped surface must request the registered reviewer before merge (or, where authorised in writing, proceed under a sami-stamped exception).

## How registration works

Registration is two-step:

1. An entry below names the reviewer, their domain, and the trigger paths.
2. Where a GitHub handle is known, `.github/CODEOWNERS` auto-requests the reviewer on a matching PR. Until a handle is supplied, PR authors manually request the review by name in the PR description.

Removing or downgrading a registration requires sami's explicit instruction in a commit message + a dated entry in the Changes section at the bottom of this file.

---

## Active registrations

### Uncle Ferox — Track F (Zakat fiqh al-muamalat)

| Field | Value |
|---|---|
| Domain | Fiqh al-muamalat review of Track F (Zakat engine) rule ADRs + school-specific code |
| Authorised since | 2026-06-03 (autonomous-execution directive, this PR) |
| GitHub handle | _TBD — sami to supply; PR authors request review by name until then_ |
| Triggers | (a) Any ADR under `docs/adr/` named `*-school-rules*`, `*-zakat-*`, `*-fiqh-*`, `*-hawl-*`, `*-zakatability-*`; (b) Any code change under `mizan-4/crates/zakat/src/schools/*`, `mizan-4/crates/zakat/src/rules/*`, `mizan-4/crates/zakat/src/hawl_tracker.rs`; (c) Any change to the AAOIFI screening criteria ADR ([0012](adr/0012-aaoifi-screening-criteria.md)) |
| Authority | Approval of fiqh-substance for Maliki + Hanbali school rules per [docs/plans/06-track-f-zakat-engine-coverage.md](plans/06-track-f-zakat-engine-coverage.md) §F2 / §F4. Approves rule wording, asset-classification fiqh, hawl arithmetic. Does NOT approve unrelated code (Rust style, infra, etc.) — sami + ai handle those. |
| What happens at PR time | PR author tags @ferox (once handle supplied) or pings by name in the PR body. PR cannot self-merge under autonomous-execution authority for Track F fiqh substance — Uncle Ferox's approval is the gate. |
| What if unreachable | Track F slips. No fallback reviewer. Per CLAUDE.md §16.3 (no financial-advice without authorisation) + working-agreement §A2 (scholarly sign-off precedes code). |

### Sami Sayyed — workspace owner / catch-all approver

| Field | Value |
|---|---|
| Domain | Everything not registered to another reviewer |
| Authority | Self-merge under autonomous-execution allowed where explicitly authorised in writing; otherwise sami's nod precedes merge |

---

## Changes

| Date | Change | Authority |
|---|---|---|
| 2026-06-03 | Registered Uncle Ferox as approved fiqh al-muamalat reviewer for Track F | Autonomous-execution directive (this PR) |
