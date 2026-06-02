# Mizan Desktop — Tauri 2 App

Operating manual for Claude Code. Auto-loaded for any session inside
`mizan-ai-native/mizan-4/`. **Read this fully before making any change.**
The root operating manual (`@../CLAUDE.md`), the binding product spec
(`@../MIZAN_AI_NATIVE_PLAN.md`), and the coding contract
(`@../docs/working-agreement.md`) load alongside this one.

> **Desktop is the front door.** Local-first, encrypted SQLite, AI-as-the-OS.
> Optional cloud sync via Mizan Connect (separate sub-product).

## Release sequencing lock

No production credentials until v3 Phase N validation passes. The root
`.claude/settings.json` hook blocks `gh pr merge`, `git push`, and `fly
deploy` unless `MIZAN_ALLOW_PRODUCTION=1` is exported.

## What this is

Mizan Desktop is the **open-source (AGPL)** Tauri 2 wealth engine. Rust
workspace + React 18 + TS strict + Vite + Tailwind + shadcn/ui in a
sandboxed WebView. Single Rust process owns SQLite via `rusqlite`, IPC,
and all crypto.

The binary signs and notarizes via Apple Developer ID (macOS) and Azure
Trusted Signing (Windows). Auto-updater pinned to
`mizan.app/updates/latest.json`.

## Workspace layout

- `apps/tauri/` — Tauri 2 main binary, IPC commands, scheduler, updater
- `apps/frontend/` — React + Vite WebView frontend
- `apps/server/` — embedded dev server for HMR + connect API mocks
- `crates/ai/` — agent runtime, tool dispatcher, AI Safety Runtime
- `crates/connect/` — Mizan Connect client adapter (auth, billing, sync)
- `crates/core/` — domain types, holdings/activities/accounts logic
- `crates/device-sync/` — E2EE device sync primitives
- `crates/market-data/` — Twelve Data / MetalpriceAPI / Yahoo / TradingView adapters
- `crates/storage-sqlite/` — typed repositories per aggregate, migrations
- `packages/ui/` — shared shadcn primitives
- `packages/addon-sdk/` — addon framework
- `migrations/` — SQLite migrations (forward-only, crash-safe DDL)
- `e2e/` — Playwright end-to-end tests
- `addons/` — first-party addons (goal-progress-tracker, investment-fees, swingfolio)

Track H plans to extract `financial-truth`, `zakat`, `insights`, `synthesis`,
`csv-import` as their own crates so the 95% coverage floor + mutation
testing + two-reviewer rules are enforceable in CI. See ADRs 0002–0006
(planned) and `docs/plans/00-master-plan.md` Track H.

## Build status

The desktop ships on the v3 phase ordering from `@../MIZAN_AI_NATIVE_PLAN.md`.
Public production posture per
`@../docs/working-agreement.md` Section 1 — the
"Apple / Netflix grade" bar is enforced, not aspirational.

## Module conventions

- `mod.rs` re-exports public surface
- `model.rs` types
- `repository.rs` SQL access
- `service.rs` orchestration above repositories
- `handlers.rs` Tauri IPC commands
- One file per AI tool under `crates/ai/src/tools/`
- One file per insights rule under `crates/insights/src/rules/` (once extracted)

## Architecture invariants (NEVER violate)

These complement (don't replace) the six absolute rules in
`@../docs/working-agreement.md` §0:

1. **Truth Ledger writes are atomic.** Every write to balances or holdings emits a `prev_hash || event_payload → blake3 → curr_hash` chain entry. Verified by golden test QA-P2.4.
2. **No silent FX fallbacks.** Every cross-currency op reads `fx_rates` explicitly with timestamp. `?? 1.0` and `unwrap_or(1.0)` are clippy-banned (QA Pass 8).
3. **Decimal for money.** `rust_decimal::Decimal` only, never `f64`. CI lints money paths (QA Pass 4 lesson).
4. **No `unwrap()` / `expect()` in service write paths.** Apple-5 pass eliminated these. Don't reintroduce.
5. **Zero `println!` / `eprintln!`.** Use `tracing::{info,warn,error,debug}!`.
6. **Datetimes:** `time::OffsetDateTime` or `chrono::DateTime<Utc>` consistently per crate. The 5-format date parser in taxonomies (QA Pass 3) handles broker-CSV variance.
7. **Secrets:** `secrecy::SecretString` wraps every env-loaded secret. Never log.
8. **AI Safety Runtime:** every tool registered in `crates/ai/src/dispatcher.rs` declares per-turn cap weight + audit scope + numeric bounds + Truth Ledger emission flag. Missing any → compile error.
9. **MCP sandbox:** when Track K ships, MCP-namespaced tools route through the read-mostly gate. Mutations to financial state from MCP are absolute compile-time rejection.
10. **Mizan Badge everywhere:** no holding / account / amount renders in UI without a Badge attached. No "compact density mode" without provenance.

## Required env vars (desktop has fewer than Connect)

| Var | Required when | Notes |
|---|---|---|
| `MIZAN_INSTANCE_ID` | always | UUID v4 unique per install, generated on first run |
| `MIZAN_CONNECT_URL` | when cloud features enabled | `https://mizan-connect.fly.dev` in prod |
| `TWELVE_DATA_API_KEY` | for stocks/ETF/FX/crypto data | passed through Connect for cloud-mode users |
| `METALPRICEAPI_KEY` | for Zakat gold/silver | similar |
| `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` | only for BYO-AI Free-tier users | Mizan AI managed users hit Connect |

Cloud-mode users never see these env vars directly — they go through Mizan Connect.

## Things to know operationally

- **Tauri updater dev short-circuit:** `cfg(debug_assertions)` in `apps/tauri/src/updater.rs` skips update check in `tauri dev` (avoids 30s IPC timeout against staging endpoint).
- **SQLCipher option:** users can opt-in to SQLCipher; default relies on OS FileVault/BitLocker.
- **Truth Ledger retry queue:** crash-safe writes via the Hardening 6 retry queue. If a Tauri command panic'd mid-write, the queue replays on next startup.
- **Frontend strict TS:** `noImplicitAny`, `noUncheckedIndexedAccess` enabled. CI gates.
- **shadcn primitive font features:** `tabular-nums` + `tracking-tight` baked into every monetary primitive per QA Pass UX-15/16. Don't override.

## Conventions

- **API versioning** — no public API; this is a desktop binary. Tauri IPC commands have versioned request/response types in a shared crate (planned Track I `ipc-schema` crate).
- **Migrations** — `migrations/NNNN_kebab_case.sql`, forward-only, each carries a `-- caches-evicted:` manifest comment (per Track I plan).
- **Logs** — structured `tracing` to local file (rotated 7d). Available via Support Diagnostic Bundle export (§A17).
- **Tests** — `cargo test --workspace`, Playwright E2E in `e2e/`. 14 critical-path E2E tests today; do not let this count drop.

## Common commands

```bash
pnpm tauri dev                                     # dev shell
pnpm tauri build --target aarch64-apple-darwin     # release DMG (mac)
pnpm tauri build --target x86_64-pc-windows-msvc   # release MSI (windows)
cargo test --workspace                             # all rust tests
cargo clippy --workspace --all-targets -- -D warnings  # zero warnings or fail
cargo fmt --all -- --check                         # CI gate
pnpm --filter frontend type-check                  # TS strict
pnpm --filter frontend lint                        # eslint
pnpm --filter frontend test -- --run               # vitest
pnpm exec playwright test --reporter=line          # E2E
```

## When adding code, FIRST consult these skills

Skills live at `@../.claude/skills/`. Relevant for desktop work:

- New SQLite migration → `mizan-migration-author`
- New tier-gated capability → `mizan-tier-gate`
- New AI tool → `mizan-ai-tool-author` + `ai-truth-contract`
- Feroz invariants check after dashboard / portfolio / asset class changes → `feroz-invariants-check`
- DraftActionGraph correctness → `mizan-action-graph-validator`
- Clean rebuild after deep changes → `mizan-clean-rebuild`
- Pre-commit / pre-PR → `mizan-pr-checklist`

## Things to ASK the user about (don't guess)

- Adding new dependencies (especially anything copyleft into desktop given AGPL → "viral" implications for any compiled-in code)
- Database schema changes that aren't pure additions
- Anything touching encryption, key derivation, or the AI Safety Runtime
- New asset class introduction
- Splitting / renaming a workspace member crate

## Off-limits without explicit approval

- Adding GPL/copyleft Rust dependencies (would force the entire desktop binary AGPL-incompatible — confirm license compatibility before adding)
- Changing the AI provider abstraction
- Bypassing the AI Safety Runtime for "just one tool"
- Writing to `truth_ledger` from any code path that isn't an authenticated, audit-logged service method
- Storing brokerage / Plaid / SnapTrade tokens on disk (they live in Mizan Connect Postgres only)
- Storing private keys, seed phrases, or full card numbers (CLAUDE.md bright line)

## Related

- `@../CLAUDE.md` — repo root operational manual
- `@../docs/working-agreement.md` — coding contract (binding)
- `@../MIZAN_AI_NATIVE_PLAN.md` — product spec (binding)
- `@../mizan-connect/CLAUDE.md` — backend manual
- `@../docs/plans/00-master-plan.md` — current execution plan
- `@../docs/adr/` — architecture decision records
