# ADR 0001 — Adopt the v1.0 Working Agreement as `docs/working-agreement.md`

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** H (Code Hygiene & Audit Pass) — PR-H1

## Context

Two parallel documents claim to be "the Claude contract" for this codebase:

1. **`/CLAUDE.md`** at the repo root (158 lines, 6,255 bytes) — operational manual auto-loaded by Claude Code. Imports `MIZAN_AI_NATIVE_PLAN.md` as binding spec, references path-scoped sub-manuals (`mizan-4/CLAUDE.md`, `mizan-connect/CLAUDE.md`), wires the `MIZAN_ALLOW_PRODUCTION` hook, lists skills, sets tier model. **Load-bearing.**
2. **`/Users/samisayyed/Downloads/CLAUDE.md`** (500 lines, ~30,000 bytes) — a v1.0 "Working Agreement" dated April 2026. 19 sections: the six absolute rules, three surfaces, hard rules, code conventions, subsystem-specific rules (financial-truth, zakat, ai, insights, mizan-connect auth/billing/sync, Mizan Badge), testing standards, perf budgets, security boundaries, cache/versioning, DB discipline, AI agent dev rules, Mizan Badge product rules, past bugs as scars, workflow, monitoring dashboard, documentation requirements, anti-patterns, references, working agreement.

The two are **not duplicates** — they serve different purposes:

| Aspect | Root `CLAUDE.md` | Downloads v1.0 |
|---|---|---|
| Purpose | Operational session bootstrap | Coding & engineering contract |
| Length | Terse (158 lines) | Comprehensive (500 lines) |
| Audience | Claude Code agent on session start | Engineers + AI working on code |
| Scope | Skills, hooks, tier model, plan import | Code conventions, testing bar, security boundaries, past bugs |
| Update cadence | Rare (operational rules stable) | Annual (per §19 of v1.0) |

Both are correct. Both should be available. **Neither replaces the other.**

The repo also references `/mizan-4/CLAUDE.md` (the desktop path-scoped manual) which **does not exist**. The mizan-connect path-scoped manual at `/mizan-connect/CLAUDE.md` does exist.

## Decision

Adopt the **layered CLAUDE.md architecture**:

```
/CLAUDE.md                           (operational entry-point — load-bearing)
├── @MIZAN_AI_NATIVE_PLAN.md         (binding product spec — v3)
├── @docs/working-agreement.md       (binding coding contract — v1.0)        ← NEW
├── @mizan-4/CLAUDE.md               (desktop path-scoped manual)            ← TO CREATE
└── @mizan-connect/CLAUDE.md         (backend path-scoped manual — exists)
```

Concretely:

1. **Install the Downloads v1.0 file as `/docs/working-agreement.md`** (done in PR-H1).
2. **Update the root `/CLAUDE.md`** to add an import line for `@docs/working-agreement.md` alongside the existing `@MIZAN_AI_NATIVE_PLAN.md`.
3. **Create `/mizan-4/CLAUDE.md`** — desktop path-scoped operating manual modeled after `/mizan-connect/CLAUDE.md`, cross-referencing the working agreement (out of scope of this ADR; tracked as Track H follow-up).
4. **Preserve the existing operational rules** in root CLAUDE.md unchanged (skills, hooks, tier model, AI safety contract summary, release sequencing lock).

The Downloads v1.0 file is the source of truth for:
- The six absolute rules (Section 0 of working-agreement.md)
- Code conventions (Rust, TS, SQL, naming, comments)
- Testing standards (coverage floors, mutation testing, E2E)
- Performance budgets
- Security boundaries
- Cache/versioning policy
- DB discipline
- AI agent development rules
- Mizan Badge product surface rules
- Anti-patterns
- The working agreement itself

The root `/CLAUDE.md` remains the source of truth for:
- Session bootstrap, imports, hooks references
- Skills index
- Tier model summary
- Operational quick-commands
- Credential-request protocol
- Where things live

## Rationale

Overwriting the existing root CLAUDE.md with the v1.0 working agreement would:

- Lose the `@MIZAN_AI_NATIVE_PLAN.md` import (the binding product spec)
- Lose the `.claude/settings.json` hook references (the `MIZAN_ALLOW_PRODUCTION` gate)
- Lose the skills index
- Lose the path-scoped sub-manual cross-references
- Break Claude Code session bootstrap on every fresh open

The layered approach preserves operational continuity while adopting the comprehensive coding contract. Both documents load together via Claude Code's `@`-import mechanism — the agent sees both at session start.

## Consequences

**Positive:**
- The 95% coverage floors, mutation testing rules, the 6 absolute rules, the perf budgets, the past-bug scars, the anti-patterns — all become binding via `@docs/working-agreement.md` import.
- Updates to coding contract land in one canonical file with clear annual review cadence.
- Path-scoped sub-manuals can cross-reference the working agreement without duplicating it.
- ADR + plan workflow (`docs/adr/`, `docs/runbooks/`, `docs/plans/`, `docs/qa-passes/`, `docs/audit/`) — referenced throughout the working agreement — now has a real home.

**Negative:**
- Two-document contract is mildly more cognitive load than one (mitigated by clear separation: operational vs coding).
- Root CLAUDE.md and `docs/working-agreement.md` overlap on a few topics (tier model, AI safety contract). Future drift is a real risk. Mitigation: annual review of root CLAUDE.md to ensure it remains the terse summary and never expands to duplicate `docs/working-agreement.md`.

**Follow-ups (tracked):**
- PR-H1.b: update root `/CLAUDE.md` to import `@docs/working-agreement.md`
- PR-H1.c: create `/mizan-4/CLAUDE.md` as desktop path-scoped manual
- Annual review of `docs/working-agreement.md` per its own Section 19

## Alternatives Considered

**Alternative A: Overwrite the root CLAUDE.md with Downloads v1.0.** Rejected — would break operational continuity (see Rationale).

**Alternative B: Inline the v1.0 contents into the root CLAUDE.md as new sections.** Rejected — produces a ~700-line root CLAUDE.md that violates its own §17 anti-pattern ("comments explain why not what"; same principle applies to operating manuals — terse > comprehensive at the root).

**Alternative C: Discard the v1.0 file and treat the existing root CLAUDE.md as sufficient.** Rejected — the v1.0 file encodes the 18 QA Pass scars, the past-bug list, the perf budgets, the mutation testing requirements that are nowhere captured in the existing root manual.

## References

- `/CLAUDE.md` (existing operational entry-point)
- `/docs/working-agreement.md` (new — coding contract)
- `/MIZAN_AI_NATIVE_PLAN.md` (binding product spec)
- `/mizan-connect/CLAUDE.md` (backend path-scoped manual, exists)
- Anthropic Claude Code documentation on `@`-import syntax in CLAUDE.md files
