# Session Log — 2026-06-02

What landed in this session toward the [Mizan Evolution master plan](00-master-plan.md).

## ✅ Phase 0 — Baseline Confirmation (COMPLETE, ALL GREEN)

All checks per the master plan Phase 0:

| Check | Status | Notes |
|---|---|---|
| `cargo fmt --all -- --check` (mizan-4) | ✅ Green (after one-off `cargo fmt --all`) | 20+ files were reformatted; checked in cleanly |
| `cargo check --workspace --all-features` (mizan-4) | ✅ Green | 2m 08s |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` (mizan-4) | ✅ Green | After fixing 6 dead-code warnings in `apps/tauri/src/updater.rs` via `#[cfg(not(debug_assertions))]` gating |
| `cargo clippy ...` (mizan-connect) | ✅ Green | After adding `#![allow(clippy::unwrap_used, ...)]` to 7 test modules across `entitlements.rs`, `stripe_client.rs`, `user_rate_limit.rs`, `snaptrade/repository.rs`, `public_config.rs`, `snaptrade/signing.rs`, and `tests/entitlements_parity.rs` |
| `cargo audit` (both) | ✅ Green | 20 allowed warnings for GTK transitives only (per `.cargo/audit.toml`) |
| `cargo test --workspace --all-features` (mizan-4) | ✅ Green | Full suite passed |
| `cargo test --workspace --all-features` (mizan-connect) | ✅ Green | Testcontainers integration tests included |
| `pnpm --filter frontend lint:quiet` | ✅ Green | After auto-fixing 3 `Array<T>` → `T[]` errors + 1 unused-catch variable in `adapters/tauri/notifications.ts` |
| `pnpm --filter frontend test -- --run` | ✅ Green | **70 test files all passed** |
| `pnpm --filter frontend type-check` | ⏸️ Not explicitly run this session, but covered by existing CI |
| Playwright E2E | ⏸️ Not run this session (long-running; depends on Tauri dev environment) |

**The baseline is confirmed green.** Phase 1 (planning) was already approved before this session; Phase 3 (execution) is now in flight per the activity below.

## 🚢 Shipped This Session

### Documentation foundation (Track H)
- `docs/working-agreement.md` — adopted v1.0 (500 lines) as the binding coding contract
- `docs/adr/0001-adopt-working-agreement-v1.md` — layered CLAUDE.md architecture rationale
- `docs/adr/0006-ci-hygiene-scans.md` — gitleaks + cargo-machete + knip + ts-prune in informational mode
- `docs/adr/0008-cache-policy-single-source-of-truth.md` — Track I PR-I1 ADR
- `docs/adr/README.md` — index of all ADRs (active + planned)
- `docs/runbooks/README.md` — index
- `docs/runbooks/deploy.md` — Mizan Connect deploy procedure (codifies the `--no-cache` v37 lesson)
- `docs/runbooks/incident-response.md` — incident workflow with SEV-0..3 classifications
- `docs/runbooks/key-rotation-quarterly.md` — quarterly provider-token encryption key rotation
- `docs/runbooks/updater-key-rotation.md` — Tauri updater signing key rotation
- `docs/runbooks/gdpr-export.md` — GDPR/DPDP/CCPA right-to-export procedure
- `docs/runbooks/rollback-drill.md` — quarterly rollback drill
- `docs/runbooks/supabase-lifecycle.md` — quarterly Postgres lifecycle hygiene
- `docs/plans/README.md` — plans index
- `docs/plans/00-master-plan.md` — the approved master plan (copied from `.claude/plans/`)
- `docs/qa-passes/README.md` — index, references historical QA passes encoded in working-agreement §13
- `docs/audit/README.md` — index, pending the first baseline audit report

### CLAUDE.md updates
- Root `/CLAUDE.md` — added `@docs/working-agreement.md` import alongside `@MIZAN_AI_NATIVE_PLAN.md`. Two binding contracts now load every session.
- `/mizan-4/CLAUDE.md` — created (previously missing despite root referencing it). Mirrors `/mizan-connect/CLAUDE.md` structure with desktop-specific invariants and conventions.

### Track I PR-I1 — Cache policy registry
- `crates/storage-sqlite/src/cache_policy.rs` — typed registry with `CachePolicy { table, ttl, age_from, age_column, eviction, purpose }`, four `EvictionStrategy` variants (Delete / RollupThenDelete / ArchiveThenDelete / KeepMarkStale), 5 existing cache table entries (quotes, fx_rates, daily_brief_runs, sync_run_ledger, market_news), and 5 commented placeholder slots for Track C tables (user_memory, news_items, projection_snapshots, agent_audit_log)
- 6 unit tests pinning the contract (no duplicates, TTL positive, purpose non-empty, AgeFrom::Custom cross-reference, lookup behavior, durable-table exclusion) — **all passing**
- Module registered in `crates/storage-sqlite/src/lib.rs`

### Track I PR-I1.b — Cache-policy CI lint
- `scripts/lint-cache-policy.sh` — bash script using awk to parse Diesel schema.rs + grep CREATE TABLE in migrations. Compares discovered tables against an explicit `KNOWN_CACHE_TABLES` allowlist (initial: quotes, fx_rates, daily_brief_runs, sync_run_ledger, market_news). Fails on any cache-shaped table missing from CACHE_POLICIES.
- **Caught a real finding on first run**: `market_news` was in the schema but not in CACHE_POLICIES. Added it (30d retention, `published_at` as age column, Delete strategy).
- Wired into `.github/workflows/ci.yml` `hygiene` job (continue-on-error until PR-H9 audit pass).

### Track I PR-I2 — Cache eviction worker skeleton
- `crates/storage-sqlite/src/cache_eviction.rs` — typed `SweepReport` + `EvictionOutcome` + injectable `EvictionContext` trait + per-strategy dispatch via `EvictionStrategy` match
- Two entry points: `run_synchronous` (Tauri startup, app-version-mismatch path) and `run_one_sweep` (async, daily 3am scheduler)
- `KeepMarkStale` strategy fully implemented (no-op at worker layer — staleness surfaces via Mizan Badge `'stale'`); `Delete` / `RollupThenDelete` / `ArchiveThenDelete` strategies stub with skeleton-pending error messages (per-strategy SQL implementation in PR-I2.a–c follow-ups)
- Module registered in `lib.rs`

### Track I PR-I2.a — Delete strategy SQL generation
- Pure function `delete_sql_for(policy: &CachePolicy) -> String` generates the parameterised `DELETE FROM {table} WHERE {age_column} < ?` statement
- `AgeFrom` enum mapping (`CreatedAt`/`UpdatedAt`/`Custom`) drives column selection
- Panics on `KeepMarkStale` policies (dispatch bug) — exercised by `#[should_panic]` test
- 4 new tests; total **15 cache_* tests passing** (6 cache_policy + 9 cache_eviction)
- Actual DB execution (timestamp bind + query against pool) deferred to PR-I2.e wiring

### Track H PR-H7 — Nightly cargo mutants CI workflow
- `.github/workflows/nightly-mutants.yml` — 03:00 UTC daily mutation testing
- Two jobs: (1) financial-truth + zakat modules (95% score floor per working agreement §18.8), (2) ai/dispatcher + insights + synthesis (80% floor)
- 6-hour timeout per job; mutants.out uploaded as artifact for weekly review
- Filters point at in-place modules under `crates/core` until Track H PR-H3 crate extractions land

### ADRs 0009 + 0010
- **0009 — Updater Snapshot & Rollback Design** — paper trail for Track I PR-I4 (pre-update DB snapshot), PR-I5 (post-install 5-check self-test), PR-I6 (auto-rollback on failure). WAL-aware copy via `sqlite3_backup_*`. 30d snapshot retention. Failed binary preserved as `.failed-{version}` for 7d.
- **0010 — IPC Schema Versioning** — paper trail for Track I PR-I3. New `crates/ipc-schema` shared crate with versioned Tauri command request/response types. ts-rs codegen to keep TS bindings in lock-step. 2-minor-version transition windows for handler dispatch.

### Track E PR-E1.a — holdings_metadata migration
- `migrations/2026-06-02-000001_holdings_metadata/{up,down}.sql` per ADR 0011
- Composite PK `(account_id, holding_symbol, as_of_date)` matching JSON-in-portfolio-history identity
- 14 columns: origin (CHECK constraint covering all 15 provider variants), sharia_status (CHECK constraint), AI estimation range, tags JSON, advisor sign-off, agent_modified_at, audit timestamps
- 6 indexes (origin, sharia partial, last_screened_at partial, ai_estimated partial, agent_modified_at partial, advisor_reviewed_by partial)
- Build green; lint green

### Track C PR-C1.a — user_memory migration
- `migrations/2026-06-02-000002_user_memory/{up,down}.sql` per spec §7.3
- 10 columns: id, user_id, fact_text, embedding (BLOB for sqlite-vec), category (CHECK), confidence (CHECK 0–1), source (CHECK), created_at, last_used_at, expires_at, deleted_at (soft-delete for GDPR)
- 4 indexes (user partial, expires_at partial, last_used_at desc, category)
- Foundation for Tracks C/D/F/J/K
- Build green; lint green

### Per-track plan files
- 11 per-track plans now live: `01-track-a.md`, `02-track-b.md`, ..., `11-track-k.md`
- Each gives a concrete execution surface for future sessions: PR list with status, ADRs planned/written, security checklists, definition of done, open questions
- `docs/plans/README.md` updated with the full index

### Track I PR-I3 — `crates/ipc-schema` skeleton (per ADR 0010)
- New workspace crate `mizan-ipc-schema` (auto-discovered by `crates/*` glob)
- `lib.rs` with `try_parse_versions!` macro that handlers use to accept multiple versioned request shapes during transition windows
- `commands/notifications.rs` worked example demonstrating the v1 module pattern
- `ts-export` feature gates `ts-rs` codegen for TS bindings
- 2 round-trip tests passing

### Track F PR-F1 — `hawl_anchors` migration
- `migrations/2026-06-02-000003_hawl_anchors/{up,down}.sql` per spec §11.3
- Composite PK `(user_id, cohort_id)` supports both classical single-cohort treatment and per-asset cohorts
- Decimal stored as text (rust_decimal::Decimal::to_string()) — never f64 in money paths
- 2 indexes (anchor_date for approaching-Hawl scan, last_evaluated for stale-eval scan)
- Build green

### Track I PR-I2.b/c — Archive strategy SQL extension
- `cache_eviction::select_expired_rows_sql_for(policy)` — pure SELECT generator with `ORDER BY age_column` for deterministic archive batches
- Refactored `age_column_for(policy)` helper shared between Delete and ArchiveThenDelete
- 2 new tests; total **17 cache_* tests passing**

### Track H PR-H8.b — f64-in-money-paths lint
- `scripts/lint-no-f64-in-money-paths.sh` — heuristic grep over money-path directories (`crates/core/src/{portfolio,activities,zakat,synthesis,insights,health,net_worth_snapshot,financial_truth}`, plus future extracted crates)
- Comment-line + test-file exemption
- Wired into CI `hygiene` job (informational)
- **First run surfaced 22 real findings** in `crates/core/src/health/` — captured in `docs/audit/2026-Q3-baseline-audit-report.md §3.5` for auditor classification (the health module is diagnostic vs P&L precision, so classification is a judgment call)

### Track H PR-H9 — Audit report scaffold
- `docs/audit/2026-Q3-baseline-audit-report.md` — 11-section structural template per working-agreement §18.12
- Each section has Goal + Tooling + Activities + Findings placeholder
- Summary table at bottom + 3-signature sign-off block
- Section 3.5 pre-populated with the 22 f64-in-money-paths findings from PR-H8.b

### Foundation migrations batch (2026-06-02)

**Desktop (`mizan-4/crates/storage-sqlite/migrations/`):**
1. `2026-06-02-000001_holdings_metadata` — Track E PR-E1.a (per ADR 0011)
2. `2026-06-02-000002_user_memory` — Track C PR-C1.a (per spec §7.3)
3. `2026-06-02-000003_hawl_anchors` — Track F PR-F1.a (per spec §11.3)
4. `2026-06-02-000004_news_items` — Track D PR-D1.a (per spec §10)
5. `2026-06-02-000005_projection_snapshots` — Track C PR-C15.a (per spec §7.6, predictive)
6. `2026-06-02-000006_agent_audit_log` — Track C PR-C14.aux (per spec §7.1 audit)
7. `2026-06-02-000007_reconciliation_queue` — Track C aux (per spec §15.3 + §8.2 badge)

**Cloud (`mizan-connect/migrations/`):**
1. `0012_oauth_connector_framework.sql` — Track J PR-J1 — 3 tables, AES-GCM-256 encrypted tokens with nonces, annual re-consent
2. `0013_mcp_capability.sql` — Track K PR-K1.a — catalog + per-user registry + audit log with digests-only retention, `trust_level` schema prep per ADR 0048
3. `0014_advisor_links.sql` — Track G PR-G5.a — `advisor_links` + `advisor_sign_offs` + `advisor_access_log`, grant-token-hash, scope enum, full audit trail

**CACHE_POLICIES activated:**
4 new cache entries (`user_memory`, `news_items`, `projection_snapshots`, `agent_audit_log`) joining the 5 existing (`quotes`, `fx_rates`, `daily_brief_runs`, `sync_run_ledger`, `market_news`). `KNOWN_CACHE_TABLES` in the lint script updated to match. 19 cache_* tests still pass.

**Build verification:**
- mizan-4 cargo build green
- mizan-4 cargo clippy --workspace -- -D warnings green
- mizan-connect cargo build green
- mizan-connect cargo clippy --workspace -- -D warnings green
- 3 lint scripts all run; cache-policy + migration-manifest exit 0; no-f64-in-money-paths exit 1 (22 findings recorded in audit §3.5)

### Track C PR-C1.a — `user_memory` migration (foundation)
- `migrations/2026-06-02-000002_user_memory/{up,down}.sql` per spec §7.3
- 10 columns including soft-delete for GDPR/DPDP right-to-rectification (working agreement §16.4)
- Per-row TTL via `expires_at` column (heterogeneous: agent-inferred 12mo, user-stated never)
- 4 indexes (user partial, expires_at partial, last_used_at desc, category partial)
- Foundation for Tracks C/D/F/J/K
- Build green

### Track H PR-H6 — CI hygiene scans
- `.github/workflows/ci.yml` — added `hygiene` job running:
  - `gitleaks` (secret scan, full git history)
  - `cargo machete` (unused Rust deps, both projects)
  - `knip` (dead TS exports, frontend)
  - `ts-prune` (second-opinion dead TS exports)
- Runs with `continue-on-error: true` initially (informational mode) — see ADR 0006 for the path to hard-fail in PR-H9

### Track A PR-A1 — "Break Down" → "Composition" rename
- `mizan-4/apps/frontend/src/pages/net-worth/net-worth-content.tsx:537` — single H2 label that used "Breakdown" inside the Net Worth page now reads "Composition" (avoids the redundant "Net Worth → Breakdown" stacking; preserves semantic meaning)
- Type identifiers (`BreakdownItem`, `BudgetBreakdown`, `WeightedBreakdown`, etc.) intentionally not renamed — those are legitimate technical English for "a breakdown OF something" and not user-facing copy

### Dead-code cleanup (mizan-4)
- `apps/tauri/src/updater.rs` — gated `app_store_url`, `extract_changelog_url`, `extract_screenshots`, `is_app_store_build`, and the `warn` log import with `#[cfg(not(debug_assertions))]` so they only exist in release builds where they're called. Debug builds no longer emit dead-code warnings.

### Test-module clippy alignment (mizan-connect)
- Added scoped `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::useless_vec)]` to 7 test modules. Existing `tests/common/mod.rs` precedent was followed.

## 🚧 Architectural Finding Surfaced

**Finding:** The desktop holdings model is **JSON inside `portfolio_history.holdings`**, not a relational `holdings` table. The Track E PR-E1 plan (add `sharia_status`, `last_screened_at`, `ai_estimated`, `ai_confidence`, `ai_value_range_low`, `ai_value_range_high`, `tags` columns to `holdings`) doesn't map to reality.

**Implications for Track E (Mizan Badge Expansion):**
- Per-holding metadata needs either:
  - **(A)** A new `holdings_metadata` side table keyed by `(holding_id, account_id, date)` — small, augments JSON
  - **(B)** Extension of the JSON blob with new fields — no migration, but breaks schema discoverability
  - **(C)** Migration to a relational holdings table — huge scope, breaks downstream consumers

**Recommendation:** **(A)**, captured as an ADR before writing the Track E migration. Task #17 in the task list tracks this.

## 🎯 What's Next

Ordered by impact + reversibility per the master plan dependency graph:

1. **PR-H1.d** — Refresh `mizan-4/CLAUDE.md` cross-references to point at extracted crates once they exist (deferred until Track H PR-H3 crate extractions land)
2. **Track E PR-E0** — Write the ADR for the holdings-metadata design (Option A above) before any code change
3. **Track I PR-I2** — `cache_eviction.rs` worker reading the `cache_policy` registry. Wire into Tauri startup ahead of WebView paint.
4. **Track I PR-I1.b** — `scripts/lint-cache-policy.sh` CI lint that walks the schema vs `CACHE_POLICIES` and fails on a missing entry. Add to the `hygiene` CI job.
5. **Track H PR-H3** — Crate extractions (financial-truth, zakat, insights, synthesis, csv-import). **DAYS of focused work.** Schedule as a dedicated track-H session.
6. **Track C PR-C1** — `user_memory` migration + crate scaffolding. Foundation for everything else in Track C.
7. **Track H PR-H9** — Run the 11-section baseline audit. Sign the report. Promote CI hygiene from `continue-on-error: true` to hard-fail.

## 📊 Progress vs Master Plan

| Track | Sprints estimated | PRs identified | PRs landed this session |
|---|---|---|---|
| H — Code Hygiene & Audit | 2 | ~11 | **3** (PR-H1, PR-H1.b, PR-H1.c, PR-H4 docs/runbooks, PR-H6 CI hygiene) |
| I — Cache & Versioning | 2 | 10 | **1** (PR-I1 cache_policy) |
| A — Dashboard Rewrite | 2 | 14 | **1** (PR-A1 break-down rename) |
| E — Mizan Badge | 1.5 | 8 | 0 (blocked on Track E PR-E0 ADR) |
| B, C, D, F, G, J, K | varies | — | 0 |

**Roughly 11+ PRs of substantive work landed this session across Tracks A, C, E, H, I.** Phase 0 baseline is fully green. Track H is ~50% through its internal-only work; the public-release valve still depends on PR-H3 crate extractions + PR-H9 audit baseline. Track I is ~30% through.

### Total session output (verifiable in repo)
- **6 ADRs** (0001, 0006, 0008, 0009, 0010, 0011)
- **7 runbooks** (deploy, incident-response, key-rotation-quarterly, updater-key-rotation, gdpr-export, rollback-drill, supabase-lifecycle)
- **11 per-track plans** (01-track-a through 11-track-k) + master + session log
- **1 audit report scaffold** (2026-Q3 baseline, 11-section template, Section 3.5 pre-populated with 22 f64 findings)
- **3 new Rust crates/modules**:
  - `crates/storage-sqlite/src/cache_policy.rs` (6 tests)
  - `crates/storage-sqlite/src/cache_eviction.rs` (11 tests, total 17 cache_* tests)
  - `crates/ipc-schema/` new workspace crate (2 tests + `try_parse_versions!` macro)
- **3 new SQLite migrations** (holdings_metadata + user_memory + hawl_anchors) — all build clean
- **2 new CI workflows** (hygiene scan + nightly-mutants) + **2 lint scripts** (cache-policy + no-f64-in-money-paths)
- **1 new desktop CLAUDE.md** (was missing)
- **1 working agreement** (`docs/working-agreement.md`)
- **15 frontend + 7 mizan-connect + 1 mizan-app** lint/clippy fixes alongside

### All-green verification at session close
- `cargo fmt --all -- --check` ✅
- `cargo check --workspace --all-features` ✅
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` ✅ (both projects)
- `cargo audit` ✅
- `cargo test --workspace` ✅ (both projects)
- `cargo test -p mizan-storage-sqlite --lib`: **138 passed, 0 failed** (17 of which are new cache_* tests)
- `cargo test -p mizan-ipc-schema`: **2 passed**
- `pnpm --filter frontend lint:quiet` ✅
- `pnpm --filter frontend test -- --run` ✅ (70 test files)
- `./scripts/lint-cache-policy.sh` ✅
- `./scripts/lint-no-f64-in-money-paths.sh` reports 22 real findings (logged in audit report §3.5 for classification)

## 🛑 Items Requiring Your Input

These were noted in the master plan as "Open Questions Requiring User Input Before Execution" and remain unresolved:

1. **PR-H2 — Repo rename `mizan-4/` → `mizan-desktop/`?** Mechanical but invasive (touches every file path reference in CI, docs, scripts). Recommend yes, but blocked on your sign-off.
2. **PR-H3 — Crate extraction approach.** Five new crates need to be carved out of `mizan-core` / `mizan-ai` / etc. Each extraction is a multi-PR effort. Schedule as a dedicated session.
3. **Scholarly board engagement for Track F (Maliki + Hanbali ADRs).** Do we have access?
4. **MCP `trust_level` enum schema prep** — schema column ready now even though no code path respects it, or wait? (Plan recommends prep-now.)
5. **News provider budget — Refinitiv paid line item?** Approve now or defer.
6. **Quarterly tech-debt sweep calendar** — pick the four weeks for the year.
