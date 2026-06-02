//! Per-command versioned schema modules.
//!
//! Each command gets its own submodule containing one or more version
//! submodules (`v1`, `v2`, ...). The currently-shipping version is the
//! highest-numbered; older versions stay until the 2-minor-version
//! transition window expires (per ADR 0010).
//!
//! Pattern for adding a NEW command:
//!
//! 1. Create `commands::<command_name>.rs` with `pub mod v1 { ... }`
//! 2. Register the file in `commands/mod.rs` (this file)
//! 3. Wire the Tauri handler in `apps/tauri/src/commands/...` to import
//!    from this crate
//! 4. Add the TS bindings emit-step in the frontend build (handled by
//!    PR-I3.b ts-rs codegen wiring)
//!
//! Pattern for VERSIONING an existing command (when a breaking change
//! lands):
//!
//! 1. Add `pub mod v2 { ... }` alongside `v1`
//! 2. Handler accepts BOTH via `try_parse_versions!` macro
//! 3. Frontend adapter is migrated to send v2-shaped requests
//! 4. After 2 minor versions ship, drop the v1 handler arm + the v1 module

pub mod notifications;
