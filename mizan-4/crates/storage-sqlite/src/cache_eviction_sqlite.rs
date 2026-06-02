//! Production [`EvictionContext`] backed by the Diesel SQLite pool +
//! the app-version startup helper that decides whether to run a
//! synchronous eviction.
//!
//! # Why this lives in `mizan-storage-sqlite`
//!
//! `cache_eviction.rs` ships the *contract* (trait + dispatch + SQL
//! generators). It is intentionally storage-backend-agnostic — the
//! Rollup / Archive strategies that land in later sub-PRs will need
//! cloud uploads, not SQLite. This file ships the concrete SQLite-backed
//! impl that the desktop wires at startup.
//!
//! Per Track H PR-I2.e (the startup wire-up).

use std::sync::Arc;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel::sqlite::Sqlite;

use crate::cache_eviction::{EvictionContext, EvictionError};
use crate::db::{get_connection, DbPool};

/// The production [`EvictionContext`] used by the Tauri startup
/// eviction call. Owns an `Arc<DbPool>` so it can hand out
/// short-lived connections per `execute_delete` call.
pub struct SqliteEvictionContext {
    pool: Arc<DbPool>,
}

impl SqliteEvictionContext {
    #[must_use]
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

impl EvictionContext for SqliteEvictionContext {
    fn execute_delete(&self, sql: &str, cutoff_micros: i64) -> Result<u64, EvictionError> {
        let mut conn = get_connection(&self.pool)
            .map_err(|e| EvictionError::new(format!("get_connection failed: {e}")))?;

        let rows = sql_query(sql)
            .bind::<BigInt, _>(cutoff_micros)
            .execute(&mut conn)
            .map_err(|e| EvictionError::new(format!("DELETE execute failed: {e}")))?;

        // Diesel returns `usize`; cast to u64 for the worker contract.
        // usize → u64 is always safe on 64-bit + lossless on 32-bit
        // (row counts never realistically exceed u32::MAX in a single
        // sweep against a cache table).
        Ok(rows as u64)
    }
}

// Diesel sql_query needs the SQL string to be `Send + 'static` in
// async contexts; we sit on a sync trait so this is a no-op marker
// to assure the compiler this struct is `Sync` even though it owns
// a connection pool (`Arc<DbPool>` is `Send + Sync`).
// `Sqlite` import above pins the diesel backend choice; suppress the
// unused-import warning that would otherwise fire when sql_query is
// the only consumer.
#[allow(dead_code)]
const _BACKEND: std::marker::PhantomData<Sqlite> = std::marker::PhantomData;

// ---------------------------------------------------------------------------
// App-version startup helper
// ---------------------------------------------------------------------------

/// The key under which the binary's last-seen version is recorded in
/// `app_settings`. Matches the convention of other `app_settings` keys
/// (theme, font, base_currency, etc.).
pub const APP_VERSION_SETTING_KEY: &str = "app_version";

/// The outcome of the startup version check — passed by the caller into
/// [`run_synchronous_if_app_version_changed`] in `apps/tauri`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppVersionCheck {
    /// First boot, or the row was just created. No eviction needed.
    FirstBoot,
    /// Same version as last boot. No eviction needed.
    Unchanged,
    /// The binary's version differs from the persisted one. Eviction
    /// should run; carries the previous version for the audit log.
    Changed { previous: String },
}

/// Read `app_settings.app_version` and compare against `current_version`
/// (typically `env!("CARGO_PKG_VERSION")`). Returns an
/// [`AppVersionCheck`] describing the boot state.
///
/// Does NOT write the new version — callers do that after eviction runs
/// so a crash mid-eviction doesn't lose the "must re-run eviction" signal.
pub fn check_app_version(
    pool: &DbPool,
    current_version: &str,
) -> Result<AppVersionCheck, EvictionError> {
    use crate::schema::app_settings::dsl::{app_settings, setting_key, setting_value};

    let mut conn = get_connection(pool)
        .map_err(|e| EvictionError::new(format!("get_connection (check) failed: {e}")))?;

    let existing: Option<String> = app_settings
        .filter(setting_key.eq(APP_VERSION_SETTING_KEY))
        .select(setting_value)
        .first::<String>(&mut conn)
        .optional()
        .map_err(|e| EvictionError::new(format!("read app_version failed: {e}")))?;

    match existing {
        None => Ok(AppVersionCheck::FirstBoot),
        Some(prev) if prev == current_version => Ok(AppVersionCheck::Unchanged),
        Some(prev) => Ok(AppVersionCheck::Changed { previous: prev }),
    }
}

/// Persist the binary's current version into `app_settings`. Called by
/// the desktop after the eviction sweep completes (or immediately on
/// FirstBoot — no eviction needed but the row needs to exist for the
/// next boot to register as Unchanged).
///
/// Uses an UPSERT (INSERT OR REPLACE) so the same call works whether or
/// not the row exists yet.
pub fn persist_app_version(pool: &DbPool, version: &str) -> Result<(), EvictionError> {
    use crate::schema::app_settings::dsl::{app_settings, setting_key, setting_value};

    let mut conn = get_connection(pool)
        .map_err(|e| EvictionError::new(format!("get_connection (persist) failed: {e}")))?;

    diesel::replace_into(app_settings)
        .values((
            setting_key.eq(APP_VERSION_SETTING_KEY),
            setting_value.eq(version),
        ))
        .execute(&mut conn)
        .map_err(|e| EvictionError::new(format!("persist app_version failed: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::cache_eviction::run_synchronous;
    use crate::cache_policy::CACHE_POLICIES;
    use diesel::r2d2::{ConnectionManager, Pool};
    use tempfile::NamedTempFile;

    fn fresh_pool() -> (Arc<DbPool>, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let url = tmp.path().to_str().unwrap().to_string();
        let manager = ConnectionManager::<SqliteConnection>::new(url);
        let pool: DbPool = Pool::builder().max_size(2).build(manager).unwrap();
        let pool = Arc::new(pool);
        // Run migrations against the temp DB so app_settings exists.
        crate::db::run_migrations(tmp.path().to_str().unwrap()).unwrap();
        (pool, tmp)
    }

    #[test]
    fn first_boot_reports_first_boot() {
        let (pool, _tmp) = fresh_pool();
        let check = check_app_version(&pool, "3.4.1").unwrap();
        assert_eq!(check, AppVersionCheck::FirstBoot);
    }

    #[test]
    fn persist_then_check_reports_unchanged() {
        let (pool, _tmp) = fresh_pool();
        persist_app_version(&pool, "3.4.1").unwrap();
        let check = check_app_version(&pool, "3.4.1").unwrap();
        assert_eq!(check, AppVersionCheck::Unchanged);
    }

    #[test]
    fn version_bump_reports_changed_with_previous() {
        let (pool, _tmp) = fresh_pool();
        persist_app_version(&pool, "3.4.0").unwrap();
        let check = check_app_version(&pool, "3.4.1").unwrap();
        assert_eq!(
            check,
            AppVersionCheck::Changed {
                previous: "3.4.0".to_string()
            }
        );
    }

    #[test]
    fn persist_is_idempotent_replace() {
        let (pool, _tmp) = fresh_pool();
        persist_app_version(&pool, "3.4.0").unwrap();
        persist_app_version(&pool, "3.4.1").unwrap();
        let check = check_app_version(&pool, "3.4.1").unwrap();
        assert_eq!(check, AppVersionCheck::Unchanged);
    }

    #[test]
    fn sqlite_eviction_context_executes_delete_against_real_db() {
        let (pool, _tmp) = fresh_pool();
        let ctx = SqliteEvictionContext::new(Arc::clone(&pool));

        // FIXED_CUTOFF matches cache_eviction's test cutoff for consistency.
        // Real DELETE queries hit the registered cache tables — they
        // start empty after migrations, so rows_evicted == 0 for each,
        // but the SQL executes cleanly (no failures recorded).
        let report = run_synchronous(&ctx, 1_780_704_000_000_000);

        let delete_outcomes: Vec<_> = report
            .outcomes
            .iter()
            .filter(|o| {
                CACHE_POLICIES
                    .iter()
                    .find(|p| p.table == o.table)
                    .is_some_and(|p| {
                        matches!(p.eviction, crate::cache_policy::EvictionStrategy::Delete)
                    })
            })
            .collect();
        assert!(
            !delete_outcomes.is_empty(),
            "registry must contain at least one Delete-strategy policy"
        );
        // Tables registered in CACHE_POLICIES ahead of their migration
        // (the registry intentionally leads by a release) yield a "no such
        // table" error. That's an expected steady-state for future tables —
        // not a worker bug. We assert that EITHER the outcome is clean
        // (rows_evicted == 0, no error) OR the error is the known
        // "no such table" SQLite message.
        for o in &delete_outcomes {
            if let Some(err) = &o.error {
                assert!(
                    err.contains("no such table"),
                    "unexpected error for table={}: {err}",
                    o.table
                );
            } else {
                assert_eq!(o.rows_evicted, 0, "fresh DB has no rows to evict");
            }
        }
        // At least one outcome MUST be clean to prove the happy-path works.
        assert!(
            delete_outcomes.iter().any(|o| o.error.is_none()),
            "expected at least one Delete-strategy policy to target a migrated \
             table and succeed cleanly; if this fires, every Delete policy is \
             registered ahead of its migration which deserves a closer look"
        );
    }
}
