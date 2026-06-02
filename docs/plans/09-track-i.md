# Track I — Cache Invalidation & Versioning Hardening

**Status:** In progress (PR-I1 done; PR-I1.b, I2–I10 pending).
**Estimated sprints:** 2.
**Source:** `docs/plans/00-master-plan.md` → "Track I — Cache Invalidation & Versioning Hardening".

## Scope

**In:** `cache_policy.rs` registry, app-version-mismatch eviction worker, Tauri updater pre-update snapshot + post-install self-test + auto-rollback, `X-Mizan-Client-Version` negotiation, IPC schema versioning crate, Vite hash verification, quarterly rollback drill runbook, Supabase Postgres lifecycle hygiene checklist.

**Out:** new features in any other track; cache policy decisions for tables that don't exist yet.

## PRs

| # | Status | Title | Scope |
|---|---|---|---|
| I1 | ✅ Done | `cache_policy.rs` registry skeleton + 6 unit tests | `crates/storage-sqlite/src/cache_policy.rs`, registered in `lib.rs`. ADR 0008. |
| I1.b | ✅ Done | `scripts/lint-cache-policy.sh` CI lint | Bash script using awk to parse Diesel schema.rs + grep CREATE TABLE. Wired into `hygiene` CI job. Caught `market_news` missing on first run — added. |
| I2 | ✅ Done (skeleton) | `cache_eviction.rs` worker reading the registry | `SweepReport` + `EvictionOutcome` + `EvictionContext` trait + per-strategy dispatch. `KeepMarkStale` fully implemented; Delete/Rollup/Archive strategies stubbed for I2.a–c. 5 tests passing. |
| I2.a | ✅ Done (SQL only) | Delete-strategy SQL | `delete_sql_for(policy) → String` generator + 4 tests. Pure function — actual DB execution deferred to I2.e. |
| I2.b | ✅ Done (trait only) | RollupThenDelete-strategy trait | `Rollup` trait + `RollupError` + test impl. Per-table impls (projection_snapshots → monthly aggregates) land with Track C PR-C15. |
| I2.c | ✅ Done (SELECT SQL) | ArchiveThenDelete-strategy SELECT | `select_expired_rows_sql_for(policy) → String` for archive batch retrieval + 2 tests. Cold-storage upload endpoint pending. |
| I2.d | ✅ Done | Migration manifest-comment CI lint | `scripts/lint-migration-cache-manifest.sh` + wired into CI hygiene. Enforces `-- caches-evicted:` on all migrations ≥ 2026-06-02. Today's 3 migrations all pass. |
| I2.e | ⏸️ Pending | Wire `run_synchronous` into Tauri startup | Called before WebView paint when binary version != `app_version` row. |
| I3 | ✅ Done (skeleton) | `crates/ipc-schema/` versioned Tauri command bindings | Cargo.toml + lib.rs + `try_parse_versions!` macro + `commands/notifications.rs` v1 worked example. ts-export feature gates `ts-rs` codegen. 2 tests passing. |
| I2.b | ⏸️ Pending | Migration manifest-comment CI lint | Every cache-table-touching migration must carry `-- caches-evicted: [list]`. Lint rejects otherwise. |
| I3 | ⏸️ Pending | `crates/ipc-schema/` shared crate skeleton | Versioned Tauri command request/response types (Rust + TS bindings). ADR 0010. |
| I4 | ⏸️ Pending | Updater pre-update DB snapshot | Copy `mizan.db` to `mizan.db.pre-{old_version}`, retain 30d. |
| I5 | ⏸️ Pending | Updater post-install self-test | Schema match + crypto round-trip + Twelve Data heartbeat + Mizan Connect heartbeat + Truth Ledger chain head verification. |
| I6 | ⏸️ Pending | Updater auto-rollback on self-test failure | Restore snapshot if self-test fails. ADR 0009. |
| I7 | ⏸️ Pending | `X-Mizan-Client-Version` middleware on Mizan Connect | Header parsing + version branch in handlers during transitions. |
| I8 | ⏸️ Pending | Vite content-hash bundle verification on WebView load | Mismatch → wipe and reload. |
| I9 | ⏸️ Pending | First rollback drill scheduled | Per `docs/runbooks/rollback-drill.md`. Drill report at `docs/runbooks/drill-reports/2026-Q3-rollback-drill.md`. |
| I10 | ⏸️ Pending | First Supabase lifecycle review scheduled | Per `docs/runbooks/supabase-lifecycle.md`. Drill report. |

## ADRs

- **0008 — Cache Policy Single Source of Truth** ✅ written
- **0009 — Updater snapshot and rollback design** ⏸️ pending (lands with PR-I4/I5/I6)
- **0010 — IPC schema versioning** ⏸️ pending (lands with PR-I3)

## Definition of Done (Track I)

- Every cache table in the SQLite store has a `cache_policy.rs` entry
- CI lint rejects new cache tables without policy registration
- Updater pre-update snapshot ships and is verified to restore on failed self-test
- One full rollback drill completed end-to-end in staging
- One full Supabase lifecycle review completed
- `X-Mizan-Client-Version` header parsed in all Mizan Connect handlers that branch on version

## Open Questions

- Tauri updater signed-manifest infrastructure — confirm or build in PR-I4?
- Sentry error-rate threshold for canary auto-rollback (working agreement §19.8) — exact number? **Recommend:** 2× rolling-24h average sustained for > 15 min, matching the §15.10 alerting calibration.

## What's done this session (2026-06-02)

- PR-I1 — cache_policy.rs registry + 6 passing unit tests + ADR 0008
- 5 EvictionStrategy variants modeled (Delete / RollupThenDelete / ArchiveThenDelete / KeepMarkStale)
- 5 initial cache table entries (quotes, fx_rates, daily_brief_runs, sync_run_ledger, market_news)
- 5 commented placeholder slots for Track C tables ready for activation
- PR-I1.b — `scripts/lint-cache-policy.sh` + wired into CI `hygiene` job. Caught `market_news` missing on first run.
- PR-I2 (skeleton) — `cache_eviction.rs` worker + `EvictionContext` trait + per-strategy dispatch
- PR-I2.a (SQL only) — `delete_sql_for(policy)` pure generator
- PR-I2.b (trait only) — `Rollup` trait + `RollupError`
- PR-I2.c (SELECT SQL) — `select_expired_rows_sql_for(policy)`
- PR-I3 (skeleton) — `crates/ipc-schema/` new workspace crate + `try_parse_versions!` macro + `commands::notifications` v1 worked example
- ADR 0008 — Cache policy single source of truth
- ADR 0009 — Updater snapshot & rollback design
- ADR 0010 — IPC schema versioning
- **19 cache_* tests passing** (6 cache_policy + 13 cache_eviction including SQL generators, Rollup trait, and panic-on-misuse cases)
