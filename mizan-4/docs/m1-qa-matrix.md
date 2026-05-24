# M1.5 — Per-tier QA matrix

Exercises every paid gate added in M1 + M1.5 across Free / Basic / Pro / Bypass.
Designed to run in ~25 minutes by a human against a local cloud + Stripe-CLI
forwarder, or against a deployed staging cloud.

## Local end-to-end setup (one-time)

```bash
# Cloud
cd ~/Documents/mizan-connect
docker-compose up -d              # Postgres on :5433
cp .env.example .env              # fill: STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET,
                                  # STRIPE_PRICE_BASIC_MONTHLY etc., OPENAI_API_KEY
cargo run                         # cloud listens on :8080
stripe listen --forward-to localhost:8080/v1/stripe/webhook
# (note the whsec_… stripe-cli prints; set STRIPE_WEBHOOK_SECRET to it for local)

# Desktop
cd ~/Documents/Mizan-4
CONNECT_API_URL=http://localhost:8080 pnpm tauri dev
```

## The matrix

Each row = (tier, action) → expected behavior. Take 2-min screenshots of any
delta from expectations.

### Tier: **Free** (no subscription)

| #   | Action                                            | Expected                                                                                                                                                        |
| --- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | Open dashboard                                    | Loads. Net worth visible. AI assistant tab present.                                                                                                             |
| F2  | Add a 2nd portfolio (Settings → Portfolios → New) | Backend `create_account` rejects with GatedError; **UpgradeModal** opens with "Add more portfolios" copy. Click Upgrade now → Stripe Checkout opens in browser. |
| F3  | Portfolio drill-down → 3rd asset class → Add      | Proactive frontend gate; UpgradeModal "Track your full wealth" copy.                                                                                            |
| F4  | Asset class → Bank → Add                          | Bank modal opens (if under holdings cap).                                                                                                                       |
| F5  | At 20 holdings → Add                              | UpgradeModal "Add more holdings" copy (proactive frontend gate).                                                                                                |
| F6  | Settings → Connect a broker                       | `sync_broker_data` rejects; UpgradeModal "Connect your broker" copy.                                                                                            |
| F7  | Settings → Enable device sync                     | `enroll_device` rejects; UpgradeModal "Sync across your devices" copy.                                                                                          |
| F8  | AI assistant → set Mizan provider (if visible)    | Card shows "Upgrade to unlock" — click → UpgradeModal "Meet Mizan AI" copy.                                                                                     |
| F9  | AI assistant → set OpenAI BYO key + chat          | Works (BYO path is free for everyone).                                                                                                                          |
| F10 | CSV import → upload + map + confirm               | Backend `import_activities` rejects; UpgradeModal "Import more statements".                                                                                     |
| F11 | Market refresh button                             | Rate-limiter clamps after 3/30s; no upgrade modal (Free allowed 5/day).                                                                                         |

### Tier: **Basic** (after F2 → Stripe Checkout success, return to app)

| #   | Action                 | Expected                                                                                                                      |
| --- | ---------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| B0  | Window focus on return | `[ENTITLEMENTS]` + `[USER_INFO]` invalidate; team.plan == "basic" within ~1s.                                                 |
| B1  | Add 2nd-5th portfolio  | Allowed (cap = 5).                                                                                                            |
| B2  | Add 6th portfolio      | Backend rejects; UpgradeModal "Add more portfolios" suggests **Pro**.                                                         |
| B3  | Connect broker         | `sync_broker_data` still rejects (Basic = device-sync-only legacy rule); UpgradeModal "Connect your broker" suggests **Pro**. |
| B4  | Enable device sync     | Allowed.                                                                                                                      |
| B5  | Mizan AI card          | Shows "Included with subscription" badge; chat header shows N/300 credits.                                                    |
| B6  | Send AI chat           | Reply streams; `mizan_credits.used` in response increments; usage_ledger row appears in cloud DB.                             |
| B7  | CSV import             | Allowed. After import, `usage_ledger` has a `csv_intel` row.                                                                  |
| B8  | Market refresh         | Allowed (50/day cap; cloud counts via `/v1/usage`).                                                                           |

### Tier: **Pro**

| #   | Action                                       | Expected                                    |
| --- | -------------------------------------------- | ------------------------------------------- |
| P1  | Upgrade Basic→Pro via Stripe Customer Portal | `team.plan` flips to "pro" on focus return. |
| P2  | Connect broker                               | Allowed (cap = 5 connections).              |
| P3  | Advanced report request                      | Allowed.                                    |
| P4  | AI credits                                   | 1500/mo balance shown in chat header.       |

### Tier: **Bypass** (dev only)

Set `CONNECT_BYPASS_PLAN_CHECK=true` and restart desktop. Every gate above
passes regardless of cloud state. Used to verify UI paths without billing.

## Stripe Dashboard prerequisites (do these once)

1. **Products** — create three Products: "Mizan Basic", "Mizan Pro", "Mizan
   Enterprise".
2. **Prices** — under each Product, add Monthly + Yearly recurring prices
   ($19.99 / $199 / $39.99 / $399 / $249 / $2490). On every Price, set
   `metadata.plan = basic|pro|enterprise`.
3. **Webhook** — Dashboard → Developers → Webhooks → Add endpoint:
   `https://<your-fly-host>/v1/stripe/webhook`. Listen for:
   `customer.subscription.created`, `customer.subscription.updated`,
   `customer.subscription.deleted`, `invoice.paid`,
   `checkout.session.completed`. Copy the `whsec_…` into Fly secrets:
   `fly secrets set STRIPE_WEBHOOK_SECRET=whsec_…`.
4. **Fly secrets** — `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`,
   `STRIPE_PRICE_BASIC_MONTHLY`, `STRIPE_PRICE_BASIC_YEARLY`,
   `STRIPE_PRICE_PRO_MONTHLY`, `STRIPE_PRICE_PRO_YEARLY`,
   `STRIPE_PRICE_ENTERPRISE_MONTHLY`, `STRIPE_PRICE_ENTERPRISE_YEARLY`,
   `MIZAN_BILLING_RETURN_URL`, `OPENAI_API_KEY`.

## Known gaps (cleanly deferred from M1.5)

- Managed-AI streaming SSE proxy — cloud `/v1/ai/chat` currently non-streaming
  JSON; sufficient for desktop's `complete` path. Streaming is a follow-up.
- Desktop's `mizan` provider only declared in the catalog (`ai_providers.json`);
  the client request path that targets it via JWT auth lands when the streaming
  proxy ships.
- Local AI usage ledger (offline visibility) — cloud's `/user/me.aiCredits` is
  authoritative; local mirror is nice-to-have for offline display.
- Backend `create_activity` holdings cap — frontend already gates proactively;
  backend chokepoint needs symbol resolution to avoid blocking buys of existing
  positions. Alt-asset creation IS gated.

---

## M2 additions (UI simplification)

### Tier-agnostic structure

| #     | Action                                                                                 | Expected                                                                                                                                                                        |
| ----- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M2-1  | Desktop sidebar                                                                        | 5 items: Home, Portfolio, Add, Goals, Assistant. "Settings" appears in the lower (secondary) group.                                                                             |
| M2-2  | Click "Add" in sidebar                                                                 | Wizard dialog opens with 7 tiles. No URL change.                                                                                                                                |
| M2-3  | Wizard → "Stocks, ETFs, Sukuk or Bonds"                                                | Closes; navigates to `/accounts/:default-portfolio-id?addHoldings=1`; holdings editor sheet auto-opens; URL param is stripped after consumption.                                |
| M2-4  | Wizard → "Bank account or cash"                                                        | Closes; AddBankAccountModal opens.                                                                                                                                              |
| M2-5  | Wizard → "Property" / "Gold" / "Collectibles" / "Loan or liability" / "Something else" | AlternativeAssetQuickAddModal opens with the matching `defaultKind`.                                                                                                            |
| M2-6  | Click "Portfolio" tab                                                                  | Navigates to `/portfolio` — PortfolioListPage with AccountsSummary + "Add asset" CTA.                                                                                           |
| M2-7  | Drill into `/accounts/:id`                                                             | "Portfolio" tab still highlighted in the sidebar (isPathActive treats `/accounts/*` as inside Portfolio).                                                                       |
| M2-8  | Mobile bottom nav                                                                      | All 5 primary tabs visible. Add fires the wizard (no nav change). Search via Cmd/Ctrl+K still works.                                                                            |
| M2-9  | Settings → desktop sidebar                                                             | Six sections: Preferences, Wealth, Sync, AI, Advanced, About. "Sync" collapses the old Connect/Market Data/AI Providers grouping; advanced surfaces collapsed under "Advanced". |
| M2-10 | Settings → "AI" → Providers                                                            | Mizan AI hero card on top (locked if Free, "Included" badge if paid). BYO-key list below excludes the `mizan` provider catalog entry.                                           |
| M2-11 | Performance page / Assistant performance tool                                          | Labels read "Investment growth" / "Net performance" (no more "TWR" / "MWR" in user-visible strings).                                                                            |
| M2-12 | Settings → Advanced → Market Data                                                      | Card title reads "Data connections" (no more "Provider health").                                                                                                                |
| M2-13 | New install onboarding step 1                                                          | Friendly welcome with 3 trust bullets (privacy / setup time / no-account-required). No more "Holdings vs Transactions" upfront jargon.                                          |

### Free / Basic / Pro / Bypass interactions with M2

| #         | Tier   | Action                                               | Expected                                                                  |
| --------- | ------ | ---------------------------------------------------- | ------------------------------------------------------------------------- |
| M2-F1     | Free   | Click "Add" → "Bank" → save → repeat to 21st holding | UpgradeModal raises on the 21st save (M1.5 alt-asset backend gate fires). |
| M2-F2     | Free   | Settings → AI → Mizan AI hero CTA                    | UpgradeModal with "Meet Mizan AI" copy → Stripe Checkout.                 |
| M2-Bypass | Bypass | Same flows                                           | Modal never raises; all actions complete.                                 |
