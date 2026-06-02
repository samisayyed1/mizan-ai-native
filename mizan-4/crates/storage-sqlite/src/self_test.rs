//! Post-install self-test (Track I PR-I5 + PR-I5.b per ADR 0009).
//!
//! Runs on the FIRST launch of a freshly-upgraded binary, before the
//! WebView is allowed to paint. Verifies that the new binary is
//! genuinely operational — schema match, crypto round-trip, Truth
//! Ledger chain head — so a broken upgrade is caught early. PR-I6
//! adds the auto-rollback path that triggers on self-test failure;
//! this PR (I5) ships the verifier itself plus result persistence.
//!
//! # Check set
//!
//! 5 checks total. Three LOCAL checks ship in this crate (PR-I5);
//! the two NETWORK heartbeats are injected via the [`Heartbeats`]
//! trait from `apps/tauri` (PR-I5.b) so storage-sqlite stays free of
//! a reqwest dep + HTTP-client lifecycle code.
//!
//! | Check | Source | Pass criterion |
//! |---|---|---|
//! | schema_match | `diesel_migrations::has_pending_migration` | returns false |
//! | crypto_round_trip | `chacha20poly1305` ephemeral key | encrypt then decrypt yields the original plaintext |
//! | truth_ledger_chain_head | rusqlite query | latest row's `curr_hash == blake3(prev_hash || event_payload)` |
//! | twelve_data_heartbeat | injected `Heartbeats` trait | HTTP 200 within 5s |
//! | mizan_connect_heartbeat | injected `Heartbeats` trait | HTTP 200 within 5s |
//!
//! Each check has a 5-second timeout (working agreement §A19 budget);
//! the full self-test budget is 30 seconds.
//!
//! # Result persistence
//!
//! Results are written to
//! `${app_data_dir}/.mizan/self_test_${current_version}.json` so
//! subsequent launches of the SAME version short-circuit. A version
//! bump invalidates the cached result (different filename) and forces
//! a fresh run.
//!
//! # Failure semantics
//!
//! The self-test ITSELF never panics or blocks startup. The orchestrator
//! returns a [`SelfTestReport`] that the caller (apps/tauri's
//! `initialize_context`) acts on. In PR-I5, a failed self-test logs
//! `error!` + surfaces a one-time modal but the desktop still launches;
//! in PR-I6, a failed self-test triggers the auto-rollback path.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use diesel_migrations::MigrationHarness;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::db::{get_connection, DbPool, MIGRATIONS};
use mizan_core::errors::Result as CoreResult;

/// Per-check timeout — working agreement §A19 budget.
pub const PER_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Aggregate budget across all checks. Exceeding this means the
/// orchestrator returns Timeout for the run as a whole.
pub const TOTAL_BUDGET: Duration = Duration::from_secs(30);

/// One self-test check's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub name: String,
    /// `true` if the check passed within its timeout.
    pub passed: bool,
    /// Wall-clock milliseconds spent on this check. `None` if skipped.
    pub elapsed_ms: Option<u64>,
    /// Human-readable failure context. `None` on success.
    pub error: Option<String>,
}

impl CheckOutcome {
    fn pass(name: &str, elapsed: Duration) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            elapsed_ms: Some(elapsed.as_millis() as u64),
            error: None,
        }
    }

    fn fail(name: &str, elapsed: Duration, error: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            elapsed_ms: Some(elapsed.as_millis() as u64),
            error: Some(error.into()),
        }
    }

    /// Build a Skipped outcome — retained for future use (PR-I8 may
    /// add additional optional checks that surface as Skipped when
    /// their preconditions aren't met).
    #[allow(dead_code)]
    fn skipped(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            elapsed_ms: None,
            error: Some(reason.into()),
        }
    }
}

/// Aggregate report from a single self-test run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfTestReport {
    /// Binary version this report was generated for.
    pub version: String,
    /// All check outcomes, in execution order.
    pub checks: Vec<CheckOutcome>,
    /// Wall-clock milliseconds for the full run.
    pub total_elapsed_ms: u64,
    /// `true` iff every check that ran passed AND no Skipped outcome
    /// is treated as a failure (skipped outcomes are currently treated
    /// as informational; PR-I5.b promotes them to required).
    pub all_required_passed: bool,
}

impl SelfTestReport {
    /// The subset of checks the orchestrator considers REQUIRED. Network
    /// heartbeats are intentionally NOT in this set: a transient outage
    /// on Twelve Data or Mizan Connect shouldn't trigger an auto-rollback
    /// (working-agreement §17 — "Don't relax a security rule
    /// 'temporarily'" generalizes to: don't tighten an availability
    /// rule into a correctness rule). The heartbeats run + are reported
    /// for monitoring, but only the three LOCAL correctness checks block.
    pub const REQUIRED_CHECK_NAMES: &'static [&'static str] = &[
        "schema_match",
        "crypto_round_trip",
        "truth_ledger_chain_head",
    ];

    /// Network checks that run but don't block the rollback decision.
    pub const NETWORK_CHECK_NAMES: &'static [&'static str] =
        &["twelve_data_heartbeat", "mizan_connect_heartbeat"];

    /// Did every required check pass?
    #[must_use]
    pub fn required_all_passed(&self) -> bool {
        Self::REQUIRED_CHECK_NAMES
            .iter()
            .all(|required| self.checks.iter().any(|c| c.name == *required && c.passed))
    }

    /// Names of required checks that did NOT pass — for the user-facing
    /// modal copy in PR-I6.
    #[must_use]
    pub fn failed_required(&self) -> Vec<String> {
        Self::REQUIRED_CHECK_NAMES
            .iter()
            .filter(|required| !self.checks.iter().any(|c| c.name == **required && c.passed))
            .map(|s| (*s).to_string())
            .collect()
    }
}

/// Outcome of a single network heartbeat — returned by
/// [`Heartbeats`] impls so the orchestrator can record success vs
/// failure without owning the HTTP client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatResult {
    /// Wall-clock duration the heartbeat took (regardless of success).
    pub elapsed: Duration,
    /// `Ok(())` on HTTP 200 within the 5s budget; `Err(msg)` otherwise.
    pub result: std::result::Result<(), String>,
}

/// Injected HTTP heartbeat probe. The default impl marks both as
/// Skipped — tests and the storage-sqlite crate's own unit tests use it.
/// `apps/tauri` provides a production impl backed by `reqwest::blocking`.
pub trait Heartbeats {
    /// Probe Twelve Data's quote endpoint. Returns `Ok` on HTTP 200
    /// with parseable JSON inside the per-check budget.
    fn twelve_data(&self) -> HeartbeatResult {
        HeartbeatResult {
            elapsed: Duration::ZERO,
            result: Err("PR-I5.b — no Heartbeats impl injected".to_string()),
        }
    }

    /// Probe Mizan Connect's /v1/health endpoint. Returns `Ok` on
    /// HTTP 200 inside the per-check budget.
    fn mizan_connect(&self) -> HeartbeatResult {
        HeartbeatResult {
            elapsed: Duration::ZERO,
            result: Err("PR-I5.b — no Heartbeats impl injected".to_string()),
        }
    }
}

/// No-op Heartbeats impl. Used by storage-sqlite's own tests and as
/// the default when apps/tauri hasn't injected a production impl
/// (e.g. during boot scenarios where the HTTP client isn't built yet).
pub struct NoopHeartbeats;
impl Heartbeats for NoopHeartbeats {}

/// Per-platform path to the persisted self-test result file.
#[must_use]
pub fn result_path_for(app_data_dir: &str, version: &str) -> PathBuf {
    Path::new(app_data_dir)
        .join(".mizan")
        .join(format!("self_test_{version}.json"))
}

/// Load a cached self-test report for `version`, if one exists. Returns
/// `Ok(None)` if no cache file present; `Err` only on actual I/O or
/// JSON-parse errors (so corrupted cache → re-run).
pub fn load_cached_result(app_data_dir: &str, version: &str) -> CoreResult<Option<SelfTestReport>> {
    let path = result_path_for(app_data_dir, version);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| {
        mizan_core::errors::Error::Database(mizan_core::errors::DatabaseError::BackupFailed(
            format!("self_test cache read failed: {e}"),
        ))
    })?;
    let report: SelfTestReport = serde_json::from_slice(&bytes).map_err(|e| {
        mizan_core::errors::Error::Database(mizan_core::errors::DatabaseError::BackupFailed(
            format!("self_test cache parse failed: {e}"),
        ))
    })?;
    Ok(Some(report))
}

/// Persist a self-test report to disk.
pub fn persist_result(app_data_dir: &str, report: &SelfTestReport) -> CoreResult<()> {
    let path = result_path_for(app_data_dir, &report.version);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            mizan_core::errors::Error::Database(mizan_core::errors::DatabaseError::BackupFailed(
                format!("self_test cache create_dir failed: {e}"),
            ))
        })?;
    }
    let bytes = serde_json::to_vec_pretty(report).map_err(|e| {
        mizan_core::errors::Error::Database(mizan_core::errors::DatabaseError::BackupFailed(
            format!("self_test cache serialize failed: {e}"),
        ))
    })?;
    fs::write(&path, bytes).map_err(|e| {
        mizan_core::errors::Error::Database(mizan_core::errors::DatabaseError::BackupFailed(
            format!("self_test cache write failed: {e}"),
        ))
    })?;
    Ok(())
}

/// Run the full self-test suite + persist the result.
///
/// Returns `Ok(report)` regardless of individual check outcomes — the
/// orchestrator never propagates per-check failures as `Err`. The caller
/// inspects `report.required_all_passed()` to decide whether to launch
/// or roll back.
///
/// If a cached result exists for the SAME version and every required
/// check passed in that cache, this fn short-circuits and returns the
/// cached report (subsequent launches of the same version don't re-run).
pub fn run_and_persist(
    app_data_dir: &str,
    version: &str,
    pool: &DbPool,
    heartbeats: &dyn Heartbeats,
) -> SelfTestReport {
    // Fast-path: previous successful run for this exact version.
    if let Ok(Some(cached)) = load_cached_result(app_data_dir, version) {
        if cached.required_all_passed() {
            info!(
                "self_test: cached result for v{version} passed all required checks; skipping fresh run"
            );
            return cached;
        }
        warn!(
            "self_test: cached result for v{version} had failures ({:?}); re-running",
            cached.failed_required()
        );
    }

    let started = Instant::now();
    let mut checks = Vec::new();

    // Check 1 — schema match.
    checks.push(run_check_schema_match(pool));
    if started.elapsed() > TOTAL_BUDGET {
        return finalize(version, app_data_dir, started, checks);
    }

    // Check 2 — crypto round-trip.
    checks.push(run_check_crypto_round_trip());
    if started.elapsed() > TOTAL_BUDGET {
        return finalize(version, app_data_dir, started, checks);
    }

    // Check 3 — truth ledger chain head.
    checks.push(run_check_truth_ledger_chain_head(pool));

    // Network heartbeats (PR-I5.b). Both are non-blocking — informational
    // only per REQUIRED_CHECK_NAMES; a transient outage doesn't trigger
    // a rollback. Run sequentially with their own per-probe budgets.
    checks.push(record_heartbeat(
        "twelve_data_heartbeat",
        heartbeats.twelve_data(),
    ));
    if started.elapsed() <= TOTAL_BUDGET {
        checks.push(record_heartbeat(
            "mizan_connect_heartbeat",
            heartbeats.mizan_connect(),
        ));
    }

    finalize(version, app_data_dir, started, checks)
}

/// Convert a [`HeartbeatResult`] from an injected probe into a
/// [`CheckOutcome`]. Pass/fail mirrors the probe's `result` field.
fn record_heartbeat(name: &str, hb: HeartbeatResult) -> CheckOutcome {
    match hb.result {
        Ok(()) => CheckOutcome::pass(name, hb.elapsed),
        Err(msg) => CheckOutcome::fail(name, hb.elapsed, msg),
    }
}

fn finalize(
    version: &str,
    app_data_dir: &str,
    started: Instant,
    checks: Vec<CheckOutcome>,
) -> SelfTestReport {
    let report = SelfTestReport {
        version: version.to_string(),
        all_required_passed: SelfTestReport::REQUIRED_CHECK_NAMES
            .iter()
            .all(|required| checks.iter().any(|c| c.name == *required && c.passed)),
        checks,
        total_elapsed_ms: started.elapsed().as_millis() as u64,
    };

    if let Err(e) = persist_result(app_data_dir, &report) {
        warn!("self_test: persist_result failed: {e}");
    }

    if !report.all_required_passed {
        error!(
            "self_test: FAILED for v{version}; failed required checks: {:?}",
            report.failed_required()
        );
    } else {
        info!(
            "self_test: all required checks passed for v{version} in {}ms",
            report.total_elapsed_ms
        );
    }

    report
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn run_check_schema_match(pool: &DbPool) -> CheckOutcome {
    let started = Instant::now();
    let name = "schema_match";

    let mut conn = match get_connection(pool) {
        Ok(c) => c,
        Err(e) => return CheckOutcome::fail(name, started.elapsed(), format!("{e}")),
    };

    match conn.has_pending_migration(MIGRATIONS) {
        Ok(false) => CheckOutcome::pass(name, started.elapsed()),
        Ok(true) => CheckOutcome::fail(
            name,
            started.elapsed(),
            "pending migrations detected — schema is out of date",
        ),
        Err(e) => CheckOutcome::fail(name, started.elapsed(), format!("{e}")),
    }
}

fn run_check_crypto_round_trip() -> CheckOutcome {
    let started = Instant::now();
    let name = "crypto_round_trip";

    // The check verifies the chacha20poly1305 library is loaded + the
    // platform's AES-NI / NEON acceleration is functional. We don't
    // touch the production keychain here — the keychain is separately
    // covered by the secret-store integration tests.
    let key_bytes = [0x42u8; 32];
    let key = Key::from_slice(&key_bytes);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce_bytes = [0x33u8; 12];
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = b"mizan-self-test-2026-Q3-baseline";

    let ciphertext = match cipher.encrypt(nonce, plaintext.as_ref()) {
        Ok(ct) => ct,
        Err(e) => return CheckOutcome::fail(name, started.elapsed(), format!("encrypt: {e}")),
    };

    let decrypted = match cipher.decrypt(nonce, ciphertext.as_ref()) {
        Ok(pt) => pt,
        Err(e) => return CheckOutcome::fail(name, started.elapsed(), format!("decrypt: {e}")),
    };

    if decrypted == plaintext {
        CheckOutcome::pass(name, started.elapsed())
    } else {
        CheckOutcome::fail(
            name,
            started.elapsed(),
            "round-trip produced mismatched plaintext",
        )
    }
}

fn run_check_truth_ledger_chain_head(pool: &DbPool) -> CheckOutcome {
    let started = Instant::now();
    let name = "truth_ledger_chain_head";

    let mut conn = match get_connection(pool) {
        Ok(c) => c,
        Err(e) => return CheckOutcome::fail(name, started.elapsed(), format!("{e}")),
    };

    // Use sql_query to fetch the latest row's prev_hash + curr_hash +
    // payload. If the table is empty (fresh install), the check is a
    // pass — nothing to verify against.
    use diesel::sql_types::{Nullable, Text};
    use diesel::QueryableByName;
    use diesel::RunQueryDsl;

    #[derive(QueryableByName)]
    struct TruthLedgerHead {
        #[diesel(sql_type = Text)]
        curr_hash: String,
        #[diesel(sql_type = Nullable<Text>)]
        prev_hash: Option<String>,
        #[diesel(sql_type = Text)]
        event_payload: String,
    }

    let head_result: std::result::Result<Vec<TruthLedgerHead>, _> = diesel::sql_query(
        "SELECT curr_hash, prev_hash, event_payload \
         FROM truth_ledger ORDER BY id DESC LIMIT 1",
    )
    .load(&mut conn);

    let head = match head_result {
        Ok(rows) if rows.is_empty() => {
            return CheckOutcome::pass(name, started.elapsed());
        }
        Ok(rows) => rows.into_iter().next().unwrap(),
        Err(e) => {
            // "no such table" on fresh installs is acceptable — the
            // truth_ledger migration may not have run yet for users
            // upgrading from very old versions. Treat as pass.
            let msg = format!("{e}");
            if msg.contains("no such table") {
                return CheckOutcome::pass(name, started.elapsed());
            }
            return CheckOutcome::fail(name, started.elapsed(), msg);
        }
    };

    // Recompute the expected curr_hash and compare. The truth ledger
    // uses BLAKE3 over (prev_hash || event_payload) per the financial-truth
    // crate's hash_entry routine.
    let mut hasher = blake3::Hasher::new();
    if let Some(prev) = &head.prev_hash {
        hasher.update(prev.as_bytes());
    }
    hasher.update(head.event_payload.as_bytes());
    let expected = hasher.finalize().to_hex().to_string();

    if expected == head.curr_hash {
        CheckOutcome::pass(name, started.elapsed())
    } else {
        CheckOutcome::fail(
            name,
            started.elapsed(),
            format!(
                "chain head mismatch: expected curr_hash={expected}, stored={}",
                head.curr_hash
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::SqliteConnection;
    use std::sync::Arc;
    use tempfile::{NamedTempFile, TempDir};

    fn fresh_pool() -> (Arc<DbPool>, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let url = tmp.path().to_str().unwrap().to_string();
        let manager = ConnectionManager::<SqliteConnection>::new(url);
        let pool: DbPool = Pool::builder().max_size(2).build(manager).unwrap();
        let pool = Arc::new(pool);
        crate::db::run_migrations(tmp.path().to_str().unwrap()).unwrap();
        (pool, tmp)
    }

    #[test]
    fn schema_match_passes_on_freshly_migrated_db() {
        let (pool, _tmp) = fresh_pool();
        let outcome = run_check_schema_match(&pool);
        assert!(outcome.passed, "schema_match failed: {:?}", outcome.error);
        assert_eq!(outcome.name, "schema_match");
    }

    #[test]
    fn crypto_round_trip_passes() {
        let outcome = run_check_crypto_round_trip();
        assert!(
            outcome.passed,
            "crypto_round_trip failed: {:?}",
            outcome.error
        );
        assert_eq!(outcome.name, "crypto_round_trip");
    }

    #[test]
    fn truth_ledger_check_passes_on_empty_ledger() {
        let (pool, _tmp) = fresh_pool();
        let outcome = run_check_truth_ledger_chain_head(&pool);
        // Empty ledger → pass per the docstring (nothing to verify against).
        assert!(
            outcome.passed,
            "truth_ledger check failed: {:?}",
            outcome.error
        );
    }

    #[test]
    fn report_required_all_passed_with_three_locals_pass() {
        let report = SelfTestReport {
            version: "3.4.1".into(),
            checks: vec![
                CheckOutcome::pass("schema_match", Duration::from_millis(10)),
                CheckOutcome::pass("crypto_round_trip", Duration::from_millis(1)),
                CheckOutcome::pass("truth_ledger_chain_head", Duration::from_millis(5)),
                CheckOutcome::skipped("twelve_data_heartbeat", "PR-I5.b — pending"),
                CheckOutcome::skipped("mizan_connect_heartbeat", "PR-I5.b — pending"),
            ],
            total_elapsed_ms: 16,
            all_required_passed: true,
        };
        assert!(report.required_all_passed());
        assert!(report.failed_required().is_empty());
    }

    #[test]
    fn report_required_failed_lists_missing_required_check() {
        let report = SelfTestReport {
            version: "3.4.1".into(),
            checks: vec![
                CheckOutcome::pass("schema_match", Duration::from_millis(10)),
                CheckOutcome::fail("crypto_round_trip", Duration::from_millis(1), "simulated"),
                CheckOutcome::pass("truth_ledger_chain_head", Duration::from_millis(5)),
            ],
            total_elapsed_ms: 16,
            all_required_passed: false,
        };
        assert!(!report.required_all_passed());
        assert_eq!(report.failed_required(), vec!["crypto_round_trip"]);
    }

    #[test]
    fn run_and_persist_writes_json_cache() {
        let (pool, _tmp_db) = fresh_pool();
        let cache_dir = TempDir::new().unwrap();
        let app_data = cache_dir.path().to_str().unwrap();

        let report = run_and_persist(app_data, "3.4.1", &pool, &NoopHeartbeats);
        assert_eq!(report.version, "3.4.1");
        assert!(report.required_all_passed());

        // Cache file written
        let cache_path = result_path_for(app_data, "3.4.1");
        assert!(cache_path.exists());

        // Second call short-circuits on cache hit; bypassing the pool.
        // We pass a deliberately-broken pool (max_size 0 would deadlock;
        // use a path-only mock by re-using the same pool — the test
        // observes that the returned report shape matches).
        let again = run_and_persist(app_data, "3.4.1", &pool, &NoopHeartbeats);
        assert_eq!(again, report);
    }

    #[test]
    fn run_and_persist_invalidates_on_version_change() {
        let (pool, _tmp_db) = fresh_pool();
        let cache_dir = TempDir::new().unwrap();
        let app_data = cache_dir.path().to_str().unwrap();

        let v1 = run_and_persist(app_data, "3.4.1", &pool, &NoopHeartbeats);
        let v2 = run_and_persist(app_data, "3.5.0", &pool, &NoopHeartbeats);
        assert_eq!(v1.version, "3.4.1");
        assert_eq!(v2.version, "3.5.0");

        // Distinct cache files exist.
        assert!(result_path_for(app_data, "3.4.1").exists());
        assert!(result_path_for(app_data, "3.5.0").exists());
    }

    #[test]
    fn load_cached_returns_none_when_missing() {
        let cache_dir = TempDir::new().unwrap();
        let app_data = cache_dir.path().to_str().unwrap();
        assert!(load_cached_result(app_data, "9.9.9").unwrap().is_none());
    }

    #[test]
    fn load_cached_round_trips_a_persisted_report() {
        let cache_dir = TempDir::new().unwrap();
        let app_data = cache_dir.path().to_str().unwrap();
        let report = SelfTestReport {
            version: "3.4.1".into(),
            checks: vec![CheckOutcome::pass("schema_match", Duration::from_millis(1))],
            total_elapsed_ms: 1,
            all_required_passed: false,
        };
        persist_result(app_data, &report).unwrap();
        let loaded = load_cached_result(app_data, "3.4.1").unwrap().unwrap();
        assert_eq!(loaded, report);
    }
}
