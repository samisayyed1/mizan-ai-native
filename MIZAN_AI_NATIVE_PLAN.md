# Mizan AI — End-to-End Plan (v2, 24 May 2026)

> "Two apps. One brain. The brain is the AI."

This document is the operating plan for **Mizan AI Native** — the desktop wealth-tracking app (`mizan-4/`) plus the backend (`mizan-connect/`), wired together so a user can build, edit, and live-sync their entire financial life through conversation.

Version 2 supersedes the v1 published 24 hours earlier. The structural decisions from Uncle Feroz (25 May 2026) and the product pivot from Sami (24 May 2026) are integrated. Anything that contradicts this doc — including v1 — is stale.

---

## 1. The product in one paragraph

Mizan AI is a private, AI-native Muslim wealth operating system. Users talk to it like a smart financial assistant — _"I have a Vanguard taxable account with 100 AAPL and 50 MSFT, plus a house worth CA$850k with a $300k mortgage"_ — and the AI drafts the entire financial state, the user confirms, and everything lands in an encrypted local store. From there, the user can connect bank accounts via Plaid (Gold), broker accounts via SnapTrade (Gold), or keep using the AI / manual entry path for institutions neither integration can reach (India, UAE, niche banks). Three plans: **Free** (prices + news + bring-your-own AI key), **Silver** (private + managed AI + manual + unlimited holdings), **Gold** (live bank/broker sync + monitoring + Zakat + AI wealth summaries).

The product target is fixed: a beautiful, boomer-friendly, AI-conducted experience that handles real money seriously, never invents data, and respects Muslim financial principles by default.

---

## 2. Architecture

```
samisayyed1/mizan-ai-native  ← monorepo, active dev
├── mizan-4/             desktop (Tauri 2 + React + Rust workspace)
│   ├── apps/frontend       React + Vite (assistant-ui, shadcn, Flexoki)
│   ├── apps/tauri          Tauri commands, scheduler, secrets
│   ├── crates/ai           rig (OpenAI/Anthropic) + tool registry
│   ├── crates/core         portfolio, activities, quotes, zakat, goals
│   ├── crates/connect      Stripe + entitlements + SnapTrade + Plaid client
│   ├── crates/storage-sqlite encrypted local DB
│   └── crates/market-data  Yahoo + TradingView + AlphaVantage providers
├── mizan-connect/       backend (Axum on Fly.io)
│   ├── src/auth         Supabase JWT extractor
│   ├── src/billing      Stripe checkout / portal / webhook (ES256-verified)
│   ├── src/plaid        Plaid Link + token exchange + webhook + sync
│   ├── src/snaptrade    SnapTrade portal + callback + sync
│   ├── src/teams        teams + invites (paused, kept for Gold advisors)
│   └── migrations/      forward-only SQL (00xx)
├── artifacts/           release DMGs (unsigned for now)
├── MIZAN_AI_NATIVE_PLAN.md  ← this doc
└── .github/workflows/
    ├── ci.yml                  path-filtered: mizan-4 frontend, mizan-4 rust, mizan-connect
    ├── deploy-mizan-connect.yml  Fly redeploys on main push
    └── release-desktop.yml       Tauri DMG build + artifact upload
```

**Live now**

- `mizan-connect.fly.dev` — `/health` 200, Plaid sandbox + SnapTrade sandbox configured, webhooks signature-verified, audit-logged.
- `mizan-connect-db` — Postgres on Supabase, migrations 0001–0008 applied.
- Desktop DMG (`Mizan AI_3.4.1_aarch64.dmg`) — unsigned, installable.

**Deprecated, kept on GitHub as reference only**

- `samisayyed1/mizan-4` standalone repo — has the entire M1→M5 history. README marked deprecated; no new commits.
- Local `~/Documents/mizan-4` — deleted (all useful fixes ported into the monorepo before deletion).

**Stack notes**

- AI runtime: `rig` crate (Rust) for OpenAI / Anthropic, with a custom tool registry. System prompt at `mizan-4/crates/ai/src/system_prompt.txt`.
- Tool UI runtime: `@assistant-ui/react` with `useExternalStoreRuntime`. Tool registry at `mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/index.ts`.
- Local DB: SQLite encrypted on disk. Schema at `mizan-4/crates/storage-sqlite/src/schema.rs`.

---

## 3. Tier matrix (the new 3-tier model)

| Capability                           | Free | Silver | Gold |
| ------------------------------------ | :--: | :----: | :--: |
| Live prices (Yahoo + TradingView)    |  ✅  |   ✅   |  ✅  |
| News feed (RSS mesh)                 |  ✅  |   ✅   |  ✅  |
| BYO AI key (OpenAI / Anthropic)      |  ✅  |   ✅   |  ✅  |
| Encrypted local storage              |  ✅  |   ✅   |  ✅  |
| Manual asset / liability entry       |  ✅  |   ✅   |  ✅  |
| 1 portfolio, 20 holdings cap         |  ✅  |        |      |
| Unlimited portfolios + holdings      |      |   ✅   |  ✅  |
| Managed AI (Mizan Connect proxy)     |      |   ✅   |  ✅  |
| AI write tools (conversational CRUD) |      |   ✅   |  ✅  |
| CSV / file ingest                    |      |   ✅   |  ✅  |
| Alternative assets (property, gold…) |      |   ✅   |  ✅  |
| Balance masking                      |      |   ✅   |  ✅  |
| Plaid bank sync                      |      |        |  ✅  |
| SnapTrade broker sync                |      |        |  ✅  |
| Zakat & purification engine          |      |        |  ✅  |
| Live liability tracking              |      |        |  ✅  |
| Background portfolio monitoring      |      |        |  ✅  |
| Allocation drift + cash-drag detect  |      |        |  ✅  |
| Weekly AI wealth summaries           |      |        |  ✅  |
| Proactive alerts                     |      |        |  ✅  |

**Free is the conversion funnel.** A user lands, sees real market prices + news + can chat with the AI using their own OpenAI/Anthropic key — limited to 1 portfolio with 20 holdings, manual entry only. When they outgrow it, Silver removes the caps + adds managed AI (we pay the OpenAI bill). Gold unlocks live sync + monitoring + Zakat.

Stripe stays — three Prices: Silver monthly/yearly, Gold monthly/yearly. Free tier requires no Stripe customer record until upgrade.

The old M1.5 4-tier model (Free/Basic/Pro/Enterprise) collapses to this 3-tier model via a cloud-side migration in `mizan-connect/migrations/0009_tier_collapse.sql`: Basic→Silver, Pro→Silver, Enterprise→Gold. The desktop's entitlements hook handles both shapes during the transition.

---

## 4. The AI-native target workflow

This is what a fresh Silver-tier install should feel like:

1. **Onboarding** — 3 steps (welcome, base currency, appearance). No "add your first account" gate. The user lands on the dashboard.
2. **Empty dashboard isn't empty** — three example liabilities are pre-seeded (mortgage, student loan, credit card), each marked `metadata.example = true` and name prefixed `"Example — "`. The "Net Worth" tile shows real math against those examples. The Assistant icon pulses gently.
3. **User opens the Assistant** — types or speaks: _"Hey, I have a Schwab brokerage with 200 AAPL averaging $150 cost basis, and an HDFC India savings account with about ₹450,000."_
4. **AI drafts** — calls `create_account` (Schwab USD brokerage), `record_activity` BUY 200 AAPL, `create_account` (HDFC INR cash), `record_activity` DEPOSIT ₹450,000. Each draft renders inline as a confirm card with editable fields.
5. **User edits + confirms** — single click per draft. State changes are atomic and reversible.
6. **User says** _"Replace the mortgage example with my actual: $480k principal, 5.2% fixed, $2,650/month, started Jan 2023."_
7. **AI calls `update_liability`** on the existing example row — doesn't create a new one. _(Edit-first UX, Feroz's principle.)_
8. **Five minutes later, the portfolio is live.** No forms touched.

The legacy forms (M2.2 Add-Asset wizard) stay accessible for power users — but the AI is the front door and the most pleasant path for everything.

**Free-tier variant**: identical flow, BUT step 3 hits a "Connect your OpenAI key in Settings" gate. Once a key is connected, the BYO-API runtime drives the conversation. Steps 4–7 work the same; managed-AI features (file ingest, AI wealth report, monthly summaries) stay locked.

---

## 5. Feroz May-17 invariants (still binding)

Every change in this plan must respect these. They are the structural moves agreed with Uncle Feroz on 17 May 2026 and have not been revisited:

1. "Accounts" is renamed to **Portfolio** everywhere.
2. The dashboard shows portfolios, goals, net worth, and a consolidated graph — **not** holdings.
3. New hierarchy: `Dashboard → Portfolio → Asset Class → Holdings`.
4. Portfolios are multi-currency containers; user picks currency per portfolio.
5. **Bank Accounts is an asset class** (each bank = a holding; multi-currency per bank → separate holdings).
6. **Vehicles excluded** from net worth (depreciating).
7. **Liabilities section** required: type, current balance, balance date, origination date, duration, optional rate. **EMI is the monthly payment, NOT the liability.**
8. **Primary / master dashboard currency** lives in Settings.
9. **Custom goals** alongside Retirement/Education/Home/Savings/Wedding; goals can link to one-or-many portfolios.
10. **Dummy data for every asset class** seeded before any soft launch.

Plus the 25 May follow-up:

11. **Zakat moves to Gold** (it's more complex than the Silver tier should bear).
12. **AI-native is the moat** — push hard.
13. **Manual entry must be first-class alongside Plaid** — Indian banks (and many outside US/CA) don't expose Plaid-grade live data.
14. **Seed 3 example liabilities on first launch** — editing beats blank-staring.
15. **Edit-first UX** over blank-form UX.

---

## 6. What's already built (don't rebuild)

The mizan-4 standalone repo shipped a lot of M1–M5 work in May. Most of it ports cleanly into the monorepo's `mizan-4/`.

**Keep + use**

- M1 / M1.5 entitlements engine, Stripe Checkout, Customer Portal, idempotent webhooks, `/v1/me`, AI proxy, usage ledger.
- M2 five-tab nav, unified Add-Asset wizard (deemphasized but accessible), 3-question onboarding, settings regroup, jargon cleanup.
- M3.1 SSE streaming `/v1/ai/chat` + OpenAI-compatible alias + cloud `usage_ledger` + rig-core desktop transport.
- M3.4 financial news mesh + daily-limit gate (move daily-limit to Free tier).
- M3.5 SnapTrade broker sync + per-broker tiles + last-synced badges _(Gold-only)._
- M3.6 monthly AI wealth report cron + report storage _(Gold-only)._
- M3.7 Zakat math module _(Gold-only — already gated in the new tier matrix)._
- M4.1 / M4.2 PDF reports infrastructure + 4 templates + amortization + portfolio-health math _(Gold-only)._
- M5.1 / M5.2 teams schema + advisor dashboard _(stays for Gold advisors)._

**Critical fixes from the 24-May repair pass** (must be in the monorepo before any AI-tool work):

- CSV string-IPC parse (`file.text()` → `parse_csv_text` Rust command). 5 MB CSV no longer hangs the browser.
- Eager startup quote sync in `apps/tauri/src/scheduler.rs::run_startup_quote_sync` (drops the 120 s initial delay).
- Background post-import quote sync in `crates/core/src/activities/activities_service.rs` (import returns in ms, not seconds).
- `warn!`-level logging on quote provider failures so silent rate-limits surface in the Health Center.
- `NumberFlow` bypass in dashboard/balance.tsx, gain-amount.tsx, gain-percent.tsx, performance-page.tsx (custom-element race in Tauri webview was rendering the raw 0–9 digit reel).
- Confirm-step silent-failure surfacing — backend `success: false` now displays the actual error message on the import confirm step.

**Drop / pause**

- The 4-tier Free/Basic/Pro/Enterprise Stripe model → collapses to Free/Silver/Gold.
- The "Add Manually" wizard as the front door — wizard still works, just not the recommended path. Onboarding never points there.
- M5.3 invites, M5.4 white-label branding, M5.5 audit log surface, M5.6 per-seat billing — paused until first 100 Gold signups.
- M4.5 optional structured DB tables — already deferred.

---

## 7. What's missing — the precise gap

### 7.1 AI tools — read complete, write incomplete

| Tool                        | Read | Draft Write | Status                                   |
| --------------------------- | :--: | :---------: | :--------------------------------------- |
| `get_accounts`              |  ✅  |             | exists                                   |
| `get_holdings`              |  ✅  |             | exists                                   |
| `search_activities`         |  ✅  |             | exists                                   |
| `get_allocation`            |  ✅  |             | exists                                   |
| `get_valuation_history`     |  ✅  |             | exists                                   |
| `get_performance`           |  ✅  |             | exists                                   |
| `get_income`                |  ✅  |             | exists                                   |
| `get_goals`                 |  ✅  |             | exists                                   |
| `get_cash_balances`         |  ✅  |             | exists                                   |
| `record_activity`           |      |     ✅      | exists                                   |
| `record_activities`         |      |     ✅      | exists                                   |
| `import_csv`                |      |     ✅      | exists                                   |
| **`create_account`**        |      |     🆕      | Phase B                                  |
| **`update_account`**        |      |     🆕      | Phase B (edit-first)                     |
| **`add_alternative_asset`** |      |     🆕      | Phase B                                  |
| **`create_liability`**      |      |     🆕      | Phase B                                  |
| **`update_liability`**      |      |     🆕      | Phase B (edit-first — "replace example") |
| **`create_goal`**           |      |     🆕      | Phase B                                  |

The Rust core services for all six new tools already exist (`accounts_service`, `AlternativeAssetService`, `goals_service`). The work is the thin AI-tool wrapper + tool-UI card + system-prompt entry per tool.

### 7.2 No seed data on first launch

Currently a fresh Silver user lands on a fully empty dashboard. Per Feroz: pre-seed 3 example liabilities. New service: `crates/core/src/onboarding/seed_examples.rs`. Idempotent; runs once when onboarding completes and no liabilities exist. Each row tagged `metadata.example = true`, prefixed `"Example — "`. Frontend renders these with a soft amber border + "Tap to edit". First edit strips the prefix and clears `example: true`.

Rows:

| Liability     | Principal | Rate   | Monthly  | Originated |
| ------------- | --------- | ------ | -------- | ---------- |
| Home mortgage | $480,000  | 5.2%   | $2,650   | 2023-01-15 |
| Student loan  | $32,000   | 6.8%   | $410     | 2019-09-01 |
| Credit card   | $4,800    | 22.99% | $145 min | 2024-03-01 |

### 7.3 Manual entry isn't first-class

The Rust core already supports manual accounts (`provider = null` or `"MANUAL"`). The frontend has a manual account form but doesn't surface it well. Gaps:

- AI-native creation — when the user says _"I have an HDFC India account with ₹4.5L cash"_, the AI calls `create_account` directly (Phase B).
- "Manual" pill on manual account cards (mirror Plaid "Live" pill).
- Quick "Update Balance" affordance on manual accounts — single-field modal that writes a cash-balance snapshot or DEPOSIT activity.

### 7.4 The Free tier doesn't exist yet

Today's monorepo entitlements are Silver/Gold. Need to add the Free row to the capability matrix, expose Yahoo + TradingView + news as no-auth no-Stripe features, surface a BYO-API onboarding flow, and gate everything Silver-only (managed AI, CSV import, unlimited holdings) behind the existing entitlements machinery.

---

## 8. Implementation plan

Seven phases. Each ends in `cargo + pnpm` verification + a single atomic commit. No phase ships half-done.

### Phase 0 — Cleanup + repair port (today)

- Confirm `mizan-ai-native/mizan-4` has all the 24-May repair commits (CSV string-IPC, startup quote sync, NumberFlow bypass, warn-level logs, confirm-step error surfacing). Port any that are missing by hand.
- Delete `~/Documents/mizan-4`.
- Mark `samisayyed1/mizan-4` GitHub README deprecated, point to monorepo.
- Run `cargo check`, `pnpm type-check`, `pnpm build` in the monorepo to confirm the baseline compiles.

Effort: 1–2 h.

### Phase A — Move Zakat to Gold

- `apps/frontend/src/domain/account/capabilities.ts` — move `zakatEngine: true` out of `BASE_CAPABILITIES` to Gold-only.
- `apps/tauri/src/commands/zakat.rs` — gate via `zakatEngine` capability (currently `advanced_reports`).
- `apps/frontend/src/routes.tsx` — capability guard on `<Route path="/zakat">`.
- `apps/frontend/src/pages/dashboard/zakat-card.tsx` — upgrade-CTA variant when `!canUseCapability(tier, "zakatEngine")`.
- `apps/frontend/src/pages/zakat/zakat-page.tsx` already labels "Gold feature" ✓.

Verify: `pnpm --filter frontend type-check && lint:quiet && test -- --run`. Manual: Silver user sees Zakat as locked Gold feature on the dashboard.

Effort: 1 h.

### Phase B — Six AI write tools

Mirror existing `record_activity.rs` pattern. Each tool: Rust tool + Tauri command if needed + React tool-UI card (`draft → confirm`) + register in `tool-uis/index.ts` + system-prompt example.

1. **`create_account`** → `accounts_service.create_account`. Args: name, account_type, currency, [is_default].
2. **`update_account`** → `accounts_service.update_account`. For "replace the example" flow.
3. **`add_alternative_asset`** → `AlternativeAssetService.create_alternative_asset`. Kind: `Property | Collectible | Precious | Other`. Args vary by kind.
4. **`create_liability`** → same service, `kind: Liability`. Args: liability_type, principal, currency, rate, monthly_payment, linked_asset_id.
5. **`update_liability`** → updates by id. Critical for "replace example mortgage".
6. **`create_goal`** → `goals_service.create_goal`. Args: title, target_amount, currency, target_date, [linked_account_id].

For each:

- Rust tool: `crates/ai/src/tools/<name>.rs` + register in `tools/mod.rs`.
- Tauri command (if needed): `apps/tauri/src/commands/ai_tools.rs`.
- React tool UI: `apps/frontend/src/features/ai-assistant/components/tool-uis/<name>-tool-ui.tsx`.
- Register in `tool-uis/index.ts`.
- Tests: round-trip schema parse, happy path, validation error.

**System prompt updates** in `crates/ai/src/system_prompt.txt`:

- Add the six new tools with examples.
- New "PORTFOLIO*BUILDING" section: "Before creating, check if an `Example — …` row exists. If a user description matches an example, call `update*\_`not`create\_\_`."
- Non-Plaid country fallback: "If the user mentions a bank outside US/CA (India, UK, UAE, …), do not suggest Plaid. Quietly call `create_account` and `record_activity`."
- Tool-call ordering: prefer one `record_activities` batch over many `record_activity` calls.

Verify: `cargo test -p mizan-ai && pnpm --filter frontend type-check && test -- --run`. Manual: install fresh, say _"I have a Vanguard taxable account with 100 AAPL"_, confirm draft, see Account + Holding appear.

Effort: 4–6 h.

### Phase C — Seed 3 example liabilities + edit-first UX

- New `crates/core/src/onboarding/seed_examples.rs`:

  ```rust
  pub async fn seed_example_liabilities(
      service: &dyn AlternativeAssetService,
  ) -> Result<()> {
      if has_any_liability(service).await? {
          return Ok(());
      }
      // 3 inserts: mortgage, student loan, credit card.
      // metadata.example = true, name prefix "Example — ".
  }
  ```

- Call site: end of onboarding step 3, after settings persist, before navigation to `/`.
- Frontend renders example rows with soft amber border + "Tap to edit" hint (`apps/frontend/src/pages/asset/linked-liabilities-card.tsx` + liabilities list). First edit strips `"Example — "` prefix and clears `example: true`.

Verify: wipe local DB, complete onboarding, see 3 example liabilities. Open Assistant, say _"Update the mortgage example to my real one: $620k principal"_ — AI calls `update_liability` on the existing row.

Effort: 2 h.

### Phase D — Manual-entry parity with Plaid

1. **Badge component** — small "Manual" pill on manual accounts (mirror Plaid "Live" pill on synced accounts). Location: `apps/frontend/src/pages/settings/accounts/components/account-item.tsx`.
2. **Update Balance affordance** — quick-action button on manual accounts that opens a single-field modal (_"New balance: \_\_\_ as of [today]"_). Persists as a cash-balance snapshot or DEPOSIT activity depending on account type.
3. **AI fallback heuristic** — Phase B system prompt already covers non-Plaid countries.

Verify: create a manual HDFC INR account via Assistant, verify it shows in account list with "Manual" badge, "Update Balance" works.

Effort: 2 h.

### Phase E — Free tier + market-data split

- **Capabilities** — add Free row:

  ```ts
  Free: {
    liveQuotes: true,         // Yahoo + TradingView
    news: true,
    byoAi: true,
    manualEntry: true,
    maxPortfolios: 1,
    maxHoldings: 20,
    managedAi: false,
    csvImport: false,
    plaid: false,
    snapTrade: false,
    zakatEngine: false,
  }
  ```

- **Stripe** — three Prices (Silver mo/yr, Gold mo/yr); no Stripe customer until upgrade. Add `mizan-connect/migrations/0009_tier_collapse.sql`: Basic→Silver, Pro→Silver, Enterprise→Gold for existing test subs.
- **`/v1/me` response** — return the new tier names; desktop's entitlements hook handles both shapes during the transition.
- **Market data**:
  - Free: Yahoo + TradingView (desktop-local providers).
  - Silver / Gold: same providers locally for now; cloud proxy to a paid provider (Polygon / IEX) is a post-launch hardening pass.
  - Free-tier daily news limit reuses the M3.4 gate.

Verify: signed-out user sees ticker + news but is blocked on managed AI / CSV import. After Stripe Checkout (Silver), managed AI works.

Effort: 3–4 h.

### Phase F — Database bug investigation (if needed)

Repro from Sami if any DB regressions show up after the monorepo merge. Most likely candidates:

- Migration 0008 (`plaid_sync_throttle.sql`) running against a DB with stale SnapTrade state.
- Local SQLite migration drift in `crates/storage-sqlite/src/schema.rs` from previously-installed DMGs.
- `provider` column default racing with new account creation.

Effort: 1–3 h.

### Phase G — Verify + rebuild + push

- `cargo check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --lib` (both `mizan-connect/` and `mizan-4/` workspaces).
- `pnpm --filter frontend type-check && lint:quiet && test -- --run && build` in `mizan-4/`.
- `pnpm tauri build --target aarch64-apple-darwin` → DMG into `artifacts/`.
- `git commit && git push` → CI runs path-filtered jobs; `deploy-mizan-connect.yml` redeploys Fly automatically if backend changed.
- Walk the §4 target experience end-to-end on the unsigned DMG.

Effort: 1 h.

---

## 9. Timeline

| Phase | Description                             | Effort  |
| :---: | --------------------------------------- | :-----: |
|   0   | Cleanup + repair port                   |  1–2 h  |
|   A   | Move zakat → Gold                       |   1 h   |
|   B   | Six AI write tools + tool UIs + prompt  |  4–6 h  |
|   C   | Seed 3 example liabilities + edit-first |   2 h   |
|   D   | Manual-entry parity                     |   2 h   |
|   E   | Free tier + market-data split           |  3–4 h  |
|   F   | DB bug investigation (if needed)        |  1–3 h  |
|   G   | Verify + DMG rebuild + push             |   1 h   |
|       | **Total**                               | 15–21 h |

≈ 2–3 focused days. 4 calendar days with sleep, prayer, and inevitable surprises.

---

## 10. Definition of done

Mizan AI is "AI-native end to end" when a brand-new install can do this in under 5 minutes without touching a single legacy form:

1. Launch the DMG, see Mizan AI in Applications.
2. Onboard (3 steps).
3. See 3 example liabilities with real-feeling numbers.
4. Open the Assistant, type a free-form description of the user's actual portfolio (one or more accounts, one or more holdings, optionally an alt asset and a goal). Free users provide a BYO API key first; Silver+ uses managed AI.
5. Confirm each draft (≤ 4 clicks).
6. Edit the example liabilities into real ones via the AI.
7. Land back on the dashboard with a fully populated, real Net Worth tile.
8. Optionally upgrade to Gold and connect Plaid sandbox (US) or SnapTrade sandbox (broker). For non-US users, keep going manually.
9. Optionally ask the AI: _"What's my zakat estimate?"_ → Gold-tier upgrade modal if Silver, real computation if Gold.

Every step above must work on the unsigned DMG, on macOS arm64, against `mizan-connect.fly.dev`, with sandbox Plaid + sandbox SnapTrade + the user's own OpenAI key (Free) or our managed AI (Silver/Gold).

Plus every Feroz May-17 invariant holds: Portfolio rename in place, dashboard hierarchy correct, multi-currency portfolios, Bank Accounts is an asset class, vehicles excluded, Liabilities reduce net worth, primary dashboard currency works.

---

## 11. Out of scope (for now)

- Apple Developer ID signing + notarisation (waiting on certs).
- Stripe live mode (test keys only).
- Plaid production credentials (sandbox only).
- SnapTrade production credentials (sandbox only).
- Tauri auto-updater (post-first signed release).
- Mobile (iOS / Android) — future.
- The Yahoo / yfinance dormant code paths in `crates/market-data/src/provider/yahoo/` — not on the active product path; full deletion is a larger refactor.
- M5.3–5.6 (invites / branding / audit log surface / per-seat billing) — paused until first 100 Gold signups.
- A real paid market-data provider (Polygon / IEX) — hardening pass after launch.
- Add-on marketplace, public REST API, multi-language UI — defer indefinitely.
- Marketing site / app icon redesign.

---

## 12. Critical files index

For fast navigation during implementation:

**AI runtime**

- `mizan-4/crates/ai/src/system_prompt.txt`
- `mizan-4/crates/ai/src/tools/`
- `mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/`
- `mizan-4/apps/frontend/src/features/ai-assistant/hooks/use-chat-runtime.ts`

**Capability matrix**

- `mizan-4/apps/frontend/src/domain/account/capabilities.ts`
- `mizan-4/crates/connect/src/entitlements.rs`

**Core services (already exist — Phase B tools wrap these)**

- `mizan-4/crates/core/src/accounts/accounts_service.rs`
- `mizan-4/crates/core/src/assets/alternative_assets_service.rs`
- `mizan-4/crates/core/src/goals/goals_service.rs`

**Plaid integration**

- `mizan-connect/src/plaid/{client,handlers,repository,types,webhook_verifier}.rs`
- `mizan-4/apps/frontend/src/features/mizan-connect/` (Plaid Link UI)

**SnapTrade integration (existing — Gold-only)**

- `mizan-4/crates/connect/src/broker/`
- `mizan-connect/src/snaptrade/` (if/when added — currently lives partly in mizan-4 standalone repo, needs porting if not in monorepo)

**Onboarding**

- `mizan-4/apps/frontend/src/pages/onboarding/`
- new: `mizan-4/crates/core/src/onboarding/seed_examples.rs`

**Liabilities + example rendering**

- `mizan-4/apps/frontend/src/pages/asset/linked-liabilities-card.tsx`
- `mizan-4/apps/frontend/src/pages/asset/alternative-assets/components/alternative-asset-quick-add-modal.tsx`

**Deploy + CI**

- `.github/workflows/ci.yml`
- `.github/workflows/deploy-mizan-connect.yml`
- `.github/workflows/release-desktop.yml`
- `mizan-connect/fly.toml`
- `mizan-connect/Dockerfile`

---

## 13. Open questions

- **Free → Silver upgrade UX** — when a Free user hits a capped flow (CSV import, 21st holding, managed AI), do we show in-app Stripe Checkout or open the browser? Current M1.5 wiring opens the browser. Recommended: keep that for consistency.
- **Free-tier abuse** — what stops a user from creating dozens of free accounts to circumvent the holding cap? Soft: device fingerprint check + email per signup. Hard: ignore for now, revisit if abuse emerges.
- **SnapTrade broker connection cap** — was 5 on the old Pro tier, unlimited on Enterprise. New: Gold = unlimited, no Silver access. Update the M4.3 "N / 5 used" badge to "N / unlimited" (or remove entirely).
- **Plaid international coverage** — Plaid Europe (Tink) and India (none) are out of scope. Confirmed manual entry is the path there.
- **Cloud-side market-data proxy** — defer until Polygon / IEX cost numbers make sense; until then Free, Silver, and Gold all share Yahoo + TradingView. Document the SLA risk in the in-app Health Center.

---

---

# v3.1 Production-Class Addendum (binding)

> **The product thesis under all of this:** Mizan is not a dashboard with AI. Mizan is a local-first financial truth system where every number is source-traceable, every AI write is drafted and confirmed, every provider sync is auditable, and every failure is visible before production credentials are ever introduced.
>
> **Mizan Connect is part of the product, not infrastructure hidden behind it.**

This addendum is binding alongside §1–§13 above. New phases (**O**, **L⁺**, **P0**) slot into the v3 phase order at the points called out in §12.

## §A1. Financial Truth Engine (new Phase O)

Every number in the app must be traceable to source, timestamp, calculation path, confidence.

New module: `mizan-4/crates/core/src/truth/{money,source,valuation,provenance,snapshot,reconciliation,audit}.rs`.

Required types:

- **`Money { amount: Decimal, currency: CurrencyCode, precision: u8 }`** — `rust_decimal` everywhere. Never floats.
- **`ValuationPoint { entity_id, entity_type, value, value_in_master_currency, fx_rate_used, source, as_of, calculated_at, stale }`**.
- **`DataSource`** enum: `ManualUserEntry | PlaidBalance | SnapTradePosition | MarketQuote | DocumentExtraction | AiDraftConfirmedByUser | DerivedCalculation`.
- **`Provenance { source_type, source_id, provider_event_id, user_confirmed, confirmed_at, imported_from_file, confidence: High|Medium|Low }`**.

Killer affordance: "Why does Mizan say my net worth is $1.42M?" → "$812k SnapTrade synced 14 min ago, $212k Plaid cash 2h ago, $480k property manually valued May 21, -$84k liabilities manual, FX USD/CAD 1.37 today."

## §A2. Immutable Ledger + Derived State (new Phase L⁺)

Activities are the ledger. Holdings + balances are derived views unless from live provider snapshots.

New module: `crates/core/src/ledger/{ledger_event,ledger_service,derived_holdings,derived_balances,cost_basis,replay}.rs`.

Event taxonomy: `BUY | SELL | DEPOSIT | WITHDRAWAL | DIVIDEND | INTEREST | FEE | TRANSFER | LIABILITY_CREATED | LIABILITY_PAYMENT | MANUAL_VALUATION_UPDATE | DOCUMENT_EXTRACTED_UPDATE | PROVIDER_SYNC_SNAPSHOT`.

**Production rule:** every AI write tool creates either a confirmed ledger event OR a reversible draft that becomes a ledger event only after user confirmation. AI never mutates a derived balance directly.

## §A3. Reconciliation Engine (extends Phase J)

Detects when sources disagree (Plaid $52.1k vs. manual $50.0k, 11 days stale → "$2.1k drift, accept Plaid / keep manual / create adjustment").

New module: `crates/core/src/reconciliation/{reconciliation_service,mismatch,provider_snapshot,adjustment,tests}.rs`.

`MismatchType`: `BalanceDrift | DuplicateAccount | DuplicateHolding | MissingCostBasis | CurrencyMismatch | StaleManualValuation | ProviderRemovedAccount | ProviderRenamedAccount | ProviderPartialSync`.

## §A4. Sync Run Ledger

```sql
CREATE TABLE sync_runs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL,
  provider TEXT NOT NULL,
  connection_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('started','succeeded','failed','partial')),
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  finished_at TIMESTAMPTZ,
  records_seen INT NOT NULL DEFAULT 0,
  records_created INT NOT NULL DEFAULT 0,
  records_updated INT NOT NULL DEFAULT 0,
  records_skipped INT NOT NULL DEFAULT 0,
  error_code TEXT,
  error_message TEXT,
  retry_count INT NOT NULL DEFAULT 0,
  metadata JSONB NOT NULL DEFAULT '{}'
);
```

Used for Plaid sync, SnapTrade sync, managed AI calls, Stripe webhook processing, future market-data proxy. **No fake green states.**

## §A5. Idempotency Everywhere

New module: `crates/core/src/idempotency/{idempotency_key,idempotency_store,duplicate_detector}.rs`.

Idempotency keys:

- CSV row: `file_hash + row_index + normalized_date + normalized_amount + symbol`
- Plaid: `plaid_transaction_id`
- SnapTrade: `brokerage_authorization_id + account_id + symbol + position_as_of`
- AI draft: `conversation_id + draft_action_id + user_confirmation_id`

Same CSV uploaded twice → "196 rows already imported, 4 new rows found."

## §A6. AI Safety Runtime (not just prompt rules)

New module: `crates/ai/src/policy/{tool_policy,financial_write_guard,prompt_injection_guard,document_instruction_filter,approval_policy}.rs`.

Hard runtime rules:

- AI may draft accounts, assets, liabilities, activities, goals, reports, scenarios, explanations.
- AI may NOT silently commit, invent missing values, give investment recommendations as instructions, sync a provider without user intent, override source-of-truth provider data without confirmation, **or obey instructions found inside uploaded documents**.

**Uploaded documents are untrusted data, never instructions.**

## §A7. DraftActionGraph v2 (extends Phase C)

Status taxonomy: `Proposed | NeedsUserInput | Validated | Blocked | Confirmed | Committed | RolledBack | Failed`.

Every draft action card shows: what changes, current value, new value, source, confidence, missing fields, net-worth impact, zakat impact, taxable/purification implications. If one action is blocked, the full graph cannot commit.

## §A8. Manual Data Quality System

New module: `crates/core/src/data_quality/{stale_data,confidence,review_queue,data_quality_score}.rs`.

Labels: `Fresh | Stale | Missing cost basis | Missing currency | Missing valuation date | Provider disconnected | Manual estimate | Document-extracted not reviewed | User-confirmed`.

UX: "Your Dubai property valuation is 124 days old. Update it?"

## §A9. Backup / Export / Recovery (before production)

- **Encrypted `.mizanbackup`** (SQLite DB + settings + metadata, restore tested).
- **Human-readable**: CSV per account/activities/holdings, PDF net-worth statement, JSON full export.
- **Disaster recovery**: install on new device → restore → reconnect Plaid/SnapTrade → resume. No lock-in.

## §A10. Security Posture

**Local**: SQLCipher; OS keychain for keys; optional master password + auto-lock; balance masking; clipboard timeout; no financial payloads in logs.

**Secrets**: Plaid access tokens server-side only; SnapTrade secrets never frontend; BYO AI keys locally encrypted; managed AI keys server-side only; `.env.production.example` only; CI secret scanning.

**Threat model**: stolen laptop, malicious PDF, compromised provider token, cloud outage, corrupted local DB, duplicate import, failed provider sync, LLM hallucination, accidental user confirmation, rollback need.

## §A11. Local-First Privacy Modes

- **Mode 1 — Local Only**: manual + local SQLite + optional BYO key; no managed AI, no cloud backup.
- **Mode 2 — Redacted Managed AI**: AI proxy with anonymized account names + rounded balances + no raw documents.
- **Mode 3 — Full Managed AI**: richer context, best summaries, explicit consent.

## §A12. Net Worth Snapshot System

New table: `net_worth_snapshots(id, snapshot_date, master_currency, total_assets, total_liabilities, net_worth, liquid_assets, illiquid_assets, zakatable_assets, source_hash, created_at)`.

Historical snapshots preserve the FX/price used at the time. Do not recalculate old net worth with today's FX unless explicitly asked.

## §A13. FX + Market Price Truth Contract

Every quote: symbol, provider, price, currency, quote time, retrieved time, stale flag, failure reason if unavailable.

Every FX rate: base, quote, rate, provider, as-of, usage context.

Display: Fresh → normal; Stale → value + "last updated 2h ago"; Failed → **never silently show old value as live**; Cost-basis fallback → say "Using cost basis fallback".

## §A14. Production Provider Certification (extends P1–P4)

**P1 Stripe** adds: live price IDs, test/live env separation, webhook endpoint separation, billing portal live verification, failed-payment handling, cancellation, downgrade, entitlement cache invalidation, receipts/invoices, tax/VAT.

**P2 Plaid + SnapTrade** adds: provider app review, OAuth redirect URIs, prod webhook verification, outage behavior, reconnect, permission-revoked, account-removed, duplicate connection prevention, sandbox/prod config separation.

**P3 Apple + Windows signing** adds: Apple Developer ID + notarisation + hardened runtime + entitlements; Windows Authenticode + signed installer + update signing + release checksum + rollback strategy.

**P4 Production ops** adds: crash reporting w/ local redaction, no PII telemetry, local diagnostic bundle, support bundle, health status page, rollback release process, versioned migrations, release notes, user-visible provider status.

## §A15. Migration Discipline

**Migration test matrix** — every migration runs against: clean DB, previous-version DB, Plaid DB, SnapTrade DB, manual-only DB, corrupted DB, seeded-examples DB, user-liabilities DB.

**Rollback policy**: restore backup → ship corrective migration. **Never manually edit production DB.**

## §A16. App Health Center v2

User-facing dashboard: Plaid status, SnapTrade status, market-data status, AI provider status, local DB status, backup status, last syncs, last successful AI call, failed imports, migration version, app version, backend version.

## §A17. Support Diagnostic Bundle

"Create Support Bundle" button. Exports: app version, OS version, migration version, recent sync run statuses, redacted logs, provider health, crash trace. **No balances, no account numbers, no tokens, no raw prompts unless user opts in.**

## §A18. Production Test Matrix

QA matrix (binding before production):

- **Account types**: Free signed-out, Free signed-in, Silver, Gold, expired sub, cancelled, downgraded, bad auth token, offline.
- **Data shapes**: no data, seeded examples only, manual-only, Plaid-only, SnapTrade-only, mixed, multi-currency, stale manual, duplicate import, disconnected provider.
- **User flows**: onboarding, AI portfolio creation, edit example liability, CSV import, manual balance update, connect Plaid sandbox, connect SnapTrade sandbox, ask "what changed?", ask Zakat (Silver gate vs Gold compute), export PDF, backup restore.
- **Failure flows**: no internet, provider down, expired token, bad CSV, malformed PDF, AI timeout, market quote timeout, migration failure, disk full, DB locked.

## §A19. Performance Budgets

- **Startup**: shell visible < 2 s, dashboard interactive < 3 s, ticker first sync < 5 s. No blocking provider calls on UI thread.
- **AI**: first token < 2 s on managed AI; draft card render < 1 s after tool result; multi-action plan creation < 5 s.
- **Import**: 50-row CSV preview < 3 s; 500-row preview < 10 s; commit goes background if > 5 s.
- **Sync**: Plaid never blocks UI; SnapTrade shows progress; market data cancellable/retryable.

## §A20. Legal + Compliance Guardrails

Mizan is not a registered investment adviser. AI says: "Here is an analysis / scenario / draft action / consideration. Consult a qualified adviser." AI never says: "You should buy/sell this", guarantees better returns, tax advice, fatwa.

Zakat + purification: show assumptions, cite calculation basis internally, allow user override, label as estimate, recommend scholar/adviser review for complex cases.

## §A21. Document Intelligence Production Rules (extends Phase K)

`DocumentStatus`: `Uploaded | Parsed | Extracted | NeedsReview | Confirmed | Rejected | Archived`.

Extracted facts include: field, value, page, snippet, confidence, suggested action, user confirmation state. **Never silently update financial state from a PDF.**

## §A22. Investor Daily Brief

Dashboard opens with "Here's what changed" — net worth change, top contributors, cash movement, portfolio movement, liability movement, stale/manual data needing review, provider sync issues, zakat/purification alerts, upcoming reminders, documents needing confirmation.

## §A23. "Ask Why" Everywhere

Every card supports a "Why?" affordance: why net worth changed, why this asset is stale, why this account is manual, why Zakat is this amount, why this provider is disconnected, why the AI drafted these 4 actions.

## §A24. Error Message Standard

Every error: what failed, why it likely failed, whether user data is safe, what the user can do next, retry action if possible, diagnostic code.

Bad: "Something went wrong."
Good: "SnapTrade sync failed because Schwab authorization expired. Your existing Mizan data is safe. Reconnect Schwab to refresh positions."

## §A25. Release Gate: "No Silent Failure" Certification

Before every release, manual cert run:

1. Disconnect internet.
2. Break Plaid token.
3. Break SnapTrade token.
4. Break market data.
5. Upload bad CSV.
6. Upload bad PDF.
7. Trigger AI timeout.
8. Remove auth env var.
9. Run stale DB migration.
10. Try duplicate import.
11. Try partial DraftActionGraph failure.

Release passes only if every failure is visible, recoverable, and does not corrupt data.

---

# Mizan Connect — End-to-End Backend Contract (binding)

Mizan Connect is the paid-service backbone of Mizan AI. The desktop app stays local-first and private by default, but Mizan Connect powers everything that requires cloud trust: authentication, billing, entitlements, managed AI, Plaid, SnapTrade, provider webhooks, usage metering, sync status, production operations.

The app and Mizan Connect must feel like one seamless system.

## Mizan Connect owns

- Supabase authentication and session validation.
- `/v1/me` user/tier/entitlement/capability response.
- Stripe Checkout, Billing Portal, subscription state, webhook reconciliation.
- Managed AI proxy for Silver and Gold.
- AI usage ledger and credit enforcement.
- Plaid Link token creation, public-token exchange, access-token storage, webhook handling, account/balance sync, reconnect.
- SnapTrade user registration, portal URL, brokerage callback, account/position sync, reconnect.
- Provider sync run ledger.
- Cloud-side audit log.
- Environment separation (local, staging, sandbox, production).
- Production readiness gates.

## Desktop owns

- Local encrypted financial database.
- Local portfolio/account/holding/liability/activity state.
- Local AI write confirmation cards.
- Local deterministic financial math.
- Local manual entry.
- Local data provenance display.
- Local backup/export.
- User-visible Health Center.
- Secure storage of BYO AI keys.
- Calling Mizan Connect only through typed, authenticated adapters.

## Shared contract — every Connect-backed feature

1. Desktop checks local session.
2. Desktop refreshes Supabase token if needed.
3. Desktop calls Mizan Connect with `Authorization: Bearer <JWT>`.
4. Mizan Connect verifies JWT and maps user to current subscription.
5. Mizan Connect returns typed entitlement/provider state.
6. Desktop updates UI immediately and clearly.
7. Any failure returns a specific typed error.
8. **No feature silently falls back to fake, stale, or mock data** unless explicitly running in a named local mock mode.

If Mizan Connect is down, unreachable, misconfigured, or missing credentials, the desktop says that clearly.

## Credential Provisioning Rule

Before any Mizan Connect work, verify required sandbox/test credentials exist in: repo env, local `.env`, Fly secrets, Supabase settings, Stripe CLI, Plaid dashboard, SnapTrade dashboard.

If any key/URL/secret/callback is missing, **stop and ask Sami for the exact missing item by name**. Never build around missing credentials with fake assumptions. Local mocks only for unit tests. End-to-end validation uses real sandbox/test credentials. Production credentials are locked until the post-MVP production gate passes.

### Required credential request format

> Missing credential needed for end-to-end test: `PLAID_CLIENT_ID`, `PLAID_SECRET`, and `PLAID_ENV=sandbox`. Please provide sandbox values or confirm I should create/update the `.env.example` only.

Never ask vaguely for "the keys."

### Secret handling

- Never commit real secrets.
- Never paste secrets into committed docs.
- Never log secrets.
- Never print tokens in terminal output.
- Never store production secrets in `.env.example`.
- Use `.env.local`, `.env.production.local`, Fly secrets, Supabase dashboard, Stripe dashboard, Plaid dashboard, SnapTrade dashboard, GitHub Actions secrets as appropriate.
- If a secret accidentally appears in git-tracked files, stop immediately and rotate.

## Environment Matrix

| Environment        | Purpose                                | Keys allowed                 | Data allowed           |
| ------------------ | -------------------------------------- | ---------------------------- | ---------------------- |
| Local dev          | Developer testing                      | sandbox/test                 | fake/test              |
| Staging/sandbox    | E2E MVP validation                     | sandbox/test                 | test users + providers |
| Production dry-run | Prod infra without live money movement | limited live, no user launch | internal test          |
| Production         | Real customers                         | live                         | real                   |

**No mixed env**: never mix sandbox Plaid with live Stripe; never mix live providers with test subscriptions; desktop must not point at a backend that doesn't expose `/health` and `/v1/config` with its environment.

## Required env vars

### Core

```env
APP_ENV=local|staging|production
CONNECT_PUBLIC_URL=
DATABASE_URL=
SUPABASE_URL=
SUPABASE_JWT_SECRET=
SUPABASE_SERVICE_ROLE_KEY=
CORS_ALLOWED_ORIGINS=
```

### Stripe

```env
STRIPE_SECRET_KEY=
STRIPE_WEBHOOK_SECRET=
STRIPE_PRICE_SILVER_MONTHLY=
STRIPE_PRICE_SILVER_YEARLY=
STRIPE_PRICE_GOLD_MONTHLY=
STRIPE_PRICE_GOLD_YEARLY=
STRIPE_CUSTOMER_PORTAL_RETURN_URL=
STRIPE_CHECKOUT_SUCCESS_URL=
STRIPE_CHECKOUT_CANCEL_URL=
```

### Plaid

```env
PLAID_CLIENT_ID=
PLAID_SECRET=
PLAID_ENV=sandbox|development|production
PLAID_PRODUCTS=auth,transactions,investments,liabilities
PLAID_COUNTRY_CODES=US,CA
PLAID_REDIRECT_URI=
PLAID_WEBHOOK_URL=
```

### SnapTrade

```env
SNAPTRADE_CLIENT_ID=
SNAPTRADE_CONSUMER_KEY=
SNAPTRADE_ENV=sandbox|production
SNAPTRADE_REDIRECT_URI=
SNAPTRADE_WEBHOOK_SECRET=
```

### Managed AI

```env
OPENAI_API_KEY=
ANTHROPIC_API_KEY=
AI_PROXY_MODE=stateless
AI_LOG_PROMPTS=false
AI_LOG_FINANCIAL_PAYLOADS=false
```

### Email / observability

```env
RESEND_API_KEY=
RESEND_FROM=
LOG_LEVEL=info
REDACT_LOGS=true
SENTRY_DSN=
SENTRY_ENVIRONMENT=
```

## Supabase Contract

Supabase is the source of authenticated user identity, not the owner of local financial truth.

Desktop sign-in flow:

1. User clicks "Sign in to Mizan Connect."
2. Desktop opens Supabase auth flow.
3. User completes login.
4. Auth callback returns to desktop.
5. Desktop stores session securely.
6. Desktop calls `GET /v1/me`.
7. Connect returns user, tier, capabilities, provider connection state.
8. Desktop unlocks Silver/Gold features if entitled.

If Supabase is unavailable: local app still opens, local data accessible, managed AI/billing/Plaid/SnapTrade/cloud sync show unavailable. User sees: "Mizan Connect sign-in is currently unavailable. Your local data is safe."

## `/v1/me` Contract

```json
{
  "user": { "id": "...", "email": "..." },
  "environment": "staging",
  "tier": "Free|Silver|Gold",
  "subscription": {
    "status": "none|trialing|active|past_due|canceled",
    "current_period_end": "...",
    "stripe_customer_id": "..."
  },
  "capabilities": {
    "managedAi": true,
    "aiWriteTools": true,
    "csvImport": true,
    "plaid": false,
    "snapTrade": false,
    "zakatEngine": false,
    "maxPortfolios": null,
    "maxHoldings": null
  },
  "usage": { "aiCreditsUsed": 0, "aiCreditsLimit": 1500 },
  "connections": { "plaid": [], "snapTrade": [] },
  "health": {
    "connect": "ok",
    "stripe": "ok",
    "plaid": "not_configured|ok|degraded",
    "snapTrade": "not_configured|ok|degraded",
    "aiProxy": "ok"
  }
}
```

If `/v1/me` fails, desktop must not pretend the user is Gold. Fall back to local-safe mode and show the specific Connect error.

## Stripe / Plaid / SnapTrade / Managed AI Contracts

- **Stripe** — Checkout/Portal/webhook signature-verified + idempotent. Desktop refreshes `/v1/me` after Stripe return. Free→Silver→Gold + cancellation tested in sandbox before MVP sign-off.
- **Plaid** — Link/exchange/sync/reconnect tested in sandbox. Plaid tokens server-side only. Never proposed for unsupported countries.
- **SnapTrade** — Portal/sync tested in sandbox. `userSecret` encrypted at rest. Removed accounts not silently deleted.
- **Managed AI** — Stateless; no financial-payload logging; no raw-prompt logging in production; usage metered server-side; BYO AI key remains available where allowed.

## Sync Run Ledger

Schema in §A4. Every external provider operation creates a sync run. Desktop Health Center reads summarized status from Connect.

## Admin Diagnostics + Health Endpoints

`/health` — public shallow: `{ status, environment, version }`.

`/v1/health/deep` — authenticated/admin: `{ database, supabase, stripe, plaid, snapTrade, aiProxy, recentFailures }` (no secrets).

## Error Standard

```json
{
  "error": {
    "code": "PLAID_RECONNECT_REQUIRED",
    "message": "Plaid connection requires reconnect.",
    "safeMessage": "Your bank connection needs to be reconnected. Your local Mizan data is safe.",
    "retryable": false,
    "requestId": "..."
  }
}
```

Required families: `AUTH_INVALID_TOKEN`, `AUTH_SESSION_EXPIRED`, `ENTITLEMENT_REQUIRED`, `STRIPE_CHECKOUT_FAILED`, `STRIPE_WEBHOOK_INVALID`, `PLAID_LINK_TOKEN_FAILED`, `PLAID_EXCHANGE_FAILED`, `PLAID_RECONNECT_REQUIRED`, `SNAPTRADE_PORTAL_FAILED`, `SNAPTRADE_SYNC_FAILED`, `AI_PROXY_TIMEOUT`, `AI_PROXY_UPSTREAM_FAILED`, `RATE_LIMITED`, `INTERNAL_ERROR`.

Desktop renders `safeMessage`.

## End-to-End MVP Validation

Before MVP is ready, full path passes on a clean machine:

1. Build desktop with valid Connect env vars.
2. Launch.
3. Sign in through Supabase.
4. Desktop calls `/v1/me`.
5. Free tier appears.
6. Upgrade to Silver via Stripe test Checkout.
7. Webhook updates subscription.
8. Desktop refreshes `/v1/me`.
9. Managed AI unlocks.
10. AI chat works.
11. Upgrade to Gold via Stripe.
12. `/v1/me` returns Gold capabilities.
13. Plaid sandbox Link works.
14. Plaid account appears in desktop.
15. SnapTrade sandbox portal works.
16. Brokerage account appears in desktop.
17. Sync run ledger records both syncs.
18. Health Center shows Connect, Plaid, SnapTrade, Stripe, AI status.
19. Disconnect or break one provider.
20. Desktop shows specific failure, not silent broken UI.
21. User can continue using local/manual data safely.

MVP is not done until this passes.

## Production Gate

Real production credentials may not be added until:

- Phase N validation passes.
- Connect sandbox flow passes end-to-end.
- Stripe test checkout + webhook pass.
- Plaid sandbox link/exchange/webhook pass.
- SnapTrade sandbox portal/sync pass.
- `/v1/me` entitlement contract stable.
- Provider failure states visible in desktop.
- Sync run ledger exists.
- No financial payloads logged.
- Build fails if required Connect env vars missing.
- Credential request protocol in place.
- `.env.example` complete.
- `.gitignore` protects all real env files.
- Fly secrets set only through secret manager.
- Production action hook blocks accidental live deploy/keys unless explicitly overridden.

Only then: P1 live Stripe → P2 live Plaid + SnapTrade → P3 Apple/Windows signing → P4 production ops.

## Rule: Real Sandbox Systems, Not Fake Assumptions

Build against real sandbox/test provider systems whenever possible. If Supabase/Stripe/Plaid/SnapTrade setup is needed, ask Sami for the exact credentials or confirm setup-instructions-only mode. Never mark Plaid/SnapTrade/Stripe/Supabase/managed AI as complete unless the real sandbox/test flow has passed. Unit tests with mocks do not count as end-to-end completion.

---

_Document owner: Sami. Last updated: 2026-05-24. Supersedes v1 + v2. Next review: after Phase G ships._
