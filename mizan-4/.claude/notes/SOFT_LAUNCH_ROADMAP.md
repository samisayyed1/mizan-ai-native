# Mizan Soft-Launch Production-Readiness Roadmap

Generated 2026-05-13 from a 5-agent parallel deep audit covering backend
correctness, storage / DB, frontend UX, performance / scalability, and
security. ~200 raw findings; this doc captures what was shipped
immediately in PR #23, what's blocking the soft-launch and must ship
in the next 30 days, and what's deferred to v1.x post-launch.

## TL;DR

- **Shipped this session** (PRs #18 through #23): CSV importer
  intelligence, valuation pipeline correctness, FIFO disposal, stale-
  quote + missing-FX gating, synchronous quote sync on import, CSP
  lockdown, hot-path indexes, file-permission tightening, panic
  removal on backup commands.
- **Must ship before soft-launch (T-30 days)**: backup encryption,
  log-line sanitization audit, addon manifest size cap, addon
  permission interceptor, accidental-token-leak scan, frontend
  non-null-assertion removal in critical pages, error boundary
  coverage.
- **Post-launch (v1.1)**: Instrument identity layer (Sprint 1 from the
  research memo), multi-provider quote fallback, performance-
  calculation engine (Modified Dietz / TWR / MWR), audit log, addon
  sandboxing.

---

## Already Shipped This Session

| PR | Title | What it did |
|---|---|---|
| #16 | CSV column intelligence | Smart column detection with broker-fingerprint + data-aware scoring + reconciler |
| #17 | CSV intel real-world validation | 7 integration tests against anonymized Yahoo Portfolio fixtures |
| #18 | Row filtering + import summary | Watchlist / zero-value / duplicate filtering + cost-basis & sell-proceeds totals surfaced in UI |
| #19 | Cost-basis fallback | Stop silently zeroing positions when live quote is missing |
| #20 | Synchronous quote sync on import | Await Yahoo sync before returning to UI; per-symbol report (live/failed/not_found/skipped) |
| #21 | Stale-quote gate + missing-FX strict mode | Quote >7 days old → fallback; missing FX pair → fallback (no more silent 1.0) |
| #22 | Holdings-import transaction-log aggregation | BUY-SELL netting + weighted-avg cost basis on the holdings-import path |
| #23 | Production hardening | CSP, hot-path DB indexes, chmod 600, .expect() panic removal, NaN guards |

Verified end-to-end against real Yahoo Portfolio CSVs: dashboard
total moved from a $187K silent-undervalue to within $454 of
ground-truth $259,788. Math is now defensible at the row level.

---

## Tier 1 — MUST ship before T-30 (production blockers)

These are correctness / security issues that will surface as user-
visible bugs or trust-breaking incidents within days of launch.

### Security

- [ ] **Backup encryption** — `apps/tauri/src/commands/utilities.rs::backup_database` exports the raw SQLite file. Database contains broker OAuth tokens, AI provider API keys, full activity log. If a user uploads the backup to Drive / iCloud, plaintext credentials leak. Wrap with `chacha20poly1305` + Argon2-derived key from a passphrase the user supplies at backup time.
- [ ] **Token-leak audit on logs** — grep every `info!` / `warn!` / `error!` for any code path that could print a token, API key, or refresh token. Add an `slog`-style redaction filter at the logger init so even an accidental `info!("{:?}", broker_response)` doesn't leak.
- [ ] **Addon manifest size cap** — `apps/tauri/src/commands/addon.rs::~107`: reject any `addon.json` >1 MB before serde tries to deserialize. Prevents billion-laughs / deeply-nested-JSON DoS during addon install.
- [ ] **Addon manifest validation hardness** — same file ~114: malformed manifests are silently SKIPPED. An attacker putting an addon with `permissions: null` in the addons dir loads with no checks. Fail hard, log the rejected addon's hash, surface a Settings warning.
- [ ] **`ditto` / file permission tighten on macOS backup path** — verify the backup file written by `backup_database_to_path` inherits `0600` mode, not the parent dir's default.
- [ ] **OpenFigi API key handling** — when Sprint 1 lands, ensure the key is loaded from `OPENFIGI_API_KEY` env at build time only, never persisted in DB, never logged. Document the threat model in the resolver service.

### Correctness

- [ ] **`.unwrap()` / `.expect()` in command handlers** — there are ~15 `.expect()` calls in `apps/tauri/src/commands/` still alive after PR #23. Each is a one-click Tauri runtime panic. Replace with `?` + meaningful error strings.
- [ ] **`window.__mizan_query_client__ = queryClient` (apps/frontend/src/App.tsx:29)** — global mutation without type safety. An addon can overwrite this and tank caching app-wide. Convert to a sealed `getQueryClient()` factory.
- [ ] **Frontend non-null assertion sweep** — 30+ `someObject!.field` patterns documented by the frontend audit. Each is a production crash waiting on the right edge case. Replace the top 10 with optional-chaining + fallback rendering. Audit list: `pages/holdings/components/holdings-grouped-table.tsx:91`, `pages/account/account-contribution-limit.tsx:55`, `pages/holdings/components/allocation-detail-sheet.tsx:154`, etc.
- [ ] **Form `as any` removal in symbol-search.tsx** — 18 occurrences of `as any` in one file bypass `react-hook-form`'s schema validation. Replace with proper generic typing.
- [ ] **Error boundary at root + per-route** — `App.tsx` has no global `ErrorBoundary`. The inline ErrorBoundary class inside `activity-import-page.tsx:550` is redefined every render (broken React invariant). Extract to a proper component, wrap `routes.tsx` with a root `ErrorBoundary` that surfaces a "something went wrong, here's the recovery action" screen.
- [ ] **`<Suspense>` fallback dynamic-route error path (routes.tsx:108)** — if an addon chunk fails to load, the user sees "Loading…" forever. Wrap each dynamic route in an `<ErrorBoundary>` with a "reload this page" affordance.

### Data integrity

- [ ] **Add `ON DELETE CASCADE` to holdings_snapshots + daily_account_valuation** — PR #18 added a defensive cleanup in the account-delete path that handles this at the repo layer, but the constraint should also exist at the schema layer so anyone bypassing the repo (manual SQL, future ingestion paths) can't orphan rows. Migration is non-trivial because SQLite can't ALTER TABLE add FK; need the table-swap pattern. Plan: create new table with FK → copy rows → drop old → rename. Test against a 100K-snapshot fixture before shipping.
- [ ] **JSON `CHECK(json_valid(positions))` on `holdings_snapshots`** — malformed JSON in `positions` / `cash_balances` / `realized_gains` silently becomes empty. Add a SQLite CHECK constraint at write time so corrupt JSON gets rejected, not silently defaulted to `{}`.
- [ ] **Snapshot integrity check after restore** — `restore_database` doesn't run `PRAGMA integrity_check`. A corrupt backup silently restores. Add the check, abort restore on FAIL.
- [ ] **Audit `let _ = ...` patterns in delete paths** — `crates/core/src/quotes/service.rs:693` and similar swallow deletion failures. Surface as aggregate error to the caller.

### Performance

- [ ] **Snapshot rebuild incremental-by-default** — `SnapshotRecalcMode::Full` is fired in too many paths (activity edit, recalc button). With 5K activities → 1800 snapshots rebuilt. Audit every call site of `recalculate_portfolio` and route to `IncrementalFromLast` unless the user explicitly clicked "Rebuild full history".
- [ ] **Asset symbol resolution N+1 in import** — `activities_service.rs:2615-2627` loops through activities calling provider per row. Batch into a single provider call before the loop.

### Frontend production-readiness

- [ ] **`<HoldingsTable>` `React.memo`** — 1000+ row table re-renders on every parent state change. Audit also flagged `<HistoryChart>`. Memoize.
- [ ] **`localStorage` try/catch wrap** — `mizan-connect-provider.tsx:158/166/174` direct localStorage access throws in private browsing / quota-full. Wrap.
- [ ] **`toast.error()` sanitization** — multiple paths show full error stacks in toasts. Standardize on `humanizeError(e)` that strips stack traces but preserves the backend `Result<_, String>` message.

---

## Tier 2 — Should ship before T-30 (high impact, lower risk)

Quality-of-life and reliability that doesn't gate launch but will
reduce inbound support tickets.

- [ ] **Health-check fix actions wired up** — three `// TODO` comments in `crates/core/src/health/service.rs` mark fix actions that are no-ops (quote sync, FX sync, taxonomy migration). PR #11 wired one (`fetch_fx`); finish the rest.
- [ ] **Rate-limit Tauri command surface** — no rate limiting today. Add a 100-req/sec/window middleware so a runaway frontend loop can't DoS the backend.
- [ ] **`Number.isFinite` guards on remaining frontend formatters** — `formatAmount` already guards (PR #23 verified); also audit the live-preview pane (`pages/settings/market-data/live-preview-pane.tsx:773+`) for un-guarded `.toLocaleString()` on API response data.
- [ ] **CSV injection on export** — when user exports portfolio to CSV, prefix any cell starting with `=`, `+`, `-`, `@` with a `'`. Prevents Excel formula injection when the CSV is shared with an advisor.
- [ ] **Rate-limit pairing attempts in `enroll_service.rs`** — exponential backoff after 3 failed pairing codes. Currently brute-forceable.
- [ ] **Bundle size: lazy-load `recharts` + tree-shake `lucide-react`** — the audit calls out both as eagerly loaded. ~250 KB savings on initial bundle.

---

## Tier 3 — v1.1 (post-launch)

The strategic moves from the research memo + structural items that
need design work, not just bug-fixes.

### Sprint 1 — Instrument identity layer (the research memo's headline)

- [ ] `instruments` + `instrument_aliases` + `market_identifier_codes` + `provider_exchange_mappings` + `quote_provider_health` tables.
- [ ] MIC registry seeded from the ISO 10383 monthly CSV via `include_str!`.
- [ ] OpenFIGI client (env-keyed via `OPENFIGI_API_KEY`, backend-only, retry + rate-limit).
- [ ] `InstrumentResolver` with confidence scoring per memo §9.3 (auto-accept ≥ 75, otherwise → review queue).
- [ ] `debug_resolve_instrument(symbol)` Tauri command + hidden Settings page.
- [ ] Tests for `AJBU.SI`, `558.SI`, `9A4U.SI`, `0700.HK`, `RELIANCE.NS`, `BRK.B`, `JOBY`.
- [ ] Per-symbol provider preference table so SGX → SGX EOD provider, NSE → NSE official, etc.

### Sprint 2 — Quote router + multi-provider fallback

- [ ] `QuoteProviderRouter` with Yahoo → Twelve Data → Finnhub → Alpha Vantage → Exchange EOD chain.
- [ ] Per-provider health tracking, automatic cooldown on rate-limit.
- [ ] WebSocket / real-time channel for premium tier (post-soft-launch).

### Sprint 3 — Performance engine

- [ ] Modified Dietz / TWR / MWR / IRR following GIPS methodology.
- [ ] Pre-computed rolling returns stored in `daily_account_valuation`.
- [ ] "Performance methodology" disclosure modal in UI.

### Sprint 4 — Audit log + multi-device E2E hardening

- [ ] `audit_events` table — every sensitive op (delete, export, key change, addon install) logged with hash + timestamp.
- [ ] Device-sync crypto: signed ephemeral keys (X25519 → Ed25519 signing), nonce + timestamp for replay protection, HKDF with per-session info strings.
- [ ] Certificate pinning for the Mizan Connect API endpoints.

### Sprint 5 — Addon sandboxing

- [ ] IPC interceptor that validates every addon call against its declared permissions.
- [ ] Per-addon storage namespace (`addons/{addon_id}/...`).
- [ ] Signed addon manifests (developer keys).
- [ ] Reclassify event-listener permissions as HIGH risk.

---

## Reference: Audit Source Material

This roadmap distils:

1. **Backend correctness audit** — 47 findings in `crates/core` (silent error swallowing, division-by-zero, FIFO accounting, FX correctness, concurrency).
2. **Storage / DB audit** — 36 findings across migrations, indexes, FK coverage, write-actor concurrency, NULL semantics.
3. **Frontend UX audit** — 55 findings across error boundaries, loading states, non-null assertions, `as any` escapes, useEffect bugs, accessibility, toast sanitization.
4. **Performance / scalability audit** — 31 findings on N+1 queries, full-table scans, frontend re-render storms, bundle size, snapshot rebuild cost.
5. **Security audit** — 29 findings: CSP missing, account-id authorization (n/a for local-first), addon permission enforcement, credential storage, device-sync crypto, log redaction, update signature verification, file permissions.

Audit reports themselves are not in source control. They were
returned by Explore agents during this session and informed the
fixes shipped in PR #23. Future audits should be re-run before each
major release.

---

## Owner

Sami / scout@maevemodels.co.uk · Mizan core team. Soft-launch
target: T+30 days from 2026-05-13. Update this doc as items move
between tiers.
