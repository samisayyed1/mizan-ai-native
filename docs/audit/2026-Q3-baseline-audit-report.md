# 2026-Q3 Baseline Audit Report

**Status:** 🟡 In progress (Track H PR-H9 — the blocking gate)
**Audit period:** 2026-06-02 → 2026-Q3 close
**Auditor:** _to be assigned at PR-H9 execution_
**Reviewers:** _to be assigned_

## Purpose

The first signed audit per `docs/working-agreement.md` §18.12. Establishes the production-grade baseline that subsequent quarterly audits compare against. Track H closes once this report ships with **zero blocker findings** and **zero major findings** (minors with owners + deadlines are acceptable).

Until this report ships, the CI `hygiene` job runs with `continue-on-error: true` (informational mode). Once it ships, that flag is removed — regressions cannot land.

## Findings classification

| Class | Definition | SLA |
|---|---|---|
| **Blocker** | Production cannot ship with this present | Fixed before Track H closes |
| **Major** | Significant security/correctness/performance issue | Fixed before Track H closes |
| **Minor** | Worth fixing but not release-blocking | Tracked issue with owner + deadline |
| **Informational** | Visibility for future hygiene | Logged; no immediate action |

## Section 1 — Full Dependency Tree Audit

**Goal:** Every transitive dependency reviewed for license compatibility, maintenance status, security advisories.

**Tooling:**
- `cargo tree --workspace --duplicate` — surface duplicate versions
- `cargo audit` — RustSec advisories (already CI-gated, see §1.5)
- `cargo deny check` — license + advisory + bans policy
- `cargo machete` — unused dependencies
- `pnpm list --depth 0` + `pnpm audit --production`

**Findings:** _to be populated_

## Section 2 — Secret Scan

**Goal:** Zero findings from secret scanners across full git history.

**Tooling:**
- `gitleaks` — full history scan (already CI-job, informational)
- `trufflehog` — second-opinion scan with detector ensemble
- Manual review of `.env*` patterns + Tauri config files

**Findings:** _to be populated_

## Section 3 — Dead Code Scan

**Goal:** Every linter from working agreement §18.1 runs to zero output.

**Tooling:**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` (zero warnings — already enforced)
- `cargo udeps` (nightly) — unused dependencies in any `Cargo.toml`
- `cargo machete` (stable) — second-opinion unused dep scanner
- `knip` — unused TS exports
- `ts-prune` — second-opinion unused TS exports
- `cargo tarpaulin` — branch coverage; uncovered branches reported

**Findings:** _to be populated_

### Section 3.5 — No-f64-in-money-paths (informational, 2026-06-02 pre-audit scan)

**Tooling:** `./scripts/lint-no-f64-in-money-paths.sh` (Track H PR-H8.b)

**Initial findings (informational, NOT yet classified blocker/major/minor):**

22 instances of `f64` in `crates/core/src/health/`:

| File | Line | Context |
|---|---|---|
| `health/checks/quote_sync.rs` | 160 | `error_mv: f64` market-value summation |
| `health/checks/fx_integrity.rs` | 22, 265 | `affected_mv: f64`, `mv_pct: f64` |
| `health/checks/classification.rs` | 27, 190, 329 | `market_value: f64`, summation, hash input |
| `health/checks/price_staleness.rs` | 28, 302 | `market_value: f64`, hash input |
| `health/service.rs` | 98, 239, 253, 269, 314, 512 | total_portfolio_value, fx_pair_mv map, parse::<f64>, holding_mv_map |
| `health/traits.rs` | 34, 42, 56, 241 | total_portfolio_value trait method arg |
| `health/model.rs` | 387, 429, 468, 639, 642 | affected_mv_pct, mv_escalation_threshold, classification_warn_threshold |

**Classification (TBD — to be set by auditor):**
- The `health/` module is a diagnostic surface, NOT a financial-mutation path. f64 here arguably falls under "approximate health diagnostics" tolerance rather than the QA Pass 4 P&L precision requirement.
- HOWEVER, the working agreement §0 + §13 rule is structural: "no f64 in money paths." The health module computes against market values that flow from the same `holdings` data the P&L paths use — drift in health-check thresholds could mask a real precision bug elsewhere.
- **Recommendation pending auditor review:** classify as **Major** with a deadline to migrate to `rust_decimal::Decimal` by end of Track H, OR re-scope the lint's `MONEY_PATHS` to exclude `health/` if the auditor judges it diagnostic-only. Document the decision in this section.

## Section 4 — Dead File Scan

**Goal:** No orphaned files anywhere in the repo.

**Tooling:**
- `git ls-files | xargs grep -L "{filename basename}"` — find files referenced nowhere
- No `.bak`, `.old`, `.tmp`, `.draft`, `.copy` extensions committed
- No commented-out code blocks in committed source

**Findings:** _to be populated_

## Section 5 — Schema Audit

**Goal:** Every table, every column, every index, every foreign key reviewed.

**Activities:**
- Cross-reference `crates/storage-sqlite/src/schema.rs` against the production DB
- Identify orphaned columns from deprecated features
- Verify every WHERE-clause-frequent column has a backing index
- Verify cascade behaviour on `ON DELETE` constraints is correct
- Verify partial unique indexes (e.g. `idx_subscriptions_team_active`) match the intended uniqueness scope

**Findings:** _to be populated_

## Section 6 — Query Plan Review

**Goal:** Every hot-path query's `EXPLAIN` reviewed.

**Activities:**
- Enable `pg_stat_statements` on mizan-connect Postgres (if not already)
- Pull top 50 queries by mean execution time
- Run `EXPLAIN (ANALYZE, BUFFERS)` on each
- Flag any sequential scan on a table > 10k rows
- Cross-reference with `docs/runbooks/supabase-lifecycle.md` Activity 1

**Findings:** _to be populated_

## Section 7 — Index Coverage Review

**Goal:** Every WHERE clause in production paths is index-backed.

**Activities:**
- Static analysis: grep WHERE clauses across `src/**/*.rs` repository implementations
- Dynamic analysis: `pg_stat_user_indexes` + missing-index tracking
- Cross-reference against `docs/runbooks/supabase-lifecycle.md` Activity 2

**Findings:** _to be populated_

## Section 8 — Cache Table Audit

**Goal:** Every cache has explicit TTL, eviction worker, invalidation policy.

**Activities:**
- Walk `crates/storage-sqlite/src/cache_policy.rs::CACHE_POLICIES` (single source of truth — ADR 0008)
- Cross-reference against the schema via `./scripts/lint-cache-policy.sh` (already CI-gated)
- Verify the cache_eviction worker (`cache_eviction.rs`) handles every `EvictionStrategy` variant correctly
- Verify migration `-- caches-evicted: ...` manifest comments per Track I PR-I2.d

**Status at audit start:** Cache policy registry shipped + lint enforces registration. PR-I2.a..e (per-strategy SQL + Tauri startup wiring) remaining.

**Findings:** _to be populated_

## Section 9 — API Surface Audit

**Goal:** Every Mizan Connect endpoint reviewed for auth coverage, rate limiting, request validation, response shape, error handling.

**Activities:**
- Enumerate routes via `mizan-connect/src/server.rs::build_app` walk
- For each endpoint:
  - [ ] JWT-verified through `auth::middleware`
  - [ ] Rate-limited (sliding-window bucket)
  - [ ] Request body validated via `Json<T: Validate>`
  - [ ] Response shape uses explicit DTO (never serialises domain model directly)
  - [ ] Error responses include `request_id`
  - [ ] OpenAPI entry present
- Verify webhook endpoints follow the 5-case rotation pattern (Stripe model)
- Verify admin endpoints use constant-time bearer compare

**Findings:** _to be populated_

## Section 10 — Tauri Command Audit

**Goal:** Every IPC command reviewed for input validation, error propagation, versioning.

**Activities:**
- Enumerate commands via `apps/tauri/src/lib.rs::generate_handler!` registration
- For each command:
  - [ ] Input types live in `crates/ipc-schema` (per ADR 0010)
  - [ ] Errors propagated via structured `MizanError` (§A24)
  - [ ] No `unwrap()` / `expect()` in the handler body
  - [ ] No `println!` / `eprintln!` (use `tracing`)
  - [ ] No `f64` in money paths
  - [ ] Version-aware dispatch if multiple versions supported

**Status at audit start:** ipc-schema crate skeleton shipped (PR-I3). Migration of existing commands iterative in PR-I3.c..N.

**Findings:** _to be populated_

## Section 11 — AI Tool Audit

**Goal:** Every tool in the dispatcher reviewed for AI Safety Runtime compliance.

**Activities:**
- Walk `crates/ai/src/dispatcher.rs::register_tool` calls
- For each tool:
  - [ ] Per-turn cap weight declared
  - [ ] Audit log scope declared
  - [ ] Numeric bounds declared (if applicable)
  - [ ] Truth Ledger emission flag set correctly (true for financial mutations)
  - [ ] Memory writer NOT bypassed (only `crates/ai/src/memory/writer.rs::write_fact` writes to `user_memory`)
- Verify the "no financial advice" guardrail prompts are present
- Verify Decimal arithmetic in every money path (no `f64`)

**Findings:** _to be populated_

## Summary

_to be populated at audit close_

| Section | Blockers | Majors | Minors | Info |
|---|---|---|---|---|
| 1 — Dependency tree | _TBD_ | | | |
| 2 — Secret scan | _TBD_ | | | |
| 3 — Dead code | _TBD_ | | | |
| 4 — Dead files | _TBD_ | | | |
| 5 — Schema | _TBD_ | | | |
| 6 — Query plan | _TBD_ | | | |
| 7 — Index coverage | _TBD_ | | | |
| 8 — Cache table | _TBD_ | | | |
| 9 — API surface | _TBD_ | | | |
| 10 — Tauri commands | _TBD_ | | | |
| 11 — AI tools | _TBD_ | | | |
| **TOTAL** | | | | |

## Sign-off

- [ ] Auditor: _name + date_
- [ ] Reviewer 1: _name + date_
- [ ] Reviewer 2: _name + date_

Once signed:
- CI `hygiene` job promoted from `continue-on-error: true` to hard-fail
- Track H closes
- Public-release valve opens for Tracks A–G, I–K
- Quarterly re-audit calendar entry added (Q4 2026, Q1 2027, ...)
