//! Versioned Tauri command request/response schemas.
//!
//! Per ADR 0010 (`docs/adr/0010-ipc-schema-versioning.md`) and the working
//! agreement §19.6, every Tauri command's request + response types live
//! here as the single source of truth. Both `apps/tauri` (Rust handler
//! side) and `apps/frontend` (TypeScript adapter side, via `ts-rs`
//! codegen) depend on this crate.
//!
//! ## Why this crate exists
//!
//! The failure mode: during a Tauri auto-update, the WebView has a cached
//! bundle reflecting the OLD JS bundle while the Rust binary has already
//! updated. A command shape change in the new binary breaks calls from
//! the old WebView. ADR 0010's transition-window pattern (handler accepts
//! BOTH v1 and v2 simultaneously for 2 minor versions) requires versioned
//! types — that's what this crate provides.
//!
//! ## Structure
//!
//! Every command lives in `commands::<command_name>` as a module containing
//! versioned submodules:
//!
//! ```text
//! commands/
//!   notifications.rs    -> mod v1, mod v2, ...
//!   accounts.rs         -> mod v1, mod v2, ...
//!   ...
//! ```
//!
//! Each version submodule contains `{CommandName}Request` and `{CommandName}Response`
//! structs derived with `Serialize + Deserialize + TS` (gated by the
//! `ts-export` feature).
//!
//! ## What this PR ships (PR-I3 skeleton)
//!
//! - The crate scaffolding + Cargo.toml + lib.rs.
//! - A worked example (`commands::notifications`) showing the v1/v2 pattern.
//! - The `Deserialize` fallback dispatch helper that handlers use to accept
//!   multiple versions.
//!
//! Migration of existing Tauri commands to this crate happens iteratively
//! in follow-up PRs (PR-I3.c..N), one command per PR to keep review surface
//! manageable.

pub mod commands;

pub use commands::*;

/// Helper for handlers that accept multiple versions of a request shape.
///
/// Tries each version in order (newest first). Returns the FIRST version
/// that parses cleanly. Returns an error only if every version fails.
///
/// # Example
///
/// ```ignore
/// use mizan_ipc_schema::commands::notifications;
///
/// #[tauri::command]
/// async fn notifications_list(req: serde_json::Value) -> Result<serde_json::Value, String> {
///     try_parse_versions!(req, [
///         notifications::v2::NotificationsListRequest => handle_v2,
///         notifications::v1::NotificationsListRequest => handle_v1,
///     ])
/// }
/// ```
#[macro_export]
macro_rules! try_parse_versions {
    ($json:expr, [$($ty:ty => $handler:ident),+ $(,)?]) => {{
        let mut last_err: Option<String> = None;
        $(
            match serde_json::from_value::<$ty>($json.clone()) {
                Ok(req) => return $handler(req).await,
                Err(e) => last_err = Some(format!("{}: {}", stringify!($ty), e)),
            }
        )+
        Err(format!(
            "no version of the request schema matched. Last error: {}",
            last_err.unwrap_or_else(|| "(no versions tried — empty macro arg?)".into())
        ))
    }};
}
