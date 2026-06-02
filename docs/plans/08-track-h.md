# Track H — Code Hygiene & Audit Pass (blocking gate)

**Status:** In progress (PR-H1, PR-H1.b, PR-H1.c, PR-H4 runbooks, PR-H6 CI hygiene done; PR-H2, PR-H3, PR-H7, PR-H8, PR-H9 pending).
**Estimated sprints:** 2.
**Gates:** Public-release valve for Tracks A–G, I–K. No track ships externally until PR-H9 audit baseline is signed.
**Source:** `docs/plans/00-master-plan.md` → "Track H — Code Hygiene & Audit Pass".

## Scope

**In:** crate extraction (5 candidate crates), 11-section audit per working-agreement §18.12, `docs/adr/` + `docs/runbooks/` + `docs/plans/` + `docs/qa-passes/` directory bootstrap, repo-path rename (`mizan-4/` → `mizan-desktop/` if approved), CLAUDE.md adoption.

**Out:** new features (those are Tracks A–G, I–K).

## PRs

| # | Status | Title | Scope |
|---|---|---|---|
| H1 | ✅ Done | Adopt CLAUDE.md v1.0 as docs/working-agreement.md | ADR 0001, working-agreement.md installed |
| H1.b | ✅ Done | Root CLAUDE.md imports working-agreement.md | Edit `/CLAUDE.md` to add `@docs/working-agreement.md` |
| H1.c | ✅ Done | Create mizan-4/CLAUDE.md desktop path-scoped manual | New file mirroring mizan-connect/CLAUDE.md |
| H2 | ⏸️ Deferred | Repo rename `mizan-4/` → `mizan-desktop/` | Requires user sign-off (CP-0 Q6). Mechanical but invasive — touches CI, scripts, every doc reference. |
| H3.a | ⏸️ Future | Extract `crates/financial-truth` from `mizan-core` | ADR 0002 + workspace member + consumer migration |
| H3.b | ⏸️ Future | Extract `crates/zakat` | ADR 0003 + same shape as H3.a |
| H3.c | ⏸️ Future | Extract `crates/insights` | ADR 0004 |
| H3.d | ⏸️ Future | Extract `crates/synthesis` | ADR 0005 |
| H3.e | ⏸️ Future | Extract `crates/csv-import` | ADR 0049 (was originally numbered 0006, but 0006 got reassigned to CI hygiene scans) |
| H4 | ✅ Done | docs/runbooks/ bootstrap | deploy.md, updater-key-rotation.md, incident-response.md, gdpr-export.md, key-rotation-quarterly.md, rollback-drill.md, supabase-lifecycle.md |
| H5 | ⏸️ Pending | docs/qa-passes/ bootstrap with QA-P19 template | The next QA pass once Track C wires AI-initiated truth-ledger writes |
| H6 | ✅ Done | CI: gitleaks + cargo-machete + knip + ts-prune | ADR 0006. `continue-on-error: true` until PR-H9 |
| H7 | ⏸️ Pending | CI: nightly cargo mutants | Pointed at financial-truth, zakat, ai/dispatcher, insights, synthesis (post-H3 extractions). 80% mutation score floor (95% on financial crates per working agreement §5) |
| H8 | ⏸️ Deferred | CI: clippy::disallowed_methods for FX silent-fallbacks | Requires specific FX-conversion method names — needs the dedicated FX module that lands in H3.a (financial-truth extraction). Re-open once H3.a is done. |
| H8.b | ✅ Done | CI: f64-in-money-paths lint | `scripts/lint-no-f64-in-money-paths.sh` grep-based CI check rejecting `f64` in money paths (working agreement §5 + §13 QA Pass 4). Wired into CI hygiene job (informational). First run surfaced 22 findings in `crates/core/src/health/` — recorded in audit report §3.5 for classification. |
| H9 | ✅ Done (scaffold) | Audit report scaffold | `docs/audit/2026-Q3-baseline-audit-report.md` — 11-section structural template + 22 f64 findings logged in §3.5. Full audit execution + sign-off pending. |
| H10..N | ⏸️ Pending | Per-finding resolution PRs | One PR per blocker, batched for majors. |
| Hfinal | ⏸️ Pending | Signed audit report merged | Track H closes. Public-release valve opens for Tracks A–G, I–K. |

## Definition of Done (Track H)

- All ADRs from the planned list (0001-0007, 0049) merged
- 5 crates extracted (financial-truth, zakat, insights, synthesis, csv-import) with consumers migrated
- 7 runbooks live
- 5+ CI hygiene scans green (clippy, audit, machete, gitleaks, knip, ts-prune, fmt)
- Audit baseline signed (zero blockers, zero majors, minors with owners + deadlines)
- Coverage thresholds met on extracted crates (95% on financial-truth, zakat, ai/dispatcher, billing, auth, webhooks)
- CI hygiene job promoted from `continue-on-error: true` to hard-fail

## Open Questions

- Repo rename (CP-0 Q6) — yes/no/when?
- For the crate extraction PRs, do we extract modules verbatim (preserve internal structure) or restructure during extraction (per the spec's organisation)? **Recommend:** extract verbatim first, restructure in a follow-up PR within the same track.
- For PR-H9 audit, what's the policy on majors? Working agreement implies zero majors at sign-off — confirm this is the bar.

## What's done this session (2026-06-02)

- PR-H1, PR-H1.b, PR-H1.c, PR-H4 (7 runbooks), PR-H6 (CI hygiene)
- PR-H7 (nightly cargo mutants workflow)
- PR-H8.b (f64-in-money-paths lint — caught 22 findings in `crates/core/src/health/`)
- PR-H9 (audit report scaffold + Section 3.5 pre-populated with f64 findings)
- ADR 0001 (working agreement adoption)
- ADR 0006 (CI hygiene scans)
- ADR 0008 (cache policy — Track I)
- ADR 0009 (updater snapshot & rollback — Track I)
- ADR 0010 (IPC schema versioning — Track I)
- ADR 0011 (holdings metadata — pre-Track E)
- Updater dead-code cleanup + cfg-import fix
- mizan-connect test-module clippy alignment (7 test modules)
- mizan-4 frontend lint cleanup (3 Array<T>, 1 err)
- Net Worth page "Breakdown" → "Composition" rename
