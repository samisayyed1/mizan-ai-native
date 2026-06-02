# ADR 0006 — CI Hygiene Scans (Informational)

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** H (Code Hygiene & Audit Pass) — PR-H6

## Context

The working agreement §18.1 / §18.2 / §18.4 requires the codebase to be free of:

- Dead code (clippy / TS unused exports / unused functions)
- Dead files (orphaned source files referenced nowhere)
- Dead dependencies (unused Rust crates, unused npm packages)
- Secrets (zero findings from secret scanners across full git history)

Today, **clippy is enforced**, but the other three categories are policy-only.
There's no CI scanner running `cargo machete` / `knip` / `ts-prune` / `gitleaks`.

The working agreement §18.12 baseline audit explicitly calls for these scans.
Track H PR-H9 (the signed audit) cannot complete without their output.

## Decision

Add a new CI job `hygiene` to `.github/workflows/ci.yml` running:

- **gitleaks** — secret scan over full git history (`fetch-depth: 0`)
- **cargo-machete** — unused Rust dependency scan on both `mizan-4/` and `mizan-connect/`
- **knip** — TypeScript dead-export scan on the desktop frontend
- **ts-prune** — second-opinion TypeScript dead-export scan on `mizan-4/apps/frontend`

The job runs **with `continue-on-error: true`** initially — findings appear in the
PR's checks panel but **do not block merge**. This is intentional: we have
no signed audit baseline yet, and pre-existing findings would block every
PR until they're triaged.

PR-H9 (the signed audit pass) removes `continue-on-error` after findings are
classified blocker / major / minor / informational and all blockers + majors
are resolved.

## Rationale

**Why informational mode first:**

Per the working agreement §14 ("Don't add error handling, fallbacks, or
validation for scenarios that can't happen") and §17 ("Don't relax a security
rule 'temporarily.' Temporary becomes permanent"), the right pattern is to
turn a gate ON when we're prepared to keep it on. We're not yet — the
baseline hygiene audit hasn't classified existing findings. Promoting to
hard-fail would block every PR with findings that are pre-existing technical
debt, not regressions introduced by that PR.

The informational mode surfaces findings now so the team can see what the
audit will catch, while keeping the merge train moving. It's the well-known
"baseline then enforce" pattern.

**Why gitleaks specifically (not trufflehog):**

Gitleaks has a cleaner GitHub Action, faster scan times, and detects most
common secret patterns. The working agreement §18.12 mentions both —
trufflehog can be added in a follow-up if gitleaks misses something specific.
For now, gitleaks alone is the proportionate baseline.

**Why cargo-machete (not cargo-udeps):**

Both detect unused Rust dependencies. cargo-machete runs on stable Rust;
cargo-udeps requires nightly. Pinning to stable is the working agreement
posture. cargo-machete is also markedly faster.

**Why both knip and ts-prune:**

Per working agreement §18.4, "second-opinion scanners" are intentional —
each catches what the other misses. knip is project-aware (config-driven);
ts-prune is per-file (mechanical). Together they cover both modes.

## Consequences

**Positive:**

- Every PR gets a hygiene check; findings surface in the PR view
- Team gets immediate visibility into pre-existing technical debt
- Track H PR-H9 (signed audit) has automated input for its findings classification
- Once promoted to hard-fail (PR-H9-followup), regressions cannot land

**Negative:**

- The first time these run on a fresh PR, findings may be loud and overwhelming
  (mitigation: informational mode, no merge block)
- The scans add ~2 minutes to CI wall time (mitigation: `continue-on-error`
  means they run in parallel with required jobs, not in series)

**Follow-ups (tracked):**

- PR-H6.b: knip configuration file (`knip.json`) tuned to mizan-4's structure
- PR-H6.c: ts-prune configuration in `.tsprunerc` if defaults are too noisy
- PR-H9 (Track H final): remove `continue-on-error` after baseline audit signs off
- PR-H7 (separate ADR): nightly `cargo mutants` job

## Alternatives Considered

**Alternative A: Make the job hard-fail from day 1.** Rejected — would block every PR until baseline findings are addressed. The team velocity hit isn't worth it; informational mode achieves the visibility goal without the cost.

**Alternative B: Only add gitleaks; defer dead-code scans to PR-H9.** Rejected — the dead-code scanners are easy to add now and give the audit pass real input. Splitting them across two PRs serves no purpose.

**Alternative C: Run hygiene scans only nightly, not per-PR.** Rejected — nightly cadence means a regression sits visible for 24h before anyone sees it, defeating the "catch it in the PR" loop the working agreement values.

## References

- `.github/workflows/ci.yml` — the `hygiene` job
- `docs/working-agreement.md` §18.1, §18.2, §18.4, §18.12
- `docs/plans/00-master-plan.md` Track H PR-H6
- ADR 0001 (working agreement adoption)
