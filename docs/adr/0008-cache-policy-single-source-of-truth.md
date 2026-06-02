# ADR 0008 — Cache Policy Single Source of Truth

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** I (Cache Invalidation & Versioning Hardening) — PR-I1

## Context

Per the working agreement §10 (Database Discipline) and §19.7 (Cache Eviction Workers):

> Every cache table in SQLite has a TTL declared in `crates/storage-sqlite/src/cache_policy.rs`. New cache tables without an entry are rejected by CI lint. No cache row lives forever.

Today, cache TTLs are scattered across the codebase:
- `quotes` freshness lives inside the market-data service
- `fx_rates` freshness logic is implicit in the FX read path
- `daily_brief_runs` retention is ad-hoc
- Future tables (`user_memory`, `news_items`, `projection_snapshots`, `agent_audit_log`) per Tracks C/D will compound the dispersal

The QA Pass 3 + Pass 8 + Pass 11 + Pass 14 history shows the pattern: silent data staleness is the failure mode no one notices until a user spots a wrong number on screen. Centralising cache policy is a structural fix.

## Decision

Establish `crates/storage-sqlite/src/cache_policy.rs` as the **single authoritative registry of cache TTL + eviction-strategy entries**. Every cache table in the SQLite store must appear in `CACHE_POLICIES`.

The registry exposes a typed `CachePolicy` struct:

```rust
pub struct CachePolicy {
    pub table: &'static str,
    pub ttl: Duration,
    pub age_from: AgeFrom,         // CreatedAt / UpdatedAt / Custom
    pub age_column: &'static str,  // when age_from == Custom
    pub eviction: EvictionStrategy, // Delete / RollupThenDelete / ArchiveThenDelete / KeepMarkStale
    pub purpose: &'static str,      // human-readable for diagnostic bundles
}
```

The CI lint `scripts/lint-cache-policy.sh` (planned PR-I1.b) walks the schema for any table not registered in `CACHE_POLICIES` and fails the build. Migrations touching a registered cache table emit a manifest comment `-- caches-evicted: [list]` (planned PR-I2 enforces).

The eviction worker (`cache_eviction.rs`, planned PR-I2) reads the registry on startup and on the daily 3am sweep.

## Rationale

**Why a const slice rather than a config file:**
- Compile-time visibility — adding a cache table without a policy fails to compile (after PR-I1.b's CI lint runs)
- Type-safe — `AgeFrom::Custom` requires a non-empty `age_column` (tested)
- Refactoring-friendly — IDE renames work across registry references
- Diagnostic-bundle friendly — the `purpose` field is queried at runtime for the Support Diagnostic Bundle (§A17)

**Why per-policy `EvictionStrategy` enum rather than just TTL:**
- `quotes` and `fx_rates` should NOT auto-delete on TTL — they should mark stale via Mizan Badge `'stale'` and continue to serve while the badge nudges the user to refresh. Captured as `EvictionStrategy::KeepMarkStale`.
- `projection_snapshots` rolls up to monthly aggregates rather than hard-deleting (§18.3). Captured as `RollupThenDelete`.
- `agent_audit_log` archives to Mizan Connect cold storage after 12 months (§18.3). Captured as `ArchiveThenDelete`.
- Modeling the strategy in the registry keeps the eviction worker simple — one match arm per strategy, all data-driven.

**Why durable tables (truth_ledger, accounts, hawl_anchors) are explicitly absent:**
- The registry's `policy_for(table) -> Option` returns `None` for non-cache tables
- The test `policy_for_returns_none_for_durable_table` pins this behavior
- Eviction worker treats `None` as "do not touch" — protects against accidental deletion of financial state

## Consequences

**Positive:**
- One file to review when assessing cache hygiene; one file to update when adding a new cache table
- The 6 unit tests in `cache_policy::tests` form a permanent contract — duplicate entries, zero TTL, mismatched `AgeFrom::Custom`/`age_column`, missing purpose all fail at test time
- The `purpose` field powers the Support Diagnostic Bundle's cache section (§A17)
- Future cache table additions in Tracks C/D arrive with their policy automatically declared (placeholder slots already in the registry as commented-out stubs)

**Negative:**
- One additional file to remember when adding a cache table (mitigated by CI lint forcing the entry)
- Const-slice approach means adding entries requires a compile + ship, not a config push (acceptable trade-off — cache policy is structural, not operational)

**Follow-ups (tracked):**
- PR-I1.b: `scripts/lint-cache-policy.sh` walking the schema vs `CACHE_POLICIES`
- PR-I2: `cache_eviction.rs` worker reading the registry
- PR-I2.b: migration manifest-comment CI lint (`-- caches-evicted:`)
- Tracks C/D: uncomment the placeholder entries as those tables land

## Alternatives Considered

**Alternative A: Annotate cache tables with a custom `#[cache(ttl = 15s)]` attribute.** Rejected — schema tables aren't Rust structs, so this would require a parallel DSL or codegen that adds complexity for no gain.

**Alternative B: Store cache policy in a config file (YAML / TOML).** Rejected — typo-prone, no compile-time validation of `AgeFrom::Custom`/`age_column` cross-reference, and the runtime cost of parsing on every startup is wasteful for a static config.

**Alternative C: Per-module cache policy (e.g., `crates/storage-sqlite/src/quotes/cache_policy.rs`).** Rejected — defeats the "single source of truth" goal. Reviewers and the eviction worker would have to discover policies across the codebase.

## References

- `crates/storage-sqlite/src/cache_policy.rs` — the registry
- `docs/working-agreement.md` §10, §19.7, §18.3
- `docs/plans/00-master-plan.md` Track I
- ADR 0009 (planned) — Updater snapshot & rollback (consumes the registry on app-version-mismatch eviction)
