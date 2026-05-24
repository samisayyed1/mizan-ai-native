# Changelog

All notable changes to Mizan desktop ship from this file. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/spec/v2.0.0.html).

## [3.4.1] — 2026-05-09

### Added

- **Realized P&L is computed, stored, and surfaced.** When a SELL activity is
  processed, the holdings calculator now derives
  `realized_gain = proceeds − cost_basis − fees` from the FIFO disposal that was
  already happening (and already producing the right `cost_basis_removed` number
  — it was just being thrown away). Both currency rails are tracked using
  trade-date FX:
  - **Account currency** (the account's reporting currency at trade date).
  - **Base currency** (the portfolio's base currency at trade date).
  - Gross numbers stored separately (proceeds / cost basis / sell-side fees) so
    an annual realized-gains report can reconcile back to broker statements
    column-for-column.
- **Lifetime accumulator persisted on every snapshot.** `AccountStateSnapshot`
  carries a new `realized_gains: HashMap<asset_id, RealizedGainEntry>` that
  accumulates across every SELL the calculator has processed up to that snapshot
  date — kept signed (so realized losses surface as negative numbers) and
  forward-rolled automatically via the existing daily snapshot clone. Each entry
  also tracks `quantity_sold` and the most-recent `last_sale_date` for
  downstream views.
- **Holdings table now shows realized gain.** The existing performance column
  header is correctly relabelled `Total Return` (it now genuinely reflects
  unrealized + realized, since `total_gain` is the sum of both). A new
  `Realized Gain` column — hidden by default, available via the column toggle —
  renders the lifetime realized P&L per holding with account / base currency
  display respecting the existing convert-to-base toggle. Holdings with no SELLs
  ever show an em-dash so the column stays uncluttered for typical buy-and-hold
  portfolios.
- **`Holding.total_gain` and `Holding.total_gain_pct` correctly populated.**
  Previously the valuation service zeroed `realized_gain` back to `None` at the
  end of every valuation pass and copied `unrealized_gain` straight into
  `total_gain` — meaning even when realized data existed in the snapshot, the
  engine threw it away before the frontend could see it. New
  `populate_total_gain()` helper folds unrealized + realized into total at the
  three valuation exit-points (security, alternative asset, expired option).

### Verified

- 935 / 935 mizan-core unit tests pass (3 new realized-gain tests + 201 existing
  portfolio tests including the full sell-activity matrix).
  `test_sell_activity_updates_holdings_and_cash` extended to assert the exact
  realized-gain math for the existing scenario: selling 5 of 10 AAPL @ 160 CAD
  against avg cost 150 with a 2 CAD fee produces +48 CAD realized gain.
- `test_realized_gain_accumulates_across_multiple_sells` proves the accumulator
  carries forward across daily snapshots — two trims of the same NVDA position
  on different days correctly cumulate to the sum of their individual realized
  gains.
- `test_realized_loss_recorded_correctly` covers the loss case (selling PLTR at
  $20 against $50 cost basis lands as a $-301 realized loss, fee included).

### Backfill

- Realized gains backfill automatically. The portfolio-recalculate path rebuilds
  snapshots from scratch, processing every historical SELL through the new code,
  so existing users see their lifetime realized P&L populate on the next
  recalculation tick — no migration step needed, no manual intervention.

### Notes

- The current ship surfaces realized gains for **currently-held** positions in
  the holdings table. A dedicated "Realized Gains" view for fully-sold positions
  (where the lifetime accumulator still holds the data but the position itself
  is gone from the live holdings list) and an annual realized-gains CSV export
  are reserved for `3.4.2`. The accumulator already stores everything those
  views need — the 3.4.2 work is purely a UI surface, no further engine changes.

## [3.4.0] — 2026-05-09

### Added

- **Schedule a Recurring Buy / SIP / DCA (end-to-end).** New action on every
  account page: _Schedule Recurring Buy_. Opens a dialog where the user enters
  symbol, cash amount per buy, reference price per share, frequency (weekly /
  biweekly / monthly / quarterly / semi-annual / annual), start date, number of
  installments (1–240), currency, and optional notes. The backend
  deterministically generates a series of `BUY` activities at the configured
  cadence and inserts them into the account.
  - Pure scheduling logic lives in
    `crates/core/src/activities/rsp_scheduler.rs`. Thirteen unit tests cover
    every cadence (weekly through annual), fractional- share rounding to 6 d.p.,
    symbol normalisation (uppercase, trimmed), exchange MIC pass-through, and
    the full validation matrix (non-positive amount, non-positive price, empty
    symbol, zero installments, >240 installments, plus metadata traceability).
  - Each emitted BUY has `quantity = amount_per_buy / unit_price` (rounded to 6
    d.p. — the precision most brokers report for fractional fills) and
    `unit_price = unit_price`, so the engine treats it identically to any
    manually-entered or broker-synced BUY. Activities land as `POSTED` — the
    portfolio reflects the plan immediately.
  - Each emitted activity carries `source_system = "RSP_SCHEDULE"` and a JSON
    metadata blob with the originating
    symbol/amount/price/frequency/installments/ start, so future "delete this
    RSP's schedule" or "show RSP lineage" surfaces have the lineage they need.
  - New `create_recurring_buy_plan` Tauri command wired through both the Tauri
    and web platform adapters, so the same call path works on macOS Apple
    Silicon, macOS Intel, Windows, Linux, and the self-hosted server build.

### Notes

- The `unit_price` in the dialog is a **reference / planned price** applied to
  every emitted BUY in the schedule. For past dates this is the historical close
  the user is modelling; for future dates this is their estimate. Users can edit
  individual emitted BUY activities afterwards (e.g. once a real fill price is
  known) — the schedule itself is deterministic so the portfolio reflects the
  plan immediately, and individual fills can be reconciled later without
  rebuilding the rule.
- Live-price auto-execution (auto-fill `unit_price` from the close on the
  scheduled date via the market-data resolver, on a startup tick) is reserved
  for a follow-up release. The current ship is the full schedule-and-emit loop
  end-to-end; the auto-reconcile-with-real-prices loop layers on top without
  changing the user-facing flow.

## [3.3.8] — 2026-05-09

### Added

- **Schedule a Fixed Deposit (end-to-end).** New action on every account page:
  _Schedule Fixed Deposit_. Opens a dialog where the user enters principal,
  annual rate, term in months, payment frequency (monthly / quarterly /
  semi-annual / annual / at maturity), start date, currency, and optional notes.
  The backend deterministically generates a series of `INTEREST` activities at
  the configured cadence and inserts them into the account.
  - Pure scheduling logic lives in `crates/core/src/activities/fd_scheduler.rs`.
    Twelve unit tests cover every cadence (monthly through at-maturity),
    pro-rated maturity payouts for non-12-month terms, zero-rate edge case,
    term-shorter-than-cadence rejection, non-positive principal rejection, and
    metadata traceability.
  - The principal stays in the user's cash balance — Mizan does **not**
    double-count it as a separate position, and the engine treats the emitted
    INTEREST entries as gain (no `net_contribution` change), so the FD's
    interest feeds CAGR/TWR through the same path as any broker-synced cash
    sweep.
  - Each emitted activity carries `source_system = "FD_SCHEDULE"` and a JSON
    metadata blob with the originating principal/rate/term/start, so future
    "delete this FD's schedule" or "show FD lineage" surfaces have the lineage
    they need.
  - New `create_fixed_deposit` Tauri command wired through both the Tauri and
    web platform adapters, so the same call path works on macOS Apple Silicon,
    macOS Intel, Windows, Linux, and the self-hosted server build.

### Verified

- Cash CAGR treatment confirmed via the existing test
  `test_interest_increases_cash_but_not_net_contribution` (passes today): a
  $5,000 deposit + $50 interest leaves cash at $5,050 but `net_contribution`
  stays at $5,000 — so the CAGR engine sees only the $50 as gain. Documented the
  source line in the test suite for future regression watches.
- Gold live pricing path: the `metal_price_api` provider supports XAU, XAG, XPT,
  XPD plus weight units (XAU-1KG, XAU-500G); existing network-gated tests
  (`test_get_latest_quote_gold_usd`, `test_get_latest_quote_gold_chf`) prove the
  chain works end-to-end once a real `METAL_PRICE_API_KEY` is provided in
  Settings → Market Data.

## [3.3.7] — 2026-05-07

### Changed

- **Gold accent layer.** The desktop dark theme now picks up the Mizan brand
  gold (`#D4A574`) on every primary surface — buttons, focus rings, sidebar
  primary, etc. — matching the landing page's locked palette. Implemented as a
  token-only change in `globals.css`: the existing Flexoki palette stays in
  place; only `--primary`, `--primary-foreground`, `--ring`,
  `--sidebar-primary`, `--sidebar-primary-foreground`, and `--sidebar-ring` are
  repointed in the `.dark` block. Zero component edits, so the blast radius is
  every `bg-primary`/`text-primary`/`ring-ring` usage automatically, and nothing
  else.
- New `--gold-cream`, `--gold-primary`, `--gold-deep` tokens (plus HSL component
  vars `--gold-*-hsl` for alpha-blended use) exposed at `:root` so the rest of
  the app can reach the brand palette directly when needed.
- Mizan Connect sidebar icon: removed the hardcoded `text-blue-400` and follows
  `text-primary` so the icon now glows gold like the rest of the brand surface.

### Backend

- (Shipped on 2026-05-06, retroactive note for the desktop release): Disconnect
  bug — backend `DELETE /api/v1/sync/brokerage/connections/:id` and the latent
  `POST /connections/:id/refresh` now resolve by SnapTrade's `authorization_id`
  instead of the local `broker_connections.id` UUID. The desktop client only
  ever knew the SnapTrade id; old handlers declared `Path<Uuid>` and looked up
  by local id, so every disconnect 404'd with `connection not found`. Already
  deployed to Fly; no desktop install needed for that fix to land.

## [3.3.6] — 2026-05-06

### Added

- **Disconnect a broker** end-to-end. Each broker connection in the Settings →
  Mizan Connect tab and the Connect page now shows a small trash icon. Clicking
  it opens a confirm popover ("Disconnect Alpaca Paper?" / "Disconnect" /
  "Cancel"); on confirm, the cloud API revokes the upstream SnapTrade
  authorization and soft-deletes the cloud row. The connection vanishes from
  both surfaces immediately via React Query invalidation.
  - Already-synced local data (accounts, holdings snapshots, activity history)
    is preserved on-device. Disconnecting a broker doesn't erase past records —
    only stops live sync.
  - You can reconnect any time via the "Add broker" / "Connect a broker" button
    (existing flow).
  - Wired across all desktop targets: macOS Apple Silicon, macOS Intel, Windows
    x64, Linux AppImage. The web build (apps/server) routes through the same
    `DELETE /connect/connections/:id` path.

### Plumbing

- New Rust client method `ConnectApiClient::delete_connection` →
  `DELETE /api/v1/sync/brokerage/connections/:id`.
- New Tauri command `delete_broker_connection(connection_id)` registered in
  `apps/tauri/src/lib.rs` under the `connect-sync` cargo feature.
- New JS adapter binding `deleteBrokerConnection(connectionId)` and service
  wrapper, surfaced to platform indices for both the Tauri and web build
  targets.

## [3.3.5] — 2026-05-06

### Fixed

- **Broker sync now actually runs in production builds.** Clicking "Sync Now" on
  a connected broker (Alpaca Paper, etc.) was failing with "Failed to start
  sync: Unknown error" because:
  1. `has_broker_sync()` checked `CONNECT_BYPASS_PLAN_CHECK` via
     `std::env::var()` at runtime. macOS apps launched from Finder / Dock have
     no shell env, so the bypass never triggered, and since the Connect backend
     doesn't yet return `team.plan` (Chunk 4 work) the gate returned `Ok(false)`
     → "Plan does not include broker sync" → sync refused to start.
  2. The frontend toast read `error instanceof Error ? error.message`, but Tauri
     rejects `Result<_, String>` commands with a plain string. Strings aren't
     `Error` instances, so the real reason was silently swallowed and the user
     just saw "Unknown error".

  Fix: `has_broker_sync()` now also reads the value baked at compile time via
  `option_env!()`, so the GitHub Actions Variable propagates into shipped
  binaries. Local dev (`pnpm tauri dev` with the env in the shell) keeps working
  too — both paths are accepted. Frontend toast helpers now unwrap string
  rejections so the actual backend message ("Plan does not include broker sync",
  etc.) reaches the user.

### Infrastructure

- Set `CONNECT_BYPASS_PLAN_CHECK=true` GitHub Actions Variable on the repo.
  Release workflow's "Build frontend" + tauri-action steps both pass it through.
  Both will be removed in Chunk 4 once Stripe lands and the real plan check
  returns truthful values.

## [3.3.4] — 2026-05-06

### Removed

- Diagnostic amber marker bar and `[mizan-connect]` console log on the Mizan
  Connect settings page. The fix from v3.3.3 is verified against the live
  backend — sign-in, broker connection (Alpaca Paper), and signed-in /
  signed-out transitions all work end-to-end. This release is the clean handoff
  cut for external sharing.

## [3.3.3] — 2026-05-06

### Fixed

- **Settings sidebar navigation hardened.** The sidebar's `NavLink to="connect"`
  is now resolved to the absolute `/settings/connect` rather than relying on
  React Router's relative-path resolver. The route tree contains TWO routes
  named `connect` (one at AppLayout for `/connect`, one at SettingsLayout for
  `/settings/connect`). Static analysis confirms relative resolution is correct
  in isolation, but the absolute path is defensive against any runtime edge
  case.

### Diagnostic

- ConnectSettingsPage now logs to the WebView console on mount and renders a
  small amber marker bar at the top showing the live `pathname`, `isEnabled`,
  and `isConnected` values. Right-click → Inspect → Console to read the log
  line. The marker is removed in the next release once routing is verified
  healthy in production.

## [3.3.2] — 2026-05-06

### Fixed

- **Mizan Connect navigation still broken in v3.3.1 builds.** The release
  workflow had two Vite invocations — an explicit "Build frontend" step followed
  by tauri-action's own beforeBuildCommand. Only the latter received the
  `CONNECT_*` env vars. When Tauri's `generate_context!` macro caches against
  the dist/ produced by the first invocation, the second build's correctly-baked
  bundle gets shadowed. The fix is to set the env block on both steps; the chain
  is now idempotent.

### Changed

- Tauri `devtools` feature is now enabled in production builds. Right- click in
  the app and choose **Inspect** to open the WebView devtools console. This is a
  temporary diagnostic aid for the founding-member period and will be disabled
  before live-broker launch.

## [3.3.1] — 2026-05-06

### Fixed

- **Mizan Connect navigation broken in production builds.** The 3.3.0 installer
  shipped with `CONNECT_AUTH_URL` and `CONNECT_AUTH_PUBLISHABLE_KEY` empty at
  build time, which set `CONNECT_ENABLED = false` and gated the Mizan Connect
  provider / routes into a "disabled" placeholder. Clicking the sidebar entry
  appeared to do nothing — the route resolved, but the page rendered a silent
  disabled state instead of the sign-in form. Build-time variables are now
  configured on the repo and bake into every release.

## [3.3.0] — 2026-05-05

Initial public release of the Mizan desktop app under the Mizan brand, forking
from the upstream lineage. Includes the Mizan Connect cloud integration
(Supabase auth, broker sync via SnapTrade), the Mizan Compass instrument, the
editorial landing site, and the cross-platform release pipeline (macOS Apple
Silicon, macOS Intel, Windows x64, Linux AppImage).
