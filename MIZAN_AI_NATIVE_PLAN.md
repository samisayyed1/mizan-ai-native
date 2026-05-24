# Mizan AI — End-to-End Plan

> "Two apps. One brain. The brain is the AI."

This document is the operating plan for finishing **Mizan AI Native** as a real product — the desktop wealth-tracking app (`mizan-4`) plus the backend (`mizan-connect`), wired together so a user can build, edit, and live-sync their entire financial life through conversation.

It incorporates the latest direction from Uncle Feroz (25 May 2026):

1. **Zakat moves to Gold** — it's more complex than the Silver tier should bear.
2. **AI-native is the moat** — keep pushing this hard.
3. **Manual entry must be first-class** alongside Plaid — Indian banks (and many others outside the US/CA) don't expose Plaid-grade live data.
4. **Seed 3 dummy liabilities** on first launch — mortgage, student loan, credit card. Editing beats blank-staring.
5. **Edit-first UX over blank-form UX.** The product should feel like polishing examples, not filling in IRS paperwork.

---

## 1. The product in one paragraph

Mizan AI is a private, AI-native Muslim wealth operating system. Users talk to it like a smart financial assistant — "I have a Vanguard taxable account with 100 AAPL and 50 MSFT, plus a house worth CA$850k with a $300k mortgage" — and the AI drafts the entire financial state, the user confirms, and everything lands in an encrypted local store. From there, the user can connect bank accounts via Plaid (Gold tier) for live sync, or keep using the AI / manual entry path for institutions Plaid can't reach. Two plans, both real: **Silver** (private + AI + manual + zakat-free) and **Gold** (live bank sync + monitoring + AI summaries + zakat).

The product target is fixed: a beautiful, boomer-friendly, AI-conducted experience that handles real money seriously, never invents data, and respects Muslim financial principles by default.

---

## 2. Architecture (already standing)

```
┌─────────────────────────────────────────────────────────────────────┐
│  mizan-ai-native  (monorepo on GitHub — samisayyed1/mizan-ai-native) │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────┐    ┌──────────────────────────────┐  │
│  │  mizan-4 (desktop)        │    │  mizan-connect (backend)     │  │
│  │  Tauri + React + Rust     │◄──►│  Axum on Fly.io              │  │
│  │  Encrypted local SQLite   │    │  Postgres on mizan-connect-db│  │
│  │                           │    │                              │  │
│  │  ─ Assistant (OpenAI)     │    │  ─ Plaid Gold sync           │  │
│  │  ─ 9 read AI tools        │    │  ─ Stripe (Silver/Gold)      │  │
│  │  ─ 3 write AI tools (now) │    │  ─ Supabase auth             │  │
│  │  ─ Plaid Link UI          │    │  ─ AI proxy (optional)       │  │
│  │  ─ Zakat / Net Worth /    │    │  ─ Webhook ES256 verified    │  │
│  │    Goals / Activities     │    │  ─ Audit trail               │  │
│  └───────────────────────────┘    └──────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
                ┌───────────────────────┐
                │  GitHub Actions       │
                │  ─ ci.yml             │
                │  ─ deploy-mizan-conn… │
                │  ─ release-desktop    │
                └───────────────────────┘
```

**Live now**

- `mizan-connect.fly.dev` — `/health` 200, Plaid sandbox configured, webhooks signature-verified, audit-logged.
- `mizan-connect-db` — Postgres, 8 migrations applied (0001–0008).
- Desktop DMG (`Mizan AI_3.4.1_aarch64.dmg`, 121 MB) — installable, unsigned.

**Stack notes**

- AI runtime: `rig` crate (Rust) for OpenAI / Anthropic, with custom tool registry. System prompt at [crates/ai/src/system_prompt.txt](mizan-4/crates/ai/src/system_prompt.txt).
- Tool UI runtime: `@assistant-ui/react` with `useExternalStoreRuntime`. Tool registry at [apps/frontend/src/features/ai-assistant/components/tool-uis/index.ts](mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/index.ts).
- Local DB: SQLite (encrypted on disk). Schema at [crates/storage-sqlite/src/schema.rs](mizan-4/crates/storage-sqlite/src/schema.rs).

---

## 3. The Silver vs Gold split (post-Feroz)

| Capability                       | Silver | Gold |
| -------------------------------- | :----: | :--: |
| Private AI Assistant             |   ✅   |  ✅  |
| Encrypted local storage          |   ✅   |  ✅  |
| CSV / file ingestion             |   ✅   |  ✅  |
| Conversational asset creation    |   ✅   |  ✅  |
| Alternative assets (real estate, gold, …) |   ✅   |  ✅  |
| Manual liabilities               |   ✅   |  ✅  |
| Balance masking                  |   ✅   |  ✅  |
| **Zakat & purification engine**  |   ❌   |  ✅  | ← moved per Feroz
| Plaid live sync (banks, brokers) |   ❌   |  ✅  |
| Live liability tracking          |   ❌   |  ✅  |
| Background portfolio monitoring  |   ❌   |  ✅  |
| Allocation drift detection       |   ❌   |  ✅  |
| Cash-drag detection              |   ❌   |  ✅  |
| Weekly AI wealth summaries       |   ❌   |  ✅  |
| Proactive alerts                 |   ❌   |  ✅  |

No free tier. No "Basic". No "Pro / Enterprise". Two plans, billed monthly or yearly.

---

## 4. The AI-native workflow (target experience)

This is what a fresh Silver-tier install should feel like:

1. **Onboarding** — 3 steps (welcome, currency, appearance). No "Add your first account" gate. The user lands on the dashboard.
2. **Empty dashboard isn't empty** — three example liabilities are pre-seeded (mortgage, student loan, credit card), each marked "Example — edit me". The "Net Worth" tile shows real math against those examples. The Assistant icon pulses gently.
3. **User opens the Assistant** — types or speaks: *"Hey, I have a Schwab brokerage with 200 AAPL averaging $150 cost basis, and an HDFC India savings account with about ₹450,000."*
4. **AI drafts** — calls `create_account` (Schwab USD brokerage), `record_activity` BUY 200 AAPL, `create_account` (HDFC INR cash), `create_activity` DEPOSIT ₹450,000. Each draft renders inline as a confirm card with editable fields.
5. **User edits + confirms** — single click per draft. State changes are atomic and reversible.
6. **User says** *"Replace the mortgage example with my actual: $480k principal, 5.2% fixed, $2,650/month, started Jan 2023."*
7. **AI calls `update_liability`** on the existing example row — doesn't create a new one. (Edit-first UX, Feroz's point.)
8. **Five minutes later, portfolio is live.** No forms touched. The user goes back to whatever they were doing.

The user *can* still use the legacy forms — but the AI is the front door and the most pleasant path for everything.

---

## 5. What's missing — the precise gap

Mapped end-to-end by 3 parallel exploration agents. Findings condensed:

### 5.1 AI tools — read complete, write incomplete

| Tool                  | Read | Draft Write | Direct Write | Status |
| --------------------- | :--: | :---------: | :----------: | ------ |
| `get_accounts`        |  ✅  |             |              | ✅ Exists |
| `get_holdings`        |  ✅  |             |              | ✅ Exists |
| `search_activities`   |  ✅  |             |              | ✅ Exists |
| `get_allocation`      |  ✅  |             |              | ✅ Exists |
| `get_valuation_history` | ✅ |             |              | ✅ Exists |
| `get_performance`     |  ✅  |             |              | ✅ Exists |
| `get_income`          |  ✅  |             |              | ✅ Exists |
| `get_goals`           |  ✅  |             |              | ✅ Exists |
| `get_cash_balances`   |  ✅  |             |              | ✅ Exists |
| `record_activity`     |      |     ✅      |              | ✅ Exists |
| `record_activities`   |      |     ✅      |              | ✅ Exists |
| `import_csv`          |      |     ✅      |              | ✅ Exists |
| **`create_account`**          |      |    🆕      |              | ❌ Build it (Phase B) |
| **`add_alternative_asset`**   |      |    🆕      |              | ❌ Build it (Phase B) |
| **`create_liability`**        |      |    🆕      |              | ❌ Build it (Phase B) |
| **`update_liability`**        |      |    🆕      |              | ❌ Build it (Phase B) — edit-first |
| **`create_goal`**             |      |    🆕      |              | ❌ Build it (Phase B) |
| **`update_account`**          |      |    🆕      |              | ❌ Build it (Phase B) — edit-first |

The Rust core services for all of these already exist — we just need the thin AI-tool wrapper, a tool-UI component for the draft-and-confirm card, and a system-prompt update.

Files:
- [crates/ai/src/tools/](mizan-4/crates/ai/src/tools/) — add new files here (mirror `record_activity.rs` pattern).
- [crates/ai/src/system_prompt.txt](mizan-4/crates/ai/src/system_prompt.txt) — extend the tool list and the examples.
- [apps/frontend/src/features/ai-assistant/components/tool-uis/](mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/) — add one TSX per tool.
- [apps/frontend/src/features/ai-assistant/components/tool-uis/index.ts](mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/index.ts) — register them.

### 5.2 Zakat needs to move to Gold

Currently `zakatEngine: true` lives in `BASE_CAPABILITIES` (shared by Silver + Gold) at [apps/frontend/src/domain/account/capabilities.ts:24-32](mizan-4/apps/frontend/src/domain/account/capabilities.ts).

Sites that need changing:

| File                                                                  | Change                                                              |
| --------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `apps/frontend/src/domain/account/capabilities.ts`                    | Move `zakatEngine: true` out of `BASE_CAPABILITIES` → Gold-only.    |
| `apps/tauri/src/commands/zakat.rs`                                    | Gate the `compute_zakat` command behind the `zakatEngine` capability via `assertCapability` (currently uses `advanced_reports`). |
| `apps/frontend/src/routes.tsx`                                        | Add capability guard on `<Route path="/zakat">`.                    |
| `apps/frontend/src/pages/dashboard/zakat-card.tsx`                    | Render an upgrade-CTA variant when `!canUseCapability(tier, "zakatEngine")`. |
| `apps/frontend/src/pages/zakat/zakat-page.tsx`                        | Already says "Gold feature" in copy ✓.                              |

### 5.3 No seed data on first launch

The app currently lands a fresh Silver user on a fully empty dashboard. Per Feroz: pre-seed 3 example liabilities.

- New service: `crates/core/src/onboarding/seed_examples.rs` — idempotent, runs once when `onboardingCompleted` flips and no liabilities exist.
- Each row tagged with `metadata.example = true` and name prefixed `"Example — "`. The frontend renders these with an "Example, tap to make real" hint.
- Rows:
  1. Mortgage — principal $480,000, rate 5.2%, monthly $2,650, originated Jan 2023.
  2. Student loan — balance $32,000, rate 6.8%, monthly $410.
  3. Credit card — balance $4,800, rate 22.99% APR, monthly minimum $145.

### 5.4 Manual entry isn't currently first-class

The Rust core already supports manual accounts (`provider = null` or `"MANUAL"`). The frontend has a manual account form ([apps/frontend/src/pages/settings/accounts/components/account-form.tsx](mizan-4/apps/frontend/src/pages/settings/accounts/components/account-form.tsx)). What's missing:

- **AI-native creation path** — when the user says "I have an HDFC India account with ₹4.5L cash", the AI should call `create_account` directly. (Solved by Phase B.)
- **Manual "Update balance"** affordance on manual accounts — same prominence as the Plaid "Sync" button. Today there's an edit form but no quick-update flow.
- **Badge parity** — manual accounts should show a small "Manual" or "Self-tracked" pill the way Plaid accounts show "Live".

### 5.5 Edit-first UX is missing

Today, "Add Holding", "Add Liability", "Add Goal" all open a blank form. Feroz's principle: scaffold first, ask the user to edit. We get this for free from:

- The pre-seeded example liabilities (§5.3).
- The AI Assistant always returning a draft populated with sensible defaults the user can tweak (Phase B tools).
- Updating existing forms to pre-fill via `defaultValues` from "most-recently-created" sibling rows when available.

---

## 6. Implementation plan

Six phases, ordered by dependency. Each phase ends in `cargo` + `pnpm` verification and a single atomic commit. No phase ships half-done.

### Phase A — Move Zakat to Gold

**Files**
- `apps/frontend/src/domain/account/capabilities.ts` — move `zakatEngine`.
- `apps/tauri/src/commands/zakat.rs` — gate via `zakatEngine`.
- `apps/frontend/src/routes.tsx` — route guard.
- `apps/frontend/src/pages/dashboard/zakat-card.tsx` — upgrade variant.

**Verify**
- `pnpm --filter frontend type-check && lint:quiet && test -- --run`
- Manual: Silver user sees Zakat as a locked Gold feature on dashboard.

### Phase B — Six new AI write tools

Build each by mirroring `record_activity.rs`:

1. **`create_account`** → wraps `accounts_service.create_account`. Args: name, account_type, currency, [is_default]. Returns draft + available currencies for dropdown.
2. **`update_account`** → wraps `accounts_service.update_account`. Used when AI rewrites an Example or fills in details.
3. **`add_alternative_asset`** → wraps `AlternativeAssetService.create_alternative_asset`. Kind: `Property | Collectible | Precious | Liability | Other`. Args differ by kind (real estate gets address, value, currency, linked liability).
4. **`create_liability`** → also wraps `AlternativeAssetService.create_alternative_asset` with `kind: Liability`. Args: liability_type (mortgage / student_loan / credit_card / personal_loan / auto_loan / heloc), principal, currency, rate, monthly_payment, linked_asset_id.
5. **`update_liability`** → updates an existing `Liability` alternative asset by id. Critical for "replace the example mortgage" flow.
6. **`create_goal`** → wraps `goals_service.create_goal`. Args: title, target_amount, currency, target_date, [linked_account_id].

For each:
- Rust tool: `crates/ai/src/tools/<name>.rs` + register in `tools/mod.rs`.
- Tauri command if needed: `apps/tauri/src/commands/ai_tools.rs` (or extend existing).
- React tool UI: `apps/frontend/src/features/ai-assistant/components/tool-uis/<name>-tool-ui.tsx`.
- Register in `tool-uis/index.ts`.
- Tests: round-trip parse, happy path, validation error.

**System prompt updates** in [crates/ai/src/system_prompt.txt](mizan-4/crates/ai/src/system_prompt.txt):

- Add the six new tools to the numbered list with examples.
- Add a new "PORTFOLIO_BUILDING" section instructing the AI to always check for existing examples before creating; if a user description matches an Example row (name starts with `"Example — "`), the AI calls `update_*` not `create_*`.
- Add the *manual-bank* fallback: "If a user mentions a bank outside US/CA (e.g. India, UK, UAE), do not suggest Plaid — quietly call `create_account` and `record_activity`."

**Verify**
- `cargo test -p mizan-ai`
- `pnpm --filter frontend type-check && test -- --run`
- Manual: install fresh, say "I have a Vanguard taxable account with 100 AAPL", confirm draft, see Account + Holding appear.

### Phase C — Seed 3 dummy liabilities on first launch

**New Rust file**: `crates/core/src/onboarding/seed_examples.rs`

```rust
pub async fn seed_example_liabilities(service: &dyn AlternativeAssetService) -> Result<()> {
    if has_any_liability(service).await? {
        return Ok(());
    }
    service.create_alternative_asset(NewAlternativeAsset {
        kind: AssetKind::Liability,
        name: "Example — Home mortgage".into(),
        currency: "USD".into(),
        market_value: dec!(480_000),
        metadata: json!({
            "liability_type": "mortgage",
            "rate_pct": 5.2,
            "monthly_payment": 2650,
            "originated_at": "2023-01-15",
            "example": true,
        }),
        ..Default::default()
    }).await?;
    // ... student loan, credit card
    Ok(())
}
```

**Call site**: at the end of onboarding step 3, after settings persist, before navigation to `/`.

**Frontend** ([apps/frontend/src/pages/asset/linked-liabilities-card.tsx](mizan-4/apps/frontend/src/pages/asset/linked-liabilities-card.tsx) + the liabilities list): when `metadata.example === true`, render with a soft amber border and a "Tap to edit" hint. On first edit, strip the `Example — ` prefix and clear `example: true`.

**Verify**
- Wipe local DB, complete onboarding, see 3 example liabilities on dashboard.
- Open Assistant, say "Update the mortgage example to my real one: $620k principal" — AI calls `update_liability` on the existing row.

### Phase D — Manual-entry parity with Plaid

1. **Badge component** — render a small "Manual" pill on manual accounts ([apps/frontend/src/pages/settings/accounts/components/account-item.tsx:46-82](mizan-4/apps/frontend/src/pages/settings/accounts/components/account-item.tsx)).
2. **Update Balance affordance** — add a quick-action button on manual accounts that opens a single-field modal ("New balance: ___ as of [today]"). Persists as a cash-balance snapshot or an activity, depending on account type.
3. **AI fallback heuristic** — if the user mentions a non-Plaid country, the Assistant doesn't suggest connecting a bank; it offers manual entry. Encoded in system prompt.

**Verify**
- Manual: create a manual HDFC INR account via Assistant, verify it shows up in account list with "Manual" badge, "Update Balance" works.

### Phase E — Database bug investigation

Get repro from Sami. Most likely candidates given recent changes:

- Migration 0008 (`plaid_sync_throttle.sql`) running against an existing DB that already had stale SnapTrade migration state.
- Local SQLite migration drift in `crates/storage-sqlite/src/schema.rs` if a previously-installed DMG left a half-migrated DB.
- Account creation flow racing with the new `provider` column default.

Plan: ask Sami for repro, look at logs, write a regression test, fix, ship.

### Phase F — Verify + rebuild + push

- `cargo check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --lib` (both `mizan-connect/` and the `mizan-4/` workspace).
- `pnpm --filter frontend type-check && lint:quiet && test -- --run && build`.
- `pnpm tauri build --target aarch64-apple-darwin` → new DMG.
- `git commit && git push` → CI picks it up. `deploy-mizan-connect.yml` redeploys Fly automatically if backend changed.
- Open the new DMG and walk through the workflow described in §4 as a final smoke.

---

## 7. Timeline

Realistic, single-engineer, no shortcuts:

| Phase | Description                              | Effort |
| :---: | ---------------------------------------- | :----: |
|   A   | Move zakat → Gold                        |  1 h   |
|   B   | Six AI write tools + tool UIs + prompt   |  4–6 h |
|   C   | Seed 3 dummy liabilities + edit-first UX |  2 h   |
|   D   | Manual-entry parity                      |  2 h   |
|   E   | DB bug investigation + fix               |  1–3 h |
|   F   | Verify + DMG rebuild + push              |  1 h   |
|       | **Total**                                | **11–15 h** |

≈ 2 focused days. Three calendar days with sleep, prayer, and the inevitable surprise migration issue.

---

## 8. Definition of done

Mizan AI is "AI-native end to end" when a brand-new install can do this in under 5 minutes without touching a single legacy form:

1. Launch the DMG, see Mizan AI in Applications.
2. Onboard (3 steps).
3. See 3 example liabilities with real-feeling numbers.
4. Open the Assistant, type a free-form description of the user's actual portfolio (one or more accounts, one or more holdings, one or more alt assets, optionally a goal).
5. Confirm each draft (≤ 4 clicks).
6. Edit the example liabilities into real ones via the AI.
7. Land back on the dashboard with a fully populated, real Net Worth tile.
8. Optionally upgrade to Gold and connect Plaid sandbox (or, for non-US users, keep going manually).
9. Optionally ask the AI: *"What's my zakat estimate?"* → Gold-tier upgrade modal if Silver, real computation if Gold.

Every step above must work on the unsigned DMG, on macOS arm64, against `mizan-connect.fly.dev`, with sandbox Plaid + the user's own OpenAI key.

---

## 9. Out of scope (for now)

- Apple notarization / Windows Authenticode (waiting on certs).
- Stripe live mode (test keys only).
- Plaid production credentials (sandbox only).
- Tauri auto-updater (post-first signed release).
- Mobile (iOS / Android) — future.
- The dormant Yahoo / yfinance code paths in `crates/market-data/src/provider/yahoo/` — not on the active product path, full deletion is a larger refactor.
- Marketing site / app icon redesign.

---

## 10. Critical files index

For fast navigation during implementation:

**AI runtime**
- [mizan-4/crates/ai/src/system_prompt.txt](mizan-4/crates/ai/src/system_prompt.txt)
- [mizan-4/crates/ai/src/tools/](mizan-4/crates/ai/src/tools/)
- [mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/](mizan-4/apps/frontend/src/features/ai-assistant/components/tool-uis/)
- [mizan-4/apps/frontend/src/features/ai-assistant/hooks/use-chat-runtime.ts](mizan-4/apps/frontend/src/features/ai-assistant/hooks/use-chat-runtime.ts)

**Capability matrix**
- [mizan-4/apps/frontend/src/domain/account/capabilities.ts](mizan-4/apps/frontend/src/domain/account/capabilities.ts)
- [mizan-4/crates/connect/src/entitlements.rs](mizan-4/crates/connect/src/entitlements.rs)

**Core services (already exist — just need AI-tool wrappers)**
- [mizan-4/crates/core/src/accounts/accounts_service.rs](mizan-4/crates/core/src/accounts/accounts_service.rs)
- [mizan-4/crates/core/src/assets/alternative_assets_service.rs](mizan-4/crates/core/src/assets/alternative_assets_service.rs)
- [mizan-4/crates/core/src/goals/goals_service.rs](mizan-4/crates/core/src/goals/goals_service.rs)

**Onboarding**
- [mizan-4/apps/frontend/src/pages/onboarding/](mizan-4/apps/frontend/src/pages/onboarding/)

**Liabilities**
- [mizan-4/apps/frontend/src/pages/asset/linked-liabilities-card.tsx](mizan-4/apps/frontend/src/pages/asset/linked-liabilities-card.tsx)
- [mizan-4/apps/frontend/src/pages/asset/alternative-assets/components/alternative-asset-quick-add-modal.tsx](mizan-4/apps/frontend/src/pages/asset/alternative-assets/components/alternative-asset-quick-add-modal.tsx)

**Plaid integration**
- [mizan-connect/src/plaid/](mizan-connect/src/plaid/)
- [mizan-4/apps/frontend/src/features/mizan-connect/](mizan-4/apps/frontend/src/features/mizan-connect/)

**Deploy + CI**
- [.github/workflows/ci.yml](.github/workflows/ci.yml)
- [.github/workflows/deploy-mizan-connect.yml](.github/workflows/deploy-mizan-connect.yml)
- [.github/workflows/release-desktop.yml](.github/workflows/release-desktop.yml)
- [mizan-connect/fly.toml](mizan-connect/fly.toml)
- [mizan-connect/Dockerfile](mizan-connect/Dockerfile)

---

*Document owner: Sami. Last updated: 2026-05-24. Next review: after Phase F ships.*
