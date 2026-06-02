# ADR 0010 — IPC Schema Versioning

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** I (Cache Invalidation & Versioning Hardening) — PR-I3

## Context

Per `docs/working-agreement.md` §19.6:

> Tauri commands have versioned request/response types in a shared Rust + TS binding crate. Schema changes require version bumps and backward-compat handlers for at least one prior version (in case a stale WebView calls the latest Rust binary mid-update).

Today, Tauri command types are defined ad-hoc per command, often duplicating struct definitions between the Rust side (`apps/tauri/src/commands/*.rs`) and the TypeScript side (`apps/frontend/src/adapters/tauri/*.ts`). Schema changes are correlated by convention, not enforced by tooling.

The specific failure mode this ADR addresses: during a Tauri auto-update, the WebView has a cached bundle reflecting the OLD JS bundle while the Rust binary has already updated. A command shape change in the new binary breaks calls from the old WebView. The Vite content-hash check (PR-I8) catches this for the WebView ↔ frontend bundle pair, but doesn't catch the IPC ↔ Rust binary pair.

## Decision

Create a new shared crate `crates/ipc-schema` housing versioned Tauri command request/response types as the single source of truth. Both `apps/tauri` (Rust handler side) and `apps/frontend` (TypeScript adapter side, via `ts-rs` or equivalent codegen) depend on it.

### Type shape

Every Tauri command has a versioned request + response:

```rust
// crates/ipc-schema/src/commands/notifications.rs
pub mod v1 {
    #[derive(Serialize, Deserialize, TS)]
    pub struct NotificationsListRequest {
        pub limit: u32,
        pub cursor: Option<String>,
    }

    #[derive(Serialize, Deserialize, TS)]
    pub struct NotificationsListResponse {
        pub items: Vec<NotificationItem>,
        pub next_cursor: Option<String>,
    }
}

// When a breaking change lands:
pub mod v2 {
    #[derive(Serialize, Deserialize, TS)]
    pub struct NotificationsListRequest {
        pub limit: u32,
        pub cursor: Option<String>,
        pub filter: NotificationFilter,  // ← new required field
    }
    // ... NotificationsListResponse may also differ
}
```

### Handler dispatch

The Tauri command handler accepts both versions during the transition window:

```rust
// apps/tauri/src/commands/notifications.rs
#[tauri::command]
async fn notifications_list(
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Try v2 first (current). On schema mismatch, fall back to v1.
    if let Ok(v2_req) = serde_json::from_value::<ipc_schema::commands::notifications::v2::NotificationsListRequest>(request.clone()) {
        let resp = handle_v2(v2_req).await?;
        return Ok(serde_json::to_value(resp).expect("v2 response serialises"));
    }
    let v1_req = serde_json::from_value::<ipc_schema::commands::notifications::v1::NotificationsListRequest>(request)
        .map_err(|e| format!("neither v1 nor v2 request shape matched: {e}"))?;
    let v1_resp = handle_v1(v1_req).await?;
    Ok(serde_json::to_value(v1_resp).expect("v1 response serialises"))
}
```

### Transition window

A version is supported until **all installed binaries** capable of calling
it have been replaced. Practically:

- A new version `vN+1` of a command ships in app version `X.Y.0`
- The OLD `vN` handler stays in the binary until app version `X.(Y+2).0` ships
- The `vN` types stay in `ipc-schema` until even older binaries are no longer in
  the field (typically 90 days post-`X.(Y+2).0` based on update adoption telemetry)

### Codegen

`crates/ipc-schema` uses `ts-rs` (or equivalent) to emit TypeScript bindings to
`mizan-4/apps/frontend/src/lib/ipc-types.ts` at build time. CI gate: `cargo run -p ipc-schema --bin emit-ts` must produce no diff against the committed file (i.e. the committed file is the canonical TS).

## Rationale

**Why a shared crate rather than per-command struct definitions:**

- One file to find when reviewing a command's contract
- TS bindings stay in lock-step automatically via codegen
- A new command can't ship without its types living in the shared crate (the handler can't import them otherwise)

**Why version submodules (`v1`, `v2`) rather than struct renames (`NotificationsListRequest`, `NotificationsListRequestV2`):**

- Compiles enforce that you import a specific version — accidental cross-version coupling fails at compile time
- The transition window is explicit (handler accepts both versions simultaneously)
- IDE goto-definition lands on the right struct without ambiguity

**Why ts-rs codegen rather than hand-maintained TS:**

- Schema drift is mechanically prevented
- Type rename / field add / field remove all auto-propagate
- The CI diff check guarantees the committed file matches the canonical source

**Why a 2-minor-version transition window:**

- Tauri auto-updater adoption follows a known curve — 95% of users on a release within ~30 days, 99% within ~60-90 days
- Two minor versions × ~30 days each = ~60 days transition, matching the 99% adoption window
- Keeps the handler dispatch code lean — no version stays in the binary forever

## Consequences

**Positive:**

- Mid-update IPC schema mismatches degrade gracefully (old WebView → old version of new handler) rather than crashing
- TS bindings can never drift from Rust types
- Code review surface for "what does this command accept" is one file per command

**Negative:**

- Adds a workspace member (`crates/ipc-schema`) and a codegen step
- Two-version handlers are more verbose than single-version handlers (mitigation: clean abstraction via the dispatch helper above)

**Follow-ups (tracked):**

- PR-I3: `crates/ipc-schema` skeleton + first 2 commands migrated as proof (recommend: `notifications_list` + `accounts_list` since they're high-traffic + well-defined)
- PR-I3.b: ts-rs codegen wired into `pnpm dev` + CI diff check
- PR-I3.c: migrate all existing Tauri commands to ipc-schema (iterative, one command per PR after the skeleton)

## Alternatives Considered

**Alternative A: Hand-maintain TS types alongside Rust types.** Rejected — fails the drift-prevention test. Past Mizan QA passes (working agreement §13) show drift happens.

**Alternative B: Bump app version on every IPC change to force coordinated update.** Rejected — Tauri's update mechanism is asynchronous; can't guarantee coordinated upgrade across all installed users.

**Alternative C: Use protobuf instead of Rust structs + ts-rs.** Rejected — protobuf adds a build dependency for marginal gain over the ts-rs approach, and Mizan has no other protobuf surface to leverage.

## References

- `apps/tauri/src/commands/*.rs` — current ad-hoc command types
- `apps/frontend/src/adapters/tauri/*.ts` — current ad-hoc TS types
- `docs/working-agreement.md` §19.6
- `docs/plans/09-track-i.md` PR-I3
- ts-rs crate documentation
