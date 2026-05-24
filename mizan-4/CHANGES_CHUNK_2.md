# Chunk 2 — Hook Mizan desktop to Mizan Connect

Date: 2026-05-04

This chunk re-enables the Mizan Connect surface that was gated dormant during
the initial Wealthfolio → Mizan rebrand, and points it at the proprietary
backend at <https://github.com/samisayyed1/mizan-connect>. No auth, sync, or
provider logic was rewritten — composition only.

## Re-enabled

| Surface                  | Path                                                                               | Source                                                                                                                                                                                                          |
| ------------------------ | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/connect` page          | `apps/frontend/src/routes.tsx` (was `<Navigate to="/" replace />`)                 | restored to `<ConnectPage />`                                                                                                                                                                                   |
| `/auth/callback` page    | `apps/frontend/src/routes.tsx` (was `<Navigate to="/" replace />`)                 | restored to `<AuthCallbackPage />`                                                                                                                                                                              |
| `/settings/connect` page | `apps/frontend/src/routes.tsx` (was `<Navigate to="/settings" replace />`)         | new wrapper at `apps/frontend/src/pages/settings/mizan-connect/connect-settings-page.tsx` (~30 lines) — conditionally renders existing `<LoginForm />` (signed out) or existing `<ConnectedView />` (signed in) |
| Connect nav entry        | `apps/frontend/src/pages/layouts/navigation/app-navigation.tsx` (`secondary` list) | new entry titled "Connect" using `Icons.Link`, points to `/connect`                                                                                                                                             |

The Connect provider, Supabase JS client, hooks, services, and IPC commands are
unchanged — they were never deleted, only deprived of an entry point.

## Hidden behind "Coming Soon" placeholders

A new generic `ComingSoonCard` component lives at
`apps/frontend/src/components/coming-soon-card.tsx`. It is the only new
component in this chunk; no feature logic.

The `ConnectedView` component now shows `ComingSoonCard` placeholders for the
three surfaces whose endpoints don't yet exist on Mizan Connect:

- **Plans & billing** — replaces the `<SubscriptionPlans />` render block that
  used to call `/api/v1/subscription/plans`. Will return with Chunk 2 of the
  backend (Stripe billing).
- **Brokerage sync** — placeholder for the broker connection / accounts / sync
  history surface (`/api/v1/sync/brokerage/*`). Will return with Chunk 3
  (SnapTrade integration).
- **Device sync** — placeholder for the multi-device E2EE sync surface
  (`/api/v1/sync/team/*`, `/api/v1/sync/snapshots/*`, `/api/v1/sync/events/*`).
  Will return with Chunk 4 of the backend.

The original gated render blocks (`BrokerConnectionsCard`, broker accounts,
`<DeviceSyncSection />`) are still mounted in `ConnectedView` but their
`hasSubscription` / `showBrokerSync` gates evaluate to `false` against the
Chunk-1 backend response, so they remain hidden. The new placeholders sit above
them and are the only thing the user sees today.

## Security note — hardcoded Wealthfolio fallback removed

`apps/frontend/src/features/mizan-connect/providers/mizan-connect-provider.tsx`
previously contained a hardcoded fallback Supabase publishable key
(`sb_publishable_ZSZbXNtWtnh9i2nqJ2UL4A_NV8ZVutd`) that survived the Wealthfolio
rebrand. Even with `CONNECT_AUTH_URL` empty, this would have produced a "live"
Supabase client pointed at upstream Wealthfolio's project.

Three string fallbacks were replaced with empty strings:

- `AUTH_URL` (was `"https://auth.mizan.app"`)
- `AUTH_PUBLISHABLE_KEY` (was the upstream `sb_publishable_…` key)
- `HOSTED_OAUTH_CALLBACK_URL` (was `"https://connect.mizan.app/deeplink"`)

The `CONNECT_ENABLED` flag (in `apps/frontend/src/lib/connect-config.ts`) now
becomes the only gate: when `.env` is empty, `CONNECT_ENABLED === false`, the
provider returns its disabled context, and no Supabase client is ever
constructed.

## Environment

`.env` (gitignored) now holds the Mizan Connect dev configuration. See
`.env.example` for the template. The four variables that matter:

```
CONNECT_AUTH_URL=https://jtdtfnusgloizwclhobf.supabase.co
CONNECT_AUTH_PUBLISHABLE_KEY=<your Supabase anon key — paste manually>
CONNECT_API_URL=http://localhost:8080
CONNECT_OAUTH_CALLBACK_URL=http://localhost:1420/auth/callback
```

These are read by both Vite (frontend, via `envPrefix: ["CONNECT_"]`) and the
Tauri build (`apps/tauri/build.rs`, baked in via `option_env!`).

## Re-disable Connect for an offline-only build

Set `CONNECT_AUTH_URL=""` (or remove the line) in `.env`, then rebuild.
`CONNECT_ENABLED` flips to `false` and:

- The Connect nav entry stays visible but routes to a "Mizan Connect is disabled
  in this build" placeholder.
- `useMizanConnect()` returns no-op stubs everywhere.
- No Supabase JS client is constructed.
- No HTTP calls are made to `CONNECT_API_URL`.

This is the supported path for shipping a fully-offline Mizan build.

## Out of scope (per Chunk 2 spec)

- `crates/connect/` (Rust client, default API URL, token lifecycle).
- `crates/device-sync/` (E2EE protocol).
- Any backend changes — Chunks 2-4 of `mizan-connect`.
- The `mizan.app` URL placeholders in `apps/frontend/src/lib/constants.ts`, the
  original `MIZAN_CONNECT_PORTAL_URL` (still TODO from Chunk 1 of the rebrand).
  Mizan.app is not registered yet.
