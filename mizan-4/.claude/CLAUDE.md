# mizan-4 — Desktop sub-product

This file scopes to `mizan-ai-native/mizan-4/`. Root operating manual,
binding spec, AI safety contract, and tier model live in the monorepo
root (`@../CLAUDE.md` and `@../MIZAN_AI_NATIVE_PLAN.md`). Read those first.

## What this sub-product is

Tauri 2 desktop app — macOS + Windows. React + Vite frontend, Rust
workspace backend, local SQLite (SQLCipher in v3 §10), conversational
AI as the front door, manual entry first-class for non-supported regions.

## Module map

- `apps/frontend/` — React app (pages, components, commands, hooks).
- `apps/tauri/` — Tauri shell + IPC commands.
- `apps/server/` — Axum HTTP server (web mode; behind feature flag).
- `crates/core/` — domain (accounts, assets, liabilities, holdings,
  activities, goals, net worth, quotes, health, onboarding).
- `crates/ai/` — rig-core providers, system prompt, tools, intent.
- `crates/storage-sqlite/` — Diesel schema + migrations.
- `crates/market-data/` — Yahoo, TradingView, Alpha Vantage, Finnhub,
  MarketData.app providers + fallback chain.
- `crates/connect/` — Mizan Connect adapter (token, entitlements,
  AI proxy, broker sync, device sync).
- `crates/device-sync/` — E2EE sync engine.
- `packages/` — shared TS packages (ui, addon-sdk, addon-dev-tools).

## Critical files (v3 §16)

Re-use these, don't recreate:

- **AI runtime**:
  - `crates/ai/src/system_prompt.txt`
  - `crates/ai/src/tools/` (one Rust file per write tool)
  - `crates/ai/src/intent/` (FinancialIntentPlan + DraftActionGraph)
  - `apps/frontend/src/features/ai-assistant/components/tool-uis/` (one
    React confirm card per tool)
  - `apps/frontend/src/features/ai-assistant/hooks/use-chat-runtime.ts`
- **Capabilities / tier gates**:
  - `apps/frontend/src/domain/account/capabilities.ts`
  - `crates/connect/src/entitlements.rs`
- **Core services** (Phase B tools wrap these):
  - `crates/core/src/accounts/accounts_service.rs`
  - `crates/core/src/assets/alternative_assets_service.rs`
  - `crates/core/src/goals/goals_service.rs`
  - `crates/core/src/activities/activities_service.rs`
- **Onboarding + example seed**:
  - `apps/frontend/src/pages/onboarding/`
  - `crates/core/src/onboarding/seed_examples.rs` (new in Phase C)
- **Quotes + health**:
  - `crates/core/src/quotes/sync.rs`
  - `crates/core/src/health/checks/`
  - `apps/frontend/src/components/ticker-conveyor/`

## Feroz invariants (v3 §8)

Binding for any dashboard/portfolio/asset-class/holding/liability/net-
worth/goals/onboarding change. Pointer + full list in root CLAUDE.md and
v3 §8. Use the `feroz-invariants-check` skill after each such change.

## What's already built (v3 §10)

- M1 + M1.5 entitlements (`gated()` IPC + Stripe + AI guardrail)
- M2 nav (5 tabs), unified Add-Asset wizard, 3-question onboarding
- M3.1 SSE streaming AI proxy via Mizan Connect
- M3.4 financial news mesh + daily-limit gate
- M3.5 SnapTrade broker sync + per-broker tiles
- M3.6 monthly AI wealth report cron
- M3.7 Zakat math module (moves to Gold tier)
- M4.1 + M4.2 PDF reports + portfolio health math
- M5.1 + M5.2 teams cloud DB + advisor dashboard

The May-24 repair branch fixes (CSV string-IPC, NumberFlow bypass,
eager startup quote sync) are on `samisayyed1/mizan-4` (deprecated) and
must be ported into this monorepo as Phase 0.

## Quick commands

```bash
pnpm tauri dev                                    # dev shell
pnpm tauri build --target aarch64-apple-darwin    # release DMG → ../artifacts/
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter frontend type-check
pnpm --filter frontend lint:quiet
pnpm --filter frontend test -- --run
pnpm --filter frontend build
```

`.claude/settings.json` (root) wires a `PostToolUse` hook that runs
`rustfmt` after Rust edits and `prettier --write` after TS/TSX edits.

## What changed from the pre-pivot plan

- "Accounts" → **Portfolio** rename (Feroz May-17).
- Tier collapse: Free/Basic/Pro/Enterprise → **Free/Silver/Gold**.
- Zakat moves to Gold (was Pro).
- AI is the front door, not the Add-Asset wizard.
- 3 example liabilities seed on first launch (Phase C).
- Mobile dropped from MVP — macOS + Windows only.

If older docs in this folder reference M3/M4/M5 milestones, those are
historical context. v3 is the binding plan.
