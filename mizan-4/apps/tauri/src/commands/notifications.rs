//! Tauri commands for the personalized notification center (Notify-6).
//!
//! Surfaces the `NotificationService` to the frontend:
//!   - `list_notifications(limit)` → the bell-panel page,
//!   - `notifications_unread_count` → fast path for the badge,
//!   - `mark_notification_read(id)` / `dismiss_notification(id)` /
//!     `mark_all_notifications_read` → user actions from the panel.
//!
//! Errors are stringified (matching the rest of the codebase's IPC
//! convention) and front-end mapping happens in the React-Query hook.

use std::sync::Arc;

use log::debug;
use mizan_core::notifications::{Notification, NotificationsPage};
use tauri::State;

use crate::context::ServiceContext;

/// Cap the page size at a sane default. The bell panel renders ~10
/// rows visibly; the caller can request more for the "Show all"
/// fullscreen view, but we don't want a frontend bug requesting 50k
/// rows to materialise the whole table over IPC.
const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: usize = 200;

#[tauri::command]
pub async fn list_notifications(
    limit: Option<usize>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<NotificationsPage, String> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    debug!("list_notifications(limit={limit})");
    state
        .notification_service()
        .list_active(limit)
        .await
        .map_err(|e| format!("Failed to load notifications: {e}"))
}

#[tauri::command]
pub async fn notifications_unread_count(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<i64, String> {
    state
        .notification_service()
        .unread_count()
        .await
        .map_err(|e| format!("Failed to read unread count: {e}"))
}

#[tauri::command]
pub async fn mark_notification_read(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .notification_service()
        .mark_read(&id)
        .await
        .map_err(|e| format!("Failed to mark notification read: {e}"))
}

#[tauri::command]
pub async fn dismiss_notification(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .notification_service()
        .dismiss(&id)
        .await
        .map_err(|e| format!("Failed to dismiss notification: {e}"))
}

#[tauri::command]
pub async fn mark_all_notifications_read(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<usize, String> {
    state
        .notification_service()
        .mark_all_read()
        .await
        .map_err(|e| format!("Failed to mark all notifications read: {e}"))
}

/// Insights-engine debug command: emit a fixed test notification.
/// Behind `cfg(debug_assertions)` so it can never ship to production.
/// Useful for QA + UI screenshots before the real scheduler emits.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_emit_test_notification(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<bool, String> {
    use mizan_core::notifications::{NotificationKind, NotificationSeverity};
    let n = Notification {
        id: uuid::Uuid::new_v4().to_string(),
        kind: NotificationKind::AiDigest,
        severity: NotificationSeverity::Info,
        title: "Mizan AI digest — debug".to_string(),
        body:
            "This is a deterministic test notification. If you see this in production, file a bug."
                .to_string(),
        deep_link: Some("mizan://dashboard".to_string()),
        payload_json: "{}".to_string(),
        // Per-second uniqueness so the test command can be run multiple
        // times while developing without hitting the dedupe UNIQUE.
        dedupe_key: format!("debug_test:{}", chrono::Utc::now().timestamp()),
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        read_at_ms: None,
        dismissed_at_ms: None,
    };
    state
        .notification_service()
        .emit(n)
        .await
        .map_err(|e| format!("debug emit failed: {e}"))
}
