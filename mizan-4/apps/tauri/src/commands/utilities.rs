use chrono;
use mizan_storage_sqlite::db;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tauri::Manager;
use tauri::{AppHandle, Emitter};

use crate::context::ServiceContext;
#[cfg(desktop)]
use crate::updater::{check_for_update, install_update};

/// Normalize file path by removing file:// URI prefix if present (iOS/Android compatibility)
fn normalize_file_path(path: &str) -> String {
    if path.starts_with("file://") {
        path.strip_prefix("file://").unwrap_or(path).to_string()
    } else {
        path.to_string()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    db_path: String,
    logs_dir: String,
}

#[tauri::command]
pub async fn get_app_info(app_handle: AppHandle) -> Result<AppInfo, String> {
    let version = app_handle.package_info().version.to_string();

    let app_data_dir_path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .to_path_buf();

    let app_data_dir = app_data_dir_path
        .to_str()
        .ok_or_else(|| "Failed to convert app data dir path to string".to_string())?
        .to_string();

    let db_path = db::get_db_path(&app_data_dir);
    let logs_dir = app_handle
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to get app log dir: {}", e))?
        .to_str()
        .ok_or_else(|| "Failed to convert app log dir path to string".to_string())?
        .to_string();

    Ok(AppInfo {
        version,
        db_path,
        logs_dir,
    })
}

/// Check for updates and return update info if available.
#[tauri::command]
pub async fn check_for_updates(app_handle: AppHandle) -> Result<Option<serde_json::Value>, String> {
    #[cfg(desktop)]
    {
        let instance_id = app_handle
            .try_state::<std::sync::Arc<ServiceContext>>()
            .map(|state| state.instance_id.clone())
            .ok_or_else(|| "Failed to access service context".to_string())?;

        let result = check_for_update(app_handle, &instance_id).await?;
        Ok(result.map(|info| serde_json::to_value(info).unwrap()))
    }
    #[cfg(not(desktop))]
    {
        Ok(None)
    }
}

/// Download and install an available update. Emits progress events and restarts the app.
#[tauri::command]
pub async fn install_app_update(app_handle: AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    install_update(app_handle).await?;
    Ok(())
}

#[tauri::command]
pub async fn backup_database(app_handle: AppHandle) -> Result<(String, Vec<u8>), String> {
    // .expect() previously panicked the entire Tauri runtime if the
    // OS refused the app-data-dir lookup or returned a non-UTF-8 path
    // (rare on macOS, more common on locked-down Linux distros). Now
    // returns a real error so the frontend can surface "couldn't read
    // app data" instead of the user seeing the whole window crash.
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .to_str()
        .ok_or_else(|| "App data dir path is not valid UTF-8".to_string())?
        .to_string();

    let backup_path = db::backup_database(&app_data_dir).map_err(|e| e.to_string())?;

    // Read the backup file
    let mut file =
        File::open(&backup_path).map_err(|e| format!("Failed to open backup file: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read backup file: {}", e))?;

    // Get the filename
    let filename = Path::new(&backup_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Failed to get backup filename".to_string())?
        .to_string();

    Ok((filename, buffer))
}

/// Encrypted user-initiated backup. Writes a passphrase-protected
/// envelope to disk via [`crate::backup_crypto::encrypt_backup`].
///
/// The plaintext SQLite contains broker OAuth tokens, AI API keys,
/// and the entire activity log; users routinely re-upload backups to
/// Drive / iCloud / Dropbox / personal NAS where breach-at-provider
/// is a real risk. This command is what the Settings → Backup button
/// should call going forward. The legacy `backup_database_to_path`
/// remains for backwards compatibility but should be removed once
/// frontend callers are migrated.
#[tauri::command]
pub async fn backup_database_to_path_encrypted(
    app_handle: AppHandle,
    backup_dir: String,
    passphrase: String,
) -> Result<String, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .to_str()
        .ok_or_else(|| "App data dir path is not valid UTF-8".to_string())?
        .to_string();

    let normalized_backup_dir = normalize_file_path(&backup_dir);
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("mizan_backup_{}.mzbkp", timestamp);
    let backup_path = Path::new(&normalized_backup_dir).join(&backup_filename);
    let backup_path_str = backup_path.to_string_lossy().to_string();

    // Stage 1: write a plaintext SQLite snapshot to a temp file. We
    // can't encrypt the live DB in-memory because the SQLite VACUUM
    // INTO that `backup_database_to_file` invokes wants a real path.
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("mizan_backup_tmp_{}.db", timestamp));
    let tmp_path_str = tmp_path.to_string_lossy().to_string();
    db::backup_database_to_file(&app_data_dir, &tmp_path_str)
        .map_err(|e| format!("Failed to snapshot database for encryption: {}", e))?;

    // Stage 2: read the snapshot, encrypt, write envelope, scrub.
    let plaintext = match std::fs::read(&tmp_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("Failed to read snapshot for encryption: {}", e));
        }
    };

    let envelope = match crate::backup_crypto::encrypt_backup(&plaintext, &passphrase) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("Encryption failed: {}", e));
        }
    };

    // Best-effort scrub of the temp file before writing the output.
    let _ = std::fs::remove_file(&tmp_path);

    std::fs::write(&backup_path, &envelope)
        .map_err(|e| format!("Failed to write encrypted backup: {}", e))?;

    // Tighten file permissions on the encrypted backup so it's not
    // world-readable on multi-user systems even though it's encrypted.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&backup_path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(backup_path_str)
}

#[tauri::command]
pub async fn backup_database_to_path(
    app_handle: AppHandle,
    backup_dir: String,
) -> Result<String, String> {
    // .expect() previously panicked the entire Tauri runtime if the
    // OS refused the app-data-dir lookup or returned a non-UTF-8 path
    // (rare on macOS, more common on locked-down Linux distros). Now
    // returns a real error so the frontend can surface "couldn't read
    // app data" instead of the user seeing the whole window crash.
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .to_str()
        .ok_or_else(|| "App data dir path is not valid UTF-8".to_string())?
        .to_string();

    // Normalize the backup directory path (remove file:// prefix if present on iOS/Android)
    let normalized_backup_dir = normalize_file_path(&backup_dir);

    // Create a custom backup path in the specified directory
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("mizan_backup_{}.db", timestamp);
    let backup_path = Path::new(&normalized_backup_dir).join(&backup_filename);

    let backup_path_str = backup_path.to_string_lossy().to_string();

    db::backup_database_to_file(&app_data_dir, &backup_path_str)
        .map_err(|e| format!("Failed to backup database: {}", e))?;

    Ok(backup_path_str)
}

/// Restore a backup file.
///
/// `passphrase` is optional for backwards compatibility — legacy
/// unencrypted backups (raw SQLite bytes) restore without one. New
/// `.mzbkp` envelopes (per `backup_crypto.rs`) auto-detect via magic
/// header and require the passphrase to decrypt.
///
/// If the file is encrypted but no passphrase is supplied, we
/// return a clear error string so the frontend can prompt the user.
#[tauri::command]
pub async fn restore_database(
    app_handle: AppHandle,
    backup_file_path: String,
    passphrase: Option<String>,
) -> Result<(), String> {
    // .expect() previously panicked the entire Tauri runtime if the
    // OS refused the app-data-dir lookup or returned a non-UTF-8 path
    // (rare on macOS, more common on locked-down Linux distros). Now
    // returns a real error so the frontend can surface "couldn't read
    // app data" instead of the user seeing the whole window crash.
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .to_str()
        .ok_or_else(|| "App data dir path is not valid UTF-8".to_string())?
        .to_string();

    // Normalize the backup file path (remove file:// prefix if present on iOS/Android)
    let normalized_backup_path = normalize_file_path(&backup_file_path);

    // Try to get the ServiceContext to perform graceful operations before restore
    if app_handle
        .try_state::<std::sync::Arc<crate::context::ServiceContext>>()
        .is_some()
    {
        // Give some time for any pending operations to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Sniff the file header: if it's a Mizan encrypted envelope, write
    // the decrypted SQLite bytes to a tmp file and restore from THAT.
    // Otherwise restore from the supplied path directly (legacy
    // unencrypted backup path).
    let header = {
        use std::io::Read;
        let mut f = std::fs::File::open(&normalized_backup_path)
            .map_err(|e| format!("Failed to open backup file: {}", e))?;
        let mut buf = vec![0u8; 16];
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("Failed to read backup header: {}", e))?;
        buf.truncate(n);
        buf
    };

    let restore_source_path: String = if crate::backup_crypto::is_encrypted_envelope(&header) {
        let passphrase = passphrase.ok_or_else(|| {
            "Encrypted backup detected — passphrase required to restore.".to_string()
        })?;
        let envelope_bytes = std::fs::read(&normalized_backup_path)
            .map_err(|e| format!("Failed to read encrypted backup: {}", e))?;
        let plaintext = crate::backup_crypto::decrypt_backup(&envelope_bytes, &passphrase)
            .map_err(|e| e.to_string())?;

        let tmp = std::env::temp_dir().join(format!(
            "mizan_restore_tmp_{}.db",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ));
        std::fs::write(&tmp, &plaintext)
            .map_err(|e| format!("Failed to stage decrypted backup: {}", e))?;
        // Scrub plaintext from memory ASAP — the temp file persists
        // only until restore_database_safe completes.
        drop(plaintext);
        tmp.to_string_lossy().to_string()
    } else {
        normalized_backup_path.clone()
    };

    // Use the safe restore function that handles Windows file locking issues
    let restore_result = db::restore_database_safe(&app_data_dir, &restore_source_path);

    // Best-effort scrub of any staged decrypted bytes regardless of
    // restore outcome.
    if restore_source_path != normalized_backup_path {
        let _ = std::fs::remove_file(&restore_source_path);
    }

    restore_result.map_err(|e| e.to_string())?;

    // After successful restore, emit event and show restart dialog
    app_handle
        .emit("database-restored", ())
        .map_err(|e| format!("Failed to emit database-restored event: {}", e))?;

    // On desktop builds prompt for restart, but skip showing dialogs on iOS/Android
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

        let should_restart = app_handle
            .dialog()
            .message(
                "Database restored successfully!\n\n\
                 For the best experience, it's recommended to restart the application \
                 to ensure all data is properly refreshed.\n\n\
                 Would you like to restart now?",
            )
            .title("Database Restored - Restart Required")
            .buttons(MessageDialogButtons::OkCancel)
            .kind(MessageDialogKind::Info)
            .blocking_show();

        if should_restart {
            app_handle.restart();
        }
    }

    Ok(())
}
