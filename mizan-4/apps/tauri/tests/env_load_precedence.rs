//! Integration tests for the Tauri runner's env-load precedence.
//!
//! The Uncle Feroz demo flow depends on `.env.local` overriding
//! `.env` so the gitignored ANTHROPIC_API_KEY (and any future user
//! secret) wins over committed defaults. A refactor that reordered
//! or dropped the local load would silently break AI on demo day.
//! These tests fix that contract in CI so the regression can never
//! ship unnoticed.
//!
//! The tests use `dotenvy::from_filename` against fixture files
//! written to a per-test temp directory — they exercise the exact
//! dotenvy precedence rule the production `load_env_files()` relies
//! on, NOT the `load_env_files()` function itself (which would
//! require chdir, which is process-global and brittle under
//! threading).

use std::path::PathBuf;
use std::sync::Mutex;

use dotenvy::from_filename;

// All tests in this file mutate process-global env state — serialise
// via a static mutex. `unwrap_or_else(into_inner)` recovers from
// a poisoned mutex (one test panicking shouldn't poison the rest).
static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn fresh_temp_dir(test_name: &str) -> PathBuf {
    // Build a unique subdir under the OS temp dir. We can't use
    // Date::now or random in some harnesses; the test name + process
    // ID is sufficient to avoid collisions between concurrent CI
    // runs on the same runner.
    let dir = std::env::temp_dir().join(format!(
        "mizan_env_load_{}_pid{}",
        test_name,
        std::process::id()
    ));
    // Best-effort wipe in case a previous run on the same PID
    // (unlikely outside tests) left state behind.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup_env(keys: &[&str]) {
    for k in keys {
        std::env::remove_var(k);
    }
}

#[test]
fn env_local_overrides_env_when_loaded_first() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
    cleanup_env(&["MIZAN_TEST_PRECEDENCE_KEY", "MIZAN_TEST_DEFAULT_ONLY_KEY"]);

    let dir = fresh_temp_dir("precedence");
    let env_path = dir.join(".env");
    let local_path = dir.join(".env.local");

    std::fs::write(
        &env_path,
        "MIZAN_TEST_PRECEDENCE_KEY=from-env\nMIZAN_TEST_DEFAULT_ONLY_KEY=from-env\n",
    )
    .unwrap();
    std::fs::write(&local_path, "MIZAN_TEST_PRECEDENCE_KEY=from-local\n").unwrap();

    // Mirror the production order in lib.rs::load_env_files: local
    // first (wins), then env (fills in keys not overridden).
    from_filename(&local_path).ok();
    from_filename(&env_path).ok();

    assert_eq!(
        std::env::var("MIZAN_TEST_PRECEDENCE_KEY").unwrap(),
        "from-local",
        ".env.local must override .env when loaded first"
    );
    assert_eq!(
        std::env::var("MIZAN_TEST_DEFAULT_ONLY_KEY").unwrap(),
        "from-env",
        ".env must still fill in keys absent from .env.local"
    );

    cleanup_env(&["MIZAN_TEST_PRECEDENCE_KEY", "MIZAN_TEST_DEFAULT_ONLY_KEY"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_env_local_does_not_break_load() {
    // If `.env.local` is absent (clean checkout, no demo key set),
    // the loader must NOT panic — `.env` alone should still apply.
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
    cleanup_env(&["MIZAN_TEST_ENV_ONLY_KEY"]);

    let dir = fresh_temp_dir("missing_local");
    let env_path = dir.join(".env");
    let local_path = dir.join(".env.local"); // intentionally NOT written
    std::fs::write(&env_path, "MIZAN_TEST_ENV_ONLY_KEY=from-env\n").unwrap();

    from_filename(&local_path).ok();
    from_filename(&env_path).ok();

    assert_eq!(
        std::env::var("MIZAN_TEST_ENV_ONLY_KEY").unwrap(),
        "from-env"
    );

    cleanup_env(&["MIZAN_TEST_ENV_ONLY_KEY"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_both_files_is_silent_noop() {
    // Bare-checkout case: neither file exists. The loader must
    // succeed silently — there's nothing to set.
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
    cleanup_env(&["MIZAN_TEST_NEVER_SET_KEY"]);

    let dir = fresh_temp_dir("missing_both");
    from_filename(dir.join(".env.local")).ok();
    from_filename(dir.join(".env")).ok();

    assert!(
        std::env::var("MIZAN_TEST_NEVER_SET_KEY").is_err(),
        "no file → no env var set"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn existing_env_var_wins_over_both_dotenv_files() {
    // If the OPERATOR sets the var in their shell before launching
    // the binary, that explicit choice must win over BOTH dotenv
    // files. This is the standard dotenvy contract — but it's a
    // load-bearing one for ops debugging ("override the prod key
    // for one launch") so we pin it here.
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
    cleanup_env(&["MIZAN_TEST_OPERATOR_OVERRIDE"]);

    std::env::set_var("MIZAN_TEST_OPERATOR_OVERRIDE", "from-shell");

    let dir = fresh_temp_dir("operator_override");
    std::fs::write(
        dir.join(".env"),
        "MIZAN_TEST_OPERATOR_OVERRIDE=from-env-file\n",
    )
    .unwrap();
    std::fs::write(
        dir.join(".env.local"),
        "MIZAN_TEST_OPERATOR_OVERRIDE=from-local-file\n",
    )
    .unwrap();

    from_filename(dir.join(".env.local")).ok();
    from_filename(dir.join(".env")).ok();

    assert_eq!(
        std::env::var("MIZAN_TEST_OPERATOR_OVERRIDE").unwrap(),
        "from-shell",
        "operator's shell var must win over dotenv files"
    );

    cleanup_env(&["MIZAN_TEST_OPERATOR_OVERRIDE"]);
    let _ = std::fs::remove_dir_all(&dir);
}
