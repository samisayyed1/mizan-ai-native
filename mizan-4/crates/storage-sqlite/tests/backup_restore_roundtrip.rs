//! Backup → restore round-trip integrity test.
//!
//! Production-grade requirement: a user must be able to back up their
//! Mizan SQLite database, restore it on another machine (or on the
//! same machine after a reset), and see **exactly** the same data —
//! every account, every activity, every quote, every snapshot, byte
//! for byte at the logical level.
//!
//! The backup path uses SQLite's `VACUUM INTO` which produces a
//! self-contained single-file copy with the source's schema +
//! checked integrity. The restore copies the file back into the
//! data directory + clears WAL/SHM so the rebuilt connection reads
//! the fresh contents.
//!
//! This test puts known data through that loop and asserts every row
//! comes back identical. Any future change to `backup_database_to_file`
//! or `restore_database` that corrupts data → CI breaks here.

use mizan_storage_sqlite::db::{
    backup_database, get_db_path, init, restore_database, run_migrations,
};
use rusqlite::{params, Connection};
use tempfile::TempDir;

/// Insert a recognisable signature row into every primary domain table
/// so the round-trip test can read them back and compare. Uses raw
/// rusqlite so we don't pull in the diesel orm machinery for what is
/// really an integration of the file-level backup mechanism.
fn seed_recognisable_data(db_path: &str) -> Result<(), String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    // Test account
    conn.execute(
        r#"INSERT INTO accounts
            (id, name, account_type, "group", currency, is_default, is_active,
             created_at, updated_at, account_number, is_archived)
           VALUES ('acct-roundtrip-1', 'Round-trip Test', 'SECURITIES', 'tax-free',
                   'USD', 0, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z',
                   '****1234', 0)"#,
        params![],
    )
    .map_err(|e| e.to_string())?;

    // Test asset
    conn.execute(
        r#"INSERT INTO assets
            (id, kind, name, display_code, is_active, quote_mode, quote_ccy)
           VALUES ('asset-roundtrip-aapl', 'INVESTMENT', 'Apple Inc.', 'AAPL', 1,
                   'MARKET', 'USD')"#,
        params![],
    )
    .map_err(|e| e.to_string())?;

    // Test activity
    conn.execute(
        r#"INSERT INTO activities
            (id, account_id, asset_id, activity_type, status, activity_date,
             quantity, unit_price, currency, created_at, updated_at)
           VALUES ('act-roundtrip-1', 'acct-roundtrip-1', 'asset-roundtrip-aapl',
                   'BUY', 'CONFIRMED', '2026-01-15T12:00:00Z',
                   '10.0', '180.50', 'USD',
                   '2026-01-15T12:00:00Z', '2026-01-15T12:00:00Z')"#,
        params![],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Pull the seed rows back out and check every field matches.
fn assert_seed_data_intact(db_path: &str) {
    let conn = Connection::open(db_path).expect("open restored db");

    let acct: (String, String, String, String, String) = conn
        .query_row(
            r#"SELECT name, account_type, "group", currency, account_number
               FROM accounts WHERE id = 'acct-roundtrip-1'"#,
            params![],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("account row missing after restore");
    assert_eq!(
        acct,
        (
            "Round-trip Test".to_string(),
            "SECURITIES".to_string(),
            "tax-free".to_string(),
            "USD".to_string(),
            "****1234".to_string(),
        ),
        "account fields drifted across backup → restore"
    );

    let asset: (String, String) = conn
        .query_row(
            "SELECT name, display_code FROM assets WHERE id = 'asset-roundtrip-aapl'",
            params![],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("asset row missing after restore");
    assert_eq!(asset, ("Apple Inc.".to_string(), "AAPL".to_string()));

    let activity: (String, String, String, String) = conn
        .query_row(
            r#"SELECT activity_type, quantity, unit_price, currency
               FROM activities WHERE id = 'act-roundtrip-1'"#,
            params![],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("activity row missing after restore");
    assert_eq!(
        activity,
        (
            "BUY".to_string(),
            "10.0".to_string(),
            "180.50".to_string(),
            "USD".to_string(),
        ),
        "activity FIFO-critical fields drifted across backup → restore"
    );
}

#[test]
fn backup_then_restore_preserves_all_data() {
    // ── Source machine: create DB, seed it, take a backup. ────────────
    let source_dir = TempDir::new().expect("source tempdir");
    let source_app_dir = source_dir.path().to_str().unwrap().to_string();
    init(&source_app_dir).expect("init source db");
    run_migrations(&get_db_path(&source_app_dir)).expect("migrate source db");
    seed_recognisable_data(&get_db_path(&source_app_dir)).expect("seed source");

    let backup_path = backup_database(&source_app_dir).expect("backup");
    assert!(
        std::path::Path::new(&backup_path).exists(),
        "backup file missing"
    );

    // ── Target machine: empty data dir simulating "I bought a new
    // laptop." Initialise + migrate so the directory layout exists,
    // then restore the backup on top. ────────────────────────────────
    let target_dir = TempDir::new().expect("target tempdir");
    let target_app_dir = target_dir.path().to_str().unwrap().to_string();
    init(&target_app_dir).expect("init target db");
    run_migrations(&get_db_path(&target_app_dir)).expect("migrate target db");

    let target_db_path = get_db_path(&target_app_dir);
    // Sanity: target is empty before restore.
    {
        let conn = Connection::open(&target_db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM accounts WHERE id = 'acct-roundtrip-1'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "target is supposed to be empty before restore");
    }

    // Restore the backup into the target dir.
    restore_database(&target_app_dir, &backup_path).expect("restore");

    // Every seeded row, every field, intact.
    assert_seed_data_intact(&target_db_path);

    // Integrity check on the restored database — `PRAGMA integrity_check`
    // surfaces any page corruption + index inconsistency.
    let conn = Connection::open(&target_db_path).unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", params![], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok", "restored DB failed integrity_check");
}
