# Chunk 3 — Hook Mizan desktop to live SnapTrade

Date: 2026-05-05

This chunk re-enables the broker-sync UI surface (the third "Coming Soon"
placeholder we added in Chunk 2) and wires the desktop's existing flow to the
real `/api/v1/sync/brokerage/*` endpoints shipped by Mizan Connect
[`@443d458`](https://github.com/samisayyed1/mizan-connect/commit/443d458).

## Backend dependency

Requires Mizan Connect Chunk 3 or later. With an older backend, the new
`create_broker_login_portal` IPC call will surface a generic "broker integration
error" toast.

## What's now visible

| Surface                                          | Was (Chunk 2)                                                          | Now (Chunk 3)                                                                                                                |
| ------------------------------------------------ | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Brokerage `<ComingSoonCard/>` in `ConnectedView` | placeholder shown                                                      | **removed** — real `BrokerConnectionsCard` + Accounts card render below                                                      |
| `ConnectEmptyState` CTA button                   | `<ExternalLink href={MIZAN_CONNECT_PORTAL_URL}>` (unregistered domain) | **"Connect a broker"** that calls `/api/v1/sync/brokerage/login-portal` and opens the SnapTrade portal in the user's browser |
| `SyncButton` in `activity-page.tsx`              | gated by `hasBrokerSync(userInfo)` (always false — no team data)       | gated by `isEnabled && isConnected`                                                                                          |
| `ConnectPage` empty-state early return           | gated by `!hasSubscription`                                            | gated by `!isConnected`                                                                                                      |
| `useAggregatedSyncStatus` `showBrokerSync`       | `hasBrokerSync(userInfo)`                                              | `isConnected`                                                                                                                |
| `connected-view.tsx` `showBrokerSync`            | `hasBrokerSync(userInfo)`                                              | `!!userInfo`                                                                                                                 |

Each loosened gate carries an inline **`TODO(chunk-4)`** comment so Stripe
restoration in Chunk 4 is a `git grep` away.

## Plans / Device sync placeholders unchanged

The `Plans & billing` and `Device sync` ComingSoonCards in `ConnectedView`
remain in place — those land with Chunks 4 (Stripe) and 6 (E2EE device sync).

## New code (composition + thin wrapper layers, no new auth or sync logic)

| File                                                                               | Type         | Purpose                                                                                                                                                                                                                                                                                            |
| ---------------------------------------------------------------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/connect/src/client.rs`                                                     | **modified** | Added `LoginPortalResponse` struct + `ConnectApiClient::create_login_portal()` (POST /api/v1/sync/brokerage/login-portal). Mirrors existing GET-method shape. Uses the same `headers()` (Bearer auth) and `parse_response()` pipeline as every other endpoint — no new auth, no new HTTP plumbing. |
| `crates/connect/src/client.rs`                                                     | **modified** | `has_broker_sync()` short-circuits to `Ok(true)` when `CONNECT_BYPASS_PLAN_CHECK=true`. Tagged `TODO(chunk-4)`.                                                                                                                                                                                    |
| `crates/connect/src/client.rs`                                                     | **modified** | 4 new unit tests (wiremock-driven): URL+auth header verification, broker-slug pass-through, 429 propagation, env-flag bypass.                                                                                                                                                                      |
| `crates/connect/Cargo.toml`                                                        | **modified** | Added `wiremock = "0.6"` and `serial_test = "3"` as dev-dependencies.                                                                                                                                                                                                                              |
| `crates/connect/src/lib.rs`                                                        | **modified** | Re-export `LoginPortalResponse`.                                                                                                                                                                                                                                                                   |
| `apps/tauri/src/commands/brokers_sync.rs`                                          | **modified** | Added `create_broker_login_portal` Tauri command wrapping the new client method.                                                                                                                                                                                                                   |
| `apps/tauri/src/lib.rs`                                                            | **modified** | Registered `create_broker_login_portal` under `connect-sync` feature flag.                                                                                                                                                                                                                         |
| `apps/frontend/src/adapters/shared/connect.ts`                                     | **modified** | Added `BrokerLoginPortalResponse` type + `createBrokerLoginPortal()` adapter.                                                                                                                                                                                                                      |
| `apps/frontend/src/adapters/web/index.ts`                                          | **modified** | Re-exported `createBrokerLoginPortal` from web adapter (Tauri adapter picks it up via the existing `export *`).                                                                                                                                                                                    |
| `apps/frontend/src/features/mizan-connect/services/broker-service.ts`              | **modified** | Added `createBrokerLoginPortal()` service wrapper with logging.                                                                                                                                                                                                                                    |
| `apps/frontend/src/features/mizan-connect/hooks/use-create-broker-login-portal.ts` | **new**      | `useCreateBrokerLoginPortal` (mutation that opens the URL in the user's browser) + `usePollConnectionsAfterPortal` (60-second polling window invalidating the broker queries every 5 s).                                                                                                           |
| `apps/frontend/src/features/mizan-connect/hooks/index.ts`                          | **modified** | Re-export the two new hooks.                                                                                                                                                                                                                                                                       |
| `apps/frontend/src/features/mizan-connect/components/connect-empty-state.tsx`      | **modified** | Replaced the broken `Get Started with Connect` external link with a real `Connect a broker` button driven by the new hooks. Falls back to a "Sign in to Mizan Connect first" link when the user isn't authenticated.                                                                               |
| `apps/frontend/src/features/mizan-connect/components/connected-view.tsx`           | **modified** | Removed the brokerage `<ComingSoonCard/>` (lines 511–517 in the Chunk 2 snapshot). Loosened `showBrokerSync` to `!!userInfo`. Removed the now-unused `hasBrokerSync` import.                                                                                                                       |
| `apps/frontend/src/features/mizan-connect/components/sync-button.tsx`              | **modified** | Removed the `hasBrokerSync(userInfo)` gate. Loosened to `isEnabled && isConnected`.                                                                                                                                                                                                                |
| `apps/frontend/src/features/mizan-connect/pages/connect-page.tsx`                  | **modified** | Removed the `!hasSubscription` early-return. Removed the now-unused `hasSubscription` memo.                                                                                                                                                                                                        |
| `apps/frontend/src/features/mizan-connect/hooks/use-aggregated-sync-status.ts`     | **modified** | Loosened `showBrokerSync` to `isConnected`. Removed unused `hasBrokerSync` import.                                                                                                                                                                                                                 |

## Polling refetch behaviour

When the user clicks **Connect a broker** the new `useCreateBrokerLoginPortal`
hook does three things in order:

1. POSTs `/api/v1/sync/brokerage/login-portal` (with backend rate-limit
   protection).
2. Opens the returned URL in the default browser via `openUrlInBrowser` (the
   existing Tauri adapter — no new IPC).
3. Triggers `usePollConnectionsAfterPortal`, which invalidates
   `BROKER_CONNECTIONS` and `BROKER_ACCOUNTS` every 5 seconds for 60 seconds. As
   soon as SnapTrade redirects back to Mizan Connect's callback handler and the
   backend persists the row, the next poll picks up the new authorization
   automatically — no manual refresh required.

Polling stops on its own, or on component unmount, or when `start()` is called
again (resets the 60 s window).

## How to re-disable broker UI

Two options for an offline / Chunk-3-disabled build:

1. Empty `CONNECT_AUTH_URL` in `.env` → `CONNECT_ENABLED = false` →
   `useMizanConnect()` returns disabled context → every gate above returns false
   → broker UI stays hidden.
2. Run the desktop with a Mizan Connect backend that hasn't shipped Chunk 3 yet
   — the IPC will fail with the standard "broker integration error" toast and
   the user sees the empty state.

## What did NOT change

- Supabase auth flow, `/auth/callback` deep-link handling,
  `MizanConnectProvider`.
- `crates/device-sync/` (Chunk 6).
- Any other Connect endpoint paths — the Rust client's existing routes already
  match Chunk 3 backend exactly.
- The existing `broker_ingest_run` IPC and its `has_broker_sync()` gate — but
  the gate is now bypassable in dev via `CONNECT_BYPASS_PLAN_CHECK=true`.
