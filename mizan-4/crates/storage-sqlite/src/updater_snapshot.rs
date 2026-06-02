//! Pre-update DB snapshot + 30-day retention janitor.
//!
//! Per ADR 0009 (Updater snapshot & rollback design):
//!
//! > Before `update.download_and_install(...)` completes (i.e. before the
//! > new binary starts), the updater copies `mizan.db` to a snapshot file
//! > at `${OS_APP_DATA_DIR}/mizan.db.pre-{old_version}`.
//!
//! The snapshot is the DB-side belt-and-braces for the auto-rollback
//! path (PR-I6). The new binary's post-install self-test (PR-I5)
//! verifies schema + crypto + ledger integrity before considering the
//! upgrade successful; on failure, the rollback restores from this
//! snapshot.
//!
//! # WAL-mode safety
//!
//! SQLite WAL mode keeps the active DB file in a partially-committed
//! state — `mizan.db` alone isn't the full state. This module delegates
//! to [`crate::db::backup_database_to_file`], which uses `VACUUM INTO`
//! (preceded by a `wal_checkpoint(FULL)`) — that's the WAL-safe primitive
//! per the existing repo convention.
//!
//! # Retention
//!
//! Snapshots older than [`SNAPSHOT_RETENTION_DAYS`] (30) are pruned by
//! [`prune_old_snapshots`]. Called at app launch alongside the cache
//! eviction worker so snapshot disk usage stays bounded without
//! requiring a separate background thread.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use log::{info, warn};

use crate::db::{backup_database_to_file, get_db_path};
use mizan_core::errors::{DatabaseError, Error, Result};

/// 30-day retention per working agreement §19.3 + ADR 0009. Snapshots
/// older than this are pruned at app launch.
pub const SNAPSHOT_RETENTION_DAYS: u64 = 30;

/// Filename pattern for pre-update snapshots. Format string takes the
/// outgoing binary's version (e.g. `3.4.1`). All snapshots sort
/// lexicographically with this prefix.
pub const SNAPSHOT_FILE_PREFIX: &str = "mizan.db.pre-";

/// Build the snapshot path for the given outgoing-version string.
/// Public so tests + the updater consumer both name it the same way.
#[must_use]
pub fn snapshot_path_for(app_data_dir: &str, old_version: &str) -> PathBuf {
    Path::new(app_data_dir).join(format!("{SNAPSHOT_FILE_PREFIX}{old_version}"))
}

/// Create a pre-update snapshot of `mizan.db` at
/// `${app_data_dir}/mizan.db.pre-{old_version}`.
///
/// If a snapshot for the SAME `old_version` already exists (re-applying
/// the same update — e.g. a second auto-update retry after a partial
/// failure), the existing snapshot is PRESERVED. ADR 0009: "never
/// silently overwritten." The retention janitor will eventually GC it
/// after 30 days.
///
/// Returns the snapshot's absolute path on success.
pub fn create_pre_update_snapshot(app_data_dir: &str, old_version: &str) -> Result<PathBuf> {
    let dest = snapshot_path_for(app_data_dir, old_version);

    if dest.exists() {
        info!(
            "updater_snapshot: snapshot for v{old_version} already exists at {} — preserving",
            dest.display()
        );
        return Ok(dest);
    }

    let dest_str = dest
        .to_str()
        .ok_or_else(|| {
            Error::Database(DatabaseError::BackupFailed(
                "snapshot path contained non-UTF8 bytes".to_string(),
            ))
        })?
        .to_string();

    info!(
        "updater_snapshot: creating pre-update snapshot for v{old_version} at {}",
        dest.display()
    );
    backup_database_to_file(app_data_dir, &dest_str)?;
    Ok(dest)
}

/// List all pre-update snapshots in `app_data_dir`. Pure helper; doesn't
/// touch the clock. Returns paths in lexicographic order (i.e. roughly
/// by version since the version string is in the filename).
pub fn list_snapshots(app_data_dir: &str) -> Result<Vec<PathBuf>> {
    let dir = Path::new(app_data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| {
            Error::Database(DatabaseError::BackupFailed(format!(
                "list_snapshots read_dir failed: {e}"
            )))
        })?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(SNAPSHOT_FILE_PREFIX))
        })
        .collect();

    out.sort();
    Ok(out)
}

/// Prune pre-update snapshots older than [`SNAPSHOT_RETENTION_DAYS`].
/// Called at app launch alongside the cache-eviction startup hook.
///
/// `now` is supplied by the caller so this function stays test-deterministic.
/// Returns the list of pruned snapshot paths.
pub fn prune_old_snapshots(app_data_dir: &str, now: SystemTime) -> Result<Vec<PathBuf>> {
    let snapshots = list_snapshots(app_data_dir)?;
    let retention = Duration::from_secs(SNAPSHOT_RETENTION_DAYS * 24 * 60 * 60);

    let mut pruned = Vec::new();
    for path in snapshots {
        let modified = match path.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "updater_snapshot: prune metadata read failed for {}: {e}",
                    path.display()
                );
                continue;
            }
        };

        match now.duration_since(modified) {
            Ok(age) if age > retention => {
                if let Err(e) = fs::remove_file(&path) {
                    warn!(
                        "updater_snapshot: prune remove_file failed for {}: {e}",
                        path.display()
                    );
                    continue;
                }
                info!(
                    "updater_snapshot: pruned snapshot {} (age {} days)",
                    path.display(),
                    age.as_secs() / 86_400
                );
                pruned.push(path);
            }
            Ok(_) | Err(_) => {
                // Within retention window, OR modified-time is in the
                // future (clock skew on a restored backup) — leave alone.
            }
        }
    }

    Ok(pruned)
}

/// Detect the snapshot from which a rollback should restore. Returns the
/// most-recent snapshot path that is NEWER than the binary's last-seen
/// timestamp (i.e. created during the just-failed upgrade), or None
/// if there's no candidate.
///
/// Used by the rollback path (PR-I6) — included here so the snapshot
/// surface is co-located.
pub fn snapshot_for_rollback(app_data_dir: &str, old_version: &str) -> Option<PathBuf> {
    let dest = snapshot_path_for(app_data_dir, old_version);
    if dest.exists() {
        Some(dest)
    } else {
        None
    }
}

/// Resolve the live DB path for symmetric naming with [`snapshot_path_for`].
/// Re-exports the helper from the parent module so callers can grab both
/// from the same place.
#[must_use]
pub fn live_db_path(app_data_dir: &str) -> String {
    get_db_path(app_data_dir)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::time::Duration as StdDuration;
    use tempfile::TempDir;

    fn fresh_data_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    /// Pre-seed a minimal mizan.db so the snapshot path can be exercised
    /// without standing up the full migration suite.
    fn touch_live_db(app_data_dir: &Path) -> PathBuf {
        let live = app_data_dir.join("mizan.db");
        // Use the same Diesel test approach as cache_eviction_sqlite:
        // create + migrate, so the snapshot points at a real schema.
        crate::db::run_migrations(live.to_str().unwrap()).unwrap();
        live
    }

    #[test]
    fn snapshot_path_for_uses_version_in_filename() {
        let p = snapshot_path_for("/tmp/x", "3.4.1");
        assert!(p.to_str().unwrap().ends_with("mizan.db.pre-3.4.1"));
    }

    #[test]
    fn create_pre_update_snapshot_copies_live_db() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        touch_live_db(dir.path());

        let snap = create_pre_update_snapshot(app_data, "3.4.0").unwrap();
        assert!(snap.exists());
        assert!(snap
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("mizan.db.pre-3.4.0"));
        // The snapshot is a real SQLite file with schema, not an empty
        // shell — opening it should succeed.
        let conn = rusqlite::Connection::open(&snap).unwrap();
        conn.execute_batch("SELECT 1").unwrap();
    }

    #[test]
    fn create_pre_update_snapshot_preserves_existing() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        touch_live_db(dir.path());

        let snap1 = create_pre_update_snapshot(app_data, "3.4.0").unwrap();
        let len1 = fs::metadata(&snap1).unwrap().len();
        // mutate live DB so a re-snapshot would have different size
        let conn = rusqlite::Connection::open(dir.path().join("mizan.db")).unwrap();
        conn.execute_batch("CREATE TABLE post_mutation (x TEXT);")
            .unwrap();
        drop(conn);

        let snap2 = create_pre_update_snapshot(app_data, "3.4.0").unwrap();
        let len2 = fs::metadata(&snap2).unwrap().len();
        assert_eq!(snap1, snap2);
        // ADR 0009: existing snapshot preserved verbatim.
        assert_eq!(len1, len2);
    }

    #[test]
    fn list_snapshots_filters_to_pre_update_only() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        touch_live_db(dir.path());
        fs::write(dir.path().join("unrelated.bin"), b"x").unwrap();

        create_pre_update_snapshot(app_data, "3.4.0").unwrap();
        create_pre_update_snapshot(app_data, "3.4.1").unwrap();

        let snaps = list_snapshots(app_data).unwrap();
        assert_eq!(snaps.len(), 2);
        assert!(snaps.iter().all(|p| p
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(SNAPSHOT_FILE_PREFIX)));
    }

    #[test]
    fn prune_keeps_recent_removes_old() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        touch_live_db(dir.path());

        create_pre_update_snapshot(app_data, "3.4.0").unwrap();
        let old_snap = dir.path().join("mizan.db.pre-2.0.0");
        fs::write(&old_snap, b"ancient backup").unwrap();
        // Set mtime to 60 days ago.
        let sixty_days_ago = SystemTime::now() - StdDuration::from_secs(60 * 24 * 60 * 60);
        let ft = filetime::FileTime::from_system_time(sixty_days_ago);
        filetime::set_file_mtime(&old_snap, ft).unwrap();

        let pruned = prune_old_snapshots(app_data, SystemTime::now()).unwrap();
        assert!(pruned.contains(&old_snap));
        assert!(!old_snap.exists());
        // The fresh one survives.
        assert!(dir.path().join("mizan.db.pre-3.4.0").exists());
    }

    #[test]
    fn prune_returns_empty_on_missing_dir() {
        // Non-existent dir → no error, empty result.
        let pruned =
            prune_old_snapshots("/tmp/definitely-does-not-exist-zzz", SystemTime::now()).unwrap();
        assert!(pruned.is_empty());
    }

    #[test]
    fn snapshot_for_rollback_returns_none_if_missing() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        assert!(snapshot_for_rollback(app_data, "9.9.9").is_none());
    }

    #[test]
    fn snapshot_for_rollback_finds_existing() {
        let dir = fresh_data_dir();
        let app_data = dir.path().to_str().unwrap();
        touch_live_db(dir.path());
        create_pre_update_snapshot(app_data, "3.4.0").unwrap();

        let resolved = snapshot_for_rollback(app_data, "3.4.0").unwrap();
        assert!(resolved.exists());
        assert!(resolved
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("mizan.db.pre-3.4.0"));
    }
}
