# Audit Reports

Signed pre-production audit reports per `docs/working-agreement.md` §18.12. The first audit is the Track H gate that blocks the public-release valve.

## Format

Each audit report has 11 sections per the working agreement:

1. **Dependency tree** — every transitive dependency reviewed
2. **Secret scan** — `gitleaks` + `trufflehog` over full git history
3. **Dead code scan** — clippy / udeps / machete / knip / ts-prune
4. **Dead file scan** — orphaned files
5. **Schema audit** — every table, column, index, FK reviewed
6. **Query plan review** — every hot-path `EXPLAIN` reviewed
7. **Index coverage review** — every production WHERE clause index-backed
8. **Cache table audit** — every cache has explicit TTL + eviction policy
9. **API surface audit** — every endpoint reviewed for auth/rate-limit/validation
10. **Tauri command audit** — every IPC command reviewed
11. **AI tool audit** — every tool reviewed for AI Safety Runtime compliance

Each finding classified: **Blocker / Major / Minor / Informational**. Output: signed report with classification counts.

## Cadence

- **First audit (the baseline)** — Track H gate, before any other public-track ships
- **Quarterly re-runs** — drift below baseline is a release blocker

## Active reports

| Date | Type | Sign-off | Blockers | Majors | Minors | Info |
|---|---|---|---|---|---|---|
| 2026-Q3 | [Baseline (Track H PR-H9)](2026-Q3-baseline-audit-report.md) | 🟡 In progress | TBD | TBD | TBD | TBD |

## Pending

- 2026-Q4 quarterly re-audit (post-baseline)
