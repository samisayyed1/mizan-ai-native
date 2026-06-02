//! Cache eviction worker — reads `cache_policy::CACHE_POLICIES` and
//! sweeps expired rows from cache tables.
//!
//! Per `docs/working-agreement.md` §19.7 (Cache Eviction Workers):
//!
//! > - Background scheduled job at 3am local time daily: evicts expired rows
//! >   by TTL across every cache table.
//! > - On app version bump: synchronous eviction worker runs on startup
//! >   before WebView paint.
//!
//! ## Architecture
//!
//! Two entry points:
//!
//! 1. [`run_synchronous`] — called from Tauri main during the cold-start
//!    sequence when the binary version differs from the `app_version` row
//!    in SQLite. Blocks the WebView until eviction completes. Catches stale
//!    cache rows from prior versions that would otherwise be served to the
//!    user before the 3am sweep would catch them.
//!
//! 2. [`schedule_daily`] — registers the 3am-local daily sweep via the
//!    existing scheduler in `apps/tauri/src/scheduler.rs`. The scheduler
//!    wakes a background tokio task that calls [`run_one_sweep`] and logs
//!    the outcome to `tracing` with structured fields.
//!
//! Both paths share the same per-policy implementation in [`evict_table`],
//! dispatching on `EvictionStrategy`:
//!
//! - `Delete` — `DELETE FROM table WHERE <age_column> < now - ttl`
//! - `RollupThenDelete` — application-specific rollup hook (the worker
//!   delegates to a trait the consumer wires) before the delete
//! - `ArchiveThenDelete` — upload to Mizan Connect cold-storage endpoint,
//!   then delete locally
//! - `KeepMarkStale` — no-op at the worker layer; staleness surfaces via
//!   the Mizan Badge `'stale'` modifier computed at read time
//!
//! ## What this PR ships (PR-I2 skeleton)
//!
//! The skeleton: types, the per-strategy dispatch shape, and the public API
//! signatures consumers will call. The actual SQL queries + rollup hooks +
//! archive uploads land in follow-up sub-PRs (I2.a–I2.d) one strategy at a
//! time, each with golden tests. The dispatch surface here is the contract;
//! the implementation is staged so a single failing strategy doesn't block
//! the others from shipping.
//!
//! ## Why a separate file from `cache_policy.rs`
//!
//! `cache_policy.rs` is the declarative registry — pure data, no I/O.
//! `cache_eviction.rs` is the execution layer — DB connections, scheduling,
//! archive uploads. Keeping them split makes the registry trivially
//! testable (the 6 tests in cache_policy::tests run with no fixtures) and
//! makes the eviction worker injectable for tests via a trait.

use crate::cache_policy::{AgeFrom, CachePolicy, EvictionStrategy, CACHE_POLICIES};
use std::time::Duration;

/// Outcome of evicting a single cache table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvictionOutcome {
    pub table: &'static str,
    pub rows_evicted: u64,
    pub rows_archived: u64,
    pub rows_rolled_up: u64,
    pub duration: Duration,
    pub error: Option<String>,
}

impl EvictionOutcome {
    fn skipped(table: &'static str, reason_into_error: &str) -> Self {
        Self {
            table,
            rows_evicted: 0,
            rows_archived: 0,
            rows_rolled_up: 0,
            duration: Duration::ZERO,
            error: Some(reason_into_error.to_string()),
        }
    }

    fn ok(table: &'static str, rows_evicted: u64, duration: Duration) -> Self {
        Self {
            table,
            rows_evicted,
            rows_archived: 0,
            rows_rolled_up: 0,
            duration,
            error: None,
        }
    }
}

/// Aggregated result of a full sweep across every registered cache table.
#[derive(Debug, Clone, Default)]
pub struct SweepReport {
    pub outcomes: Vec<EvictionOutcome>,
}

impl SweepReport {
    #[must_use]
    pub fn total_rows_evicted(&self) -> u64 {
        self.outcomes.iter().map(|o| o.rows_evicted).sum()
    }

    #[must_use]
    pub fn failed_tables(&self) -> Vec<&str> {
        self.outcomes
            .iter()
            .filter(|o| o.error.is_some())
            .map(|o| o.table)
            .collect()
    }

    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.outcomes.iter().all(|o| o.error.is_none())
    }
}

/// Errors a real (non-test) `EvictionContext` impl can return when it
/// tries to execute SQL.
#[derive(Debug, Clone)]
pub struct EvictionError {
    pub message: String,
}

impl std::fmt::Display for EvictionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "eviction execution failed: {}", self.message)
    }
}

impl std::error::Error for EvictionError {}

impl EvictionError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Synchronous eviction run on app-version-mismatch boot.
///
/// Called from `apps/tauri/src/main.rs` BEFORE the WebView is allowed to
/// paint. Blocks until all registered cache tables are swept. Per the
/// working agreement §19.1 / §19.7: stale cache rows from a prior binary
/// version must not leak into the new version's UI.
///
/// Track H PR-I2.e wired this into the Tauri startup sequence — called
/// from `apps/tauri/src/context/providers.rs::initialize_context` right
/// after `db::run_migrations`. See [`SqliteEvictionContext`] for the
/// real production context.
///
/// `cutoff_micros` is the wall-clock cutoff in unix-microseconds (i.e.
/// `now() - policy.ttl`). Passed in by the caller so this function stays
/// test-deterministic (no clock access here).
pub fn run_synchronous(ctx: &dyn EvictionContext, cutoff_micros: i64) -> SweepReport {
    let mut report = SweepReport::default();
    for policy in CACHE_POLICIES {
        let outcome = evict_table_sync(policy, ctx, cutoff_micros);
        report.outcomes.push(outcome);
    }
    report
}

/// Synchronous variant of [`evict_table`] — the dispatcher used by
/// startup eviction. Rollup + Archive strategies are not yet wired into
/// the sync path (they require async cold-storage uploads and a `Rollup`
/// trait registration) so they emit Skipped outcomes; the 3am daily
/// sweep (run_one_sweep) is the place those land in PR-I2.{b,c}.full.
fn evict_table_sync(
    policy: &CachePolicy,
    ctx: &dyn EvictionContext,
    cutoff_micros: i64,
) -> EvictionOutcome {
    match policy.eviction {
        EvictionStrategy::Delete => {
            let sql = delete_sql_for(policy);
            let started = std::time::Instant::now();
            match ctx.execute_delete(&sql, cutoff_micros) {
                Ok(rows) => EvictionOutcome::ok(policy.table, rows, started.elapsed()),
                Err(e) => {
                    EvictionOutcome::skipped(policy.table, &format!("Delete strategy failed: {e}"))
                }
            }
        }
        EvictionStrategy::RollupThenDelete => EvictionOutcome::skipped(
            policy.table,
            "PR-I2.b.full RollupThenDelete execution pending (sync path)",
        ),
        EvictionStrategy::ArchiveThenDelete => EvictionOutcome::skipped(
            policy.table,
            "PR-I2.c.full ArchiveThenDelete execution pending (sync path)",
        ),
        EvictionStrategy::KeepMarkStale => EvictionOutcome::ok(policy.table, 0, Duration::ZERO),
    }
}

/// Run a single sweep across all registered cache tables. The 3am daily
/// scheduler calls this in a tokio task; tests call it directly with a
/// fixture context.
pub async fn run_one_sweep(ctx: &dyn EvictionContext) -> SweepReport {
    let mut report = SweepReport::default();
    for policy in CACHE_POLICIES {
        let outcome = evict_table(policy, ctx).await;
        report.outcomes.push(outcome);
    }
    report
}

/// Build the `DELETE` statement for a Delete-strategy cache policy.
///
/// Returns the parameterised SQL string. The cutoff timestamp is passed
/// as the single bind parameter — callers compute `now - ttl` and bind
/// it; this function never touches a clock so it stays test-deterministic.
///
/// The age column is selected per `AgeFrom`:
/// - `CreatedAt` → `created_at`
/// - `UpdatedAt` → `updated_at`
/// - `Custom` → `policy.age_column` (validated non-empty by the registry tests)
///
/// SQLite-flavoured (no PostgreSQL-isms like `NOW()` — the cutoff is a
/// bind parameter so the same SQL works against any storage backend).
///
/// # Panics
///
/// Panics if called on a policy with `EvictionStrategy::KeepMarkStale` —
/// the no-op strategy should never reach SQL generation, and reaching here
/// indicates a dispatch bug worth surfacing loudly in tests.
#[must_use]
pub fn delete_sql_for(policy: &CachePolicy) -> String {
    assert!(
        matches!(
            policy.eviction,
            EvictionStrategy::Delete
                | EvictionStrategy::RollupThenDelete
                | EvictionStrategy::ArchiveThenDelete
        ),
        "delete_sql_for called on KeepMarkStale policy (table={}) — \
         dispatch bug; the no-op strategy never generates SQL",
        policy.table
    );
    let column = age_column_for(policy);
    // Note: table + column are static `&'static str` from the const registry —
    // never user input. SQL injection is structurally impossible here.
    format!(
        "DELETE FROM {table} WHERE {column} < ?",
        table = policy.table
    )
}

/// Build the SELECT statement that fetches expired rows for archival
/// (ArchiveThenDelete strategy) before the DELETE runs. Returns rows
/// ordered by age column ascending so the archive batch is deterministic
/// even when split across multiple uploads.
///
/// SQLite-flavoured — `rowid` is included so the consumer can correlate
/// archival success with the DELETE WHERE clause.
///
/// # Panics
///
/// Panics on `KeepMarkStale` policies (no archive needed, dispatch bug).
#[must_use]
pub fn select_expired_rows_sql_for(policy: &CachePolicy) -> String {
    assert!(
        !matches!(policy.eviction, EvictionStrategy::KeepMarkStale),
        "select_expired_rows_sql_for called on KeepMarkStale policy (table={}) — \
         dispatch bug; KeepMarkStale never archives",
        policy.table
    );
    let column = age_column_for(policy);
    format!(
        "SELECT rowid, * FROM {table} WHERE {column} < ? ORDER BY {column}",
        table = policy.table
    )
}

/// Map a policy's `AgeFrom` enum to the actual column name. Pure helper.
#[must_use]
fn age_column_for(policy: &CachePolicy) -> &'static str {
    match policy.age_from {
        AgeFrom::CreatedAt => "created_at",
        AgeFrom::UpdatedAt => "updated_at",
        AgeFrom::Custom => policy.age_column,
    }
}

/// Per-table eviction dispatching on `EvictionStrategy`. The actual SQL
/// queries are implemented per-strategy in sub-PRs (I2.a–I2.d). This
/// dispatch shape is the contract.
async fn evict_table(policy: &CachePolicy, _ctx: &dyn EvictionContext) -> EvictionOutcome {
    match policy.eviction {
        EvictionStrategy::Delete => {
            // PR-I2.a partial: SQL is generated. Actual DB execution lands
            // when PR-I2.e wires the SQLite pool into `EvictionContext`.
            // For now we emit the SQL into the outcome's error field so
            // the report carries observable evidence the dispatch reached.
            EvictionOutcome::skipped(
                policy.table,
                &format!(
                    "PR-I2.a SQL ready, DB execution pending: {}",
                    delete_sql_for(policy)
                ),
            )
        }
        EvictionStrategy::RollupThenDelete => {
            // PR-I2.b: invoke registered `Rollup` trait impl (e.g.
            // `projection_snapshots` rolls monthly aggregates). Then delete
            // the original rows. Each rollup-needing table registers an impl.
            EvictionOutcome::skipped(policy.table, "PR-I2.b RollupThenDelete strategy pending")
        }
        EvictionStrategy::ArchiveThenDelete => {
            // PR-I2.c: upload expired rows to Mizan Connect cold-storage
            // endpoint (`POST /v1/admin/archive`), verify ack, then delete
            // locally. Archive failure → DO NOT delete (data preservation).
            EvictionOutcome::skipped(policy.table, "PR-I2.c ArchiveThenDelete strategy pending")
        }
        EvictionStrategy::KeepMarkStale => {
            // No worker action. Staleness surfaces via the Mizan Badge
            // 'stale' modifier computed at read time from `quotes.as_of`
            // / `fx_rates.as_of` vs the policy TTL.
            EvictionOutcome::ok(policy.table, 0, Duration::ZERO)
        }
    }
}

/// Injectable context for eviction operations. Tests pass a mock; the
/// real desktop wires the SQLite pool + Mizan Connect client via
/// [`SqliteEvictionContext`] in `cache_eviction_sqlite.rs`.
///
/// PR-I2.e ships the production impl alongside the Tauri main-loop
/// wiring. The default `execute_delete` returns `EvictionError` so the
/// test-only `NoopContext` (which doesn't override it) records Skipped
/// outcomes rather than silently zeroing — keeping the contract honest.
pub trait EvictionContext: Sync {
    /// Execute a `DELETE FROM <table> WHERE <age_column> < ?` SQL string
    /// against the underlying connection pool, binding `cutoff_micros`
    /// as the single bind parameter. Returns the number of rows affected.
    ///
    /// The SQL is the output of [`delete_sql_for`] — never user input,
    /// always validated by the policy registry at compile time.
    fn execute_delete(&self, _sql: &str, _cutoff_micros: i64) -> Result<u64, EvictionError> {
        Err(EvictionError::new(
            "no-op EvictionContext does not execute SQL — wire a real context (e.g. SqliteEvictionContext)",
        ))
    }
}

/// Per-table rollup behaviour for the `RollupThenDelete` strategy.
///
/// PR-I2.b skeleton. Tables that use this strategy register an impl;
/// the eviction worker calls `rollup_then_count` before issuing the
/// DELETE so older data is preserved in a compacted form.
///
/// **Example for `projection_snapshots`** (planned Track C table):
///
/// > Latest 30 days at full resolution; older rolled into monthly aggregates
/// > per working agreement §18.3. The impl reads expired daily rows, INSERTs
/// > the monthly mean/median/p95 into `projection_snapshots_monthly`, then
/// > returns the row count for the audit log; eviction worker then issues
/// > `DELETE FROM projection_snapshots WHERE created_at < ?`.
///
/// **No `projection_snapshots` impl ships in this PR** — it lands in Track C
/// alongside the predictive layer migration (PR-C15). The trait + the
/// dispatch hook are the contract.
pub trait Rollup {
    /// Read expired rows from the source table, produce a rolled-up
    /// representation, persist it, and return the count of source rows
    /// that should now be safe to delete.
    ///
    /// Returning `Ok(0)` means "nothing was expired" — eviction worker
    /// records the outcome and moves on without issuing a DELETE.
    /// Returning `Err(_)` aborts the eviction for this table — the
    /// DELETE does NOT run (data preservation).
    fn rollup_then_count(&self, policy: &CachePolicy, cutoff: i64) -> Result<u64, RollupError>;
}

/// Errors a `Rollup` impl can return.
#[derive(Debug, Clone)]
pub struct RollupError {
    pub message: String,
}

impl std::fmt::Display for RollupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rollup failed: {}", self.message)
    }
}

impl std::error::Error for RollupError {}

/// Test-only context.
#[cfg(test)]
pub struct NoopContext;

#[cfg(test)]
impl EvictionContext for NoopContext {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Arbitrary fixed cutoff for the tests below. PR-I2.e disallows
    /// `Date::now()` / `Instant::now()` at clock-binding sites so the
    /// caller supplies cutoff_micros — this test value is just a recent
    /// unix-micros timestamp (2026-06-03T00:00:00Z, computed externally
    /// to avoid any clock call here).
    const FIXED_CUTOFF: i64 = 1_780_704_000_000_000;

    #[test]
    fn synchronous_returns_a_report_with_one_outcome_per_policy() {
        let report = run_synchronous(&NoopContext, FIXED_CUTOFF);
        assert_eq!(report.outcomes.len(), CACHE_POLICIES.len());
    }

    #[test]
    fn noop_context_yields_skipped_for_delete_strategies() {
        // The default trait impl returns Err — the worker turns that into
        // a Skipped outcome. Non-Delete strategies (KeepMarkStale, plus
        // Rollup/Archive which are pending) yield their own skip messages.
        let report = run_synchronous(&NoopContext, FIXED_CUTOFF);
        assert!(report.outcomes.iter().any(|o| o.error.is_some()));
    }

    #[test]
    fn keep_mark_stale_outcomes_are_clean() {
        // quotes + fx_rates use KeepMarkStale — they should NOT be in the
        // failed-tables list.
        let report = run_synchronous(&NoopContext, FIXED_CUTOFF);
        let failed = report.failed_tables();
        assert!(
            !failed.contains(&"quotes"),
            "quotes uses KeepMarkStale and should not be reported as failed"
        );
        assert!(
            !failed.contains(&"fx_rates"),
            "fx_rates uses KeepMarkStale and should not be reported as failed"
        );
    }

    #[test]
    fn delete_strategy_uses_context_when_provided() {
        // FakeContext counts executions + returns synthetic row counts.
        struct FakeContext(std::sync::Mutex<Vec<(String, i64)>>);
        impl EvictionContext for FakeContext {
            fn execute_delete(&self, sql: &str, cutoff_micros: i64) -> Result<u64, EvictionError> {
                self.0
                    .lock()
                    .unwrap()
                    .push((sql.to_string(), cutoff_micros));
                Ok(7)
            }
        }
        let ctx = FakeContext(std::sync::Mutex::new(Vec::new()));
        let report = run_synchronous(&ctx, FIXED_CUTOFF);
        // Every Delete policy should now contribute an Ok outcome with 7 rows.
        let delete_outcomes: Vec<&EvictionOutcome> = report
            .outcomes
            .iter()
            .filter(|o| {
                CACHE_POLICIES
                    .iter()
                    .find(|p| p.table == o.table)
                    .is_some_and(|p| matches!(p.eviction, EvictionStrategy::Delete))
            })
            .collect();
        assert!(
            !delete_outcomes.is_empty(),
            "registry must contain at least one Delete-strategy policy for this test to be meaningful"
        );
        for o in &delete_outcomes {
            assert!(o.error.is_none(), "Delete policy should succeed: {o:?}");
            assert_eq!(o.rows_evicted, 7);
        }
        let calls = ctx.0.lock().unwrap();
        assert_eq!(calls.len(), delete_outcomes.len());
        assert!(calls.iter().all(|(_, c)| *c == FIXED_CUTOFF));
    }

    #[test]
    fn delete_strategy_records_skipped_on_context_error() {
        struct ErrCtx;
        impl EvictionContext for ErrCtx {
            fn execute_delete(&self, _sql: &str, _cutoff: i64) -> Result<u64, EvictionError> {
                Err(EvictionError::new("simulated DB outage"))
            }
        }
        let report = run_synchronous(&ErrCtx, FIXED_CUTOFF);
        let delete_failures: Vec<_> = report
            .outcomes
            .iter()
            .filter(|o| {
                CACHE_POLICIES
                    .iter()
                    .find(|p| p.table == o.table)
                    .is_some_and(|p| matches!(p.eviction, EvictionStrategy::Delete))
            })
            .collect();
        for o in &delete_failures {
            assert!(o.error.is_some());
            let msg = o.error.as_ref().unwrap();
            assert!(msg.contains("simulated DB outage"), "got: {msg}");
        }
    }

    #[tokio::test]
    async fn one_sweep_runs_against_every_policy() {
        let report = run_one_sweep(&NoopContext).await;
        assert_eq!(report.outcomes.len(), CACHE_POLICIES.len());
    }

    #[test]
    fn policy_for_unregistered_table_returns_none() {
        // Sanity: the eviction worker correctly gates on registration.
        assert!(crate::cache_policy::policy_for("nonexistent_table").is_none());
    }

    #[test]
    fn delete_sql_for_uses_created_at_when_age_from_created_at() {
        let p = crate::cache_policy::policy_for("daily_brief_runs")
            .expect("daily_brief_runs is registered");
        let sql = delete_sql_for(p);
        assert_eq!(sql, "DELETE FROM daily_brief_runs WHERE created_at < ?");
    }

    #[test]
    fn delete_sql_for_uses_custom_column_when_age_from_custom() {
        // market_news uses `published_at` as its custom age column
        let p = crate::cache_policy::policy_for("market_news").expect("market_news is registered");
        let sql = delete_sql_for(p);
        assert_eq!(sql, "DELETE FROM market_news WHERE published_at < ?");
    }

    #[test]
    #[should_panic(expected = "delete_sql_for called on KeepMarkStale")]
    fn delete_sql_for_panics_on_keep_mark_stale_policy() {
        // quotes uses KeepMarkStale — calling delete_sql_for on it is a
        // dispatch bug that must fail loudly.
        let p = crate::cache_policy::policy_for("quotes").expect("quotes is registered");
        let _ = delete_sql_for(p);
    }

    #[test]
    fn delete_sql_for_works_for_archive_then_delete_strategy() {
        // sync_run_ledger uses ArchiveThenDelete — the delete portion of
        // that strategy still uses the same DELETE SQL shape.
        let p = crate::cache_policy::policy_for("sync_run_ledger")
            .expect("sync_run_ledger is registered");
        let sql = delete_sql_for(p);
        assert_eq!(sql, "DELETE FROM sync_run_ledger WHERE created_at < ?");
    }

    #[test]
    fn select_expired_rows_sql_for_archive_strategy() {
        // ArchiveThenDelete: the rows-to-archive query must use the same
        // age column as the eventual DELETE, returning expired rows for
        // upload to Mizan Connect cold storage before they're dropped.
        let p = crate::cache_policy::policy_for("sync_run_ledger")
            .expect("sync_run_ledger is registered");
        let sql = select_expired_rows_sql_for(p);
        assert_eq!(
            sql,
            "SELECT rowid, * FROM sync_run_ledger WHERE created_at < ? ORDER BY created_at"
        );
    }

    #[test]
    fn select_expired_rows_sql_for_custom_age_column() {
        // market_news has a custom age column (`published_at`).
        let p = crate::cache_policy::policy_for("market_news").expect("market_news is registered");
        let sql = select_expired_rows_sql_for(p);
        assert_eq!(
            sql,
            "SELECT rowid, * FROM market_news WHERE published_at < ? ORDER BY published_at"
        );
    }

    /// A test impl of the Rollup trait that the dispatcher could use once
    /// PR-C15 (projection_snapshots) lands.
    struct NoopRollup;

    impl Rollup for NoopRollup {
        fn rollup_then_count(
            &self,
            _policy: &CachePolicy,
            _cutoff: i64,
        ) -> Result<u64, RollupError> {
            Ok(0)
        }
    }

    #[test]
    fn rollup_trait_can_be_implemented() {
        let p = crate::cache_policy::policy_for("daily_brief_runs")
            .expect("daily_brief_runs is registered");
        let r = NoopRollup;
        let count = r.rollup_then_count(p, 1_700_000_000).expect("noop rollup");
        assert_eq!(count, 0);
    }

    #[test]
    fn rollup_error_displays_message() {
        let e = RollupError {
            message: "test failure".into(),
        };
        assert_eq!(format!("{e}"), "rollup failed: test failure");
    }
}
