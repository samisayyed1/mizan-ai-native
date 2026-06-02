#[cfg(not(debug_assertions))]
use chrono::DateTime;
use log::{error, info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

// Helper function to detect if this is an App Store build.
// Only used in the release-build update-check path (see `check_for_update`'s
// `cfg(not(debug_assertions))` branch); marked `cfg(not(debug_assertions))` so
// debug builds don't fire the dead_code warning.
#[cfg(not(debug_assertions))]
fn is_app_store_build() -> bool {
    cfg!(feature = "appstore")
}

// Helper function to retrieve platform-specific store URLs.
// Release-build only — see comment on `is_app_store_build`.
#[cfg(not(debug_assertions))]
fn app_store_url() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        Some("macappstore://apps.apple.com/app/6732888445")
    }

    #[cfg(target_os = "windows")]
    {
        Some("ms-windows-store://pdp/?productid=YOUR_PRODUCT_ID")
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub is_app_store_build: bool,
    pub store_url: Option<String>,
    pub changelog_url: Option<String>,
    pub screenshots: Option<Vec<String>>,
}

/// Extract changelog_url from raw_json.
/// Release-build only — debug builds short-circuit `check_for_update` so this
/// helper is never reached. `cfg` gating keeps debug builds warning-free.
#[cfg(not(debug_assertions))]
fn extract_changelog_url(raw_json: &serde_json::Value) -> Option<String> {
    raw_json
        .get("changelog_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract screenshots from raw_json. Release-build only.
#[cfg(not(debug_assertions))]
fn extract_screenshots(raw_json: &serde_json::Value) -> Option<Vec<String>> {
    raw_json.get("screenshots").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
    })
}

/// Check for updates and return update info if available.
/// Returns `Ok(Some(UpdateInfo))` if an update is available,
/// `Ok(None)` if already up-to-date.
///
/// In `cfg(debug_assertions)` builds — i.e. `cargo run` / `pnpm tauri
/// dev` — the configured updater endpoint typically points at a
/// staging URL or doesn't resolve at all, so `.check().await` hangs
/// until Tauri's IPC layer times out (~30s) and the user sees a
/// misleading "Command 'check_for_updates' timed out" error toast.
/// Short-circuit to Ok(None) — "no update available" — which matches
/// the expectation that a developer running from source doesn't
/// want to update themselves to a CDN release in the middle of
/// `tauri dev`.
pub async fn check_for_update(
    app_handle: AppHandle,
    instance_id: &str,
) -> Result<Option<UpdateInfo>, String> {
    #[cfg(debug_assertions)]
    {
        let _ = (app_handle, instance_id); // silence unused-var warning in dev
        info!("Update check skipped: debug build (cargo run / tauri dev)");
        Ok(None)
    }

    #[cfg(not(debug_assertions))]
    {
        let is_appstore = is_app_store_build();

        let update = app_handle
            .updater_builder()
            .header("X-Instance-Id", instance_id)
            .map_err(|e| format!("Failed to set header: {}", e))?
            .build()
            .map_err(|e| format!("Failed to build updater: {}", e))?
            .check()
            .await
            .map_err(|e| {
                warn!("Update check failed: {}", e);
                format!("Failed to check for updates: {}", e)
            })?;

        match update {
            Some(update) => {
                let current_version = app_handle.package_info().version.to_string();
                if update.version != current_version {
                    let pub_date = update.date.and_then(|d| {
                        let seconds = d.unix_timestamp();
                        let nanos = d.nanosecond();
                        DateTime::from_timestamp(seconds, nanos).map(|dt| dt.to_rfc3339())
                    });

                    let changelog_url = extract_changelog_url(&update.raw_json);
                    let screenshots = extract_screenshots(&update.raw_json);

                    Ok(Some(UpdateInfo {
                        current_version,
                        latest_version: update.version.to_string(),
                        notes: update.body.clone(),
                        pub_date,
                        is_app_store_build: is_appstore,
                        store_url: app_store_url().map(|url| url.to_string()),
                        changelog_url,
                        screenshots,
                    }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    } // close cfg(not(debug_assertions))
}

/// Progress payload emitted during update download/install.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadProgress {
    downloaded: u64,
    total: Option<u64>,
    /// "downloading" or "installing"
    phase: String,
}

/// Download and install an available update, then restart the app.
/// Emits `app:update-download-progress` events so the frontend can show progress.
/// Returns `Err` on failure so the frontend can display the error inline.
pub async fn install_update(app_handle: AppHandle) -> Result<(), String> {
    info!("Starting update download and installation");

    let update = match app_handle.updater_builder().build() {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => return Err("No update available.".to_string()),
            Err(e) => {
                error!("Failed to check for updates: {}", e);
                return Err(format!("Failed to check for updates: {}", e));
            }
        },
        Err(e) => {
            error!("Failed to build updater: {}", e);
            return Err(format!("Failed to initialize updater: {}", e));
        }
    };

    info!(
        "Downloading update from version {} to {}",
        update.current_version, update.version
    );

    // Track I PR-I4 — pre-update DB snapshot per ADR 0009. Snapshot the
    // outgoing version's mizan.db so the rollback path (PR-I6) has a
    // restoration source if the new binary fails its self-test (PR-I5).
    //
    // Fail-soft: snapshot failure logs + skips. We do NOT abort the
    // update on snapshot failure — that would block users from getting
    // critical security fixes if the disk is momentarily full. The
    // working-agreement §17 trade-off: 30-day snapshot retention is the
    // safety net, but its presence is best-effort, not contractual.
    match app_handle
        .path()
        .app_data_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
    {
        Some(app_data_dir) => {
            let old_version = update.current_version.to_string();
            match mizan_storage_sqlite::updater_snapshot::create_pre_update_snapshot(
                &app_data_dir,
                &old_version,
            ) {
                Ok(path) => info!(
                    "Pre-update snapshot created at {} (rollback safety net for v{})",
                    path.display(),
                    old_version
                ),
                Err(e) => warn!(
                    "Pre-update snapshot failed (continuing with update; rollback unavailable for v{}): {}",
                    old_version, e
                ),
            }
        }
        None => warn!(
            "Could not resolve app_data_dir for pre-update snapshot — rollback unavailable for v{}",
            update.current_version
        ),
    }

    let handle_chunk = app_handle.clone();
    let handle_finish = app_handle.clone();
    let mut downloaded: u64 = 0;

    match update
        .download_and_install(
            move |chunk_len, content_len| {
                downloaded += chunk_len as u64;
                let _ = handle_chunk.emit(
                    "app:update-download-progress",
                    UpdateDownloadProgress {
                        downloaded,
                        total: content_len,
                        phase: "downloading".to_string(),
                    },
                );
            },
            move || {
                let _ = handle_finish.emit(
                    "app:update-download-progress",
                    UpdateDownloadProgress {
                        downloaded: 0,
                        total: None,
                        phase: "installing".to_string(),
                    },
                );
            },
        )
        .await
    {
        Ok(_) => {
            info!("Update installed successfully, restarting");
            app_handle.restart();
            #[allow(unreachable_code)]
            Ok(())
        }
        Err(e) => {
            error!("Failed to download and install update: {}", e);
            Err(format!("Failed to install update: {}", e))
        }
    }
}
