# Mizan Connect — Entitlements & Billing API Contract

This is the contract between the Mizan desktop/web **client** (this repo) and
the Mizan Connect **cloud backend** (`api.mizan.app`, a separate service).
Milestone 1 implemented the entire client side; the endpoints below are what the
cloud must add for the paid surface to light up end-to-end.

Until the cloud ships §A, the client **derives** entitlements from the existing
`team.plan` + `subscription_status` on `/api/v1/user/me` (see
`crates/connect/src/entitlements.rs::entitlements_for_plan`). When the cloud
starts returning an explicit `entitlements` object, the client prefers it
automatically — no client change required.

The `CONNECT_BYPASS_PLAN_CHECK=true` build/env flag unlocks everything for
dev/QA.

---

## §A — Augment `GET /api/v1/user/me`

Add two fields to the `team` object (both optional/additive; existing fields
unchanged):

```jsonc
{
  "team": {
    "plan": "pro",
    "subscriptionStatus": "active",
    // NEW — authoritative entitlements (camelCase). When present the client uses
    // it verbatim instead of deriving from `plan`.
    "entitlements": {
      "plan": "pro",
      "maxPortfolios": 25, // -1 = unlimited
      "maxHoldings": -1,
      "maxAssetClasses": -1,
      "brokerSync": true,
      "maxBrokerConnections": 5,
      "deviceSync": true,
      "cloudBackup": true,
      "managedAi": true,
      "aiCreditsMonthly": 1500, // -1 = unlimited
      "newsDailyLimit": -1,
      "marketRefreshDailyLimit": -1,
      "csvImportsMonthly": -1,
      "advancedReports": true,
      "advisorMode": false,
    },
    // NEW — live AI credit balance for display + soft metering.
    "aiCredits": {
      "monthly": 1500,
      "used": 240,
      "resetsAt": "2026-06-01T00:00:00Z",
    },
  },
}
```

Field semantics mirror `Entitlements` in `crates/connect/src/entitlements.rs`.
`-1` means "unlimited" on every numeric quota.

### Tier matrix the client assumes today (stopgap, in `entitlements_for_plan`)

| field                    | free | basic (legacy: device-sync-only) | pro / other paid | enterprise |
| ------------------------ | ---- | -------------------------------- | ---------------- | ---------- |
| maxPortfolios            | 1    | 5                                | 25               | ∞          |
| maxHoldings              | 20   | 250                              | ∞                | ∞          |
| maxAssetClasses          | 2    | ∞                                | ∞                | ∞          |
| brokerSync               | ✗    | ✗                                | ✓                | ✓          |
| maxBrokerConnections     | 0    | 0                                | 5                | ∞          |
| deviceSync / cloudBackup | ✗    | ✓                                | ✓                | ✓          |
| managedAi                | ✗    | ✓                                | ✓                | ✓          |
| aiCreditsMonthly         | 0    | 300                              | 1500             | ∞          |
| advancedReports          | ✗    | ✗                                | ✓                | ✓          |
| advisorMode              | ✗    | ✗                                | ✗                | ✓          |

> Plan **slugs** are an open question — the existing cloud returns
> `basic/essentials/duo/plus`, the product manual wants
> `free/basic/pro/enterprise`. `entitlements_for_plan` maps known slugs and
> treats any unknown active paid slug as the "pro" matrix. Once slugs are
> finalized, either pin them there or (preferably) have the cloud return the
> explicit `entitlements` object so slugs stop mattering.

---

## §B — Billing (Stripe) checkout & portal

```
POST /api/v1/billing/checkout-session   { "plan": "pro", "interval": "yearly" } -> { "url": "https://checkout.stripe.com/..." }
POST /api/v1/billing/portal             {}                                       -> { "url": "https://billing.stripe.com/..." }
```

Client flow: open `url` in the user's browser → Stripe → Stripe webhook updates
the cloud subscription → client refetches `/user/me` and invalidates the
`["entitlements"]` query, unlocking features. The client opens these via the
upgrade modal's "View plans" → Connect page.

Stripe products/prices, webhook handlers, and the `customer_subscriptions` table
are cloud-side work (not in this repo).

---

## §C — Usage ledger (server-authoritative metering)

```
POST /api/v1/usage   { "metric": "ai_reply" | "broker_poll" | "csv_intel" | "market_refresh", "units": 5 }
```

The cloud enforces monthly caps server-side and reflects remaining balance in
`/user/me.aiCredits`. Client-side counters (portfolios/holdings/etc.) are UX
gates only; cloud metering is the tamper-proof enforcement for cost-bearing
actions (managed AI, broker polling). Cloud DB: `usage_credit_ledger`.

---

## Client enforcement already in place (this repo, M1)

- **Rust IPC gates** (tamper-resistant): `sync_broker_data` (broker_sync),
  `enroll_device` (device_sync), `create_account` (max_portfolios). Each returns
  a JSON `GatedError { __gated, feature, requiredTier, currentPlan, message }`.
- **Web-mode parity**: `apps/server` mirrors the broker gate and exposes
  `GET /connect/entitlements`.
- **Frontend**: `useEntitlements()`, a contextual `UpgradeModal` (auto-raised
  from any mutation that returns a `GatedError`, via the mutation-cache bridge),
  and a proactive holdings-cap gate in the asset-class Add flow.
- **AI**: a "not financial advice" guardrail baked into the system prompt
  (`crates/ai/src/system_prompt.txt`). Free users use BYO API keys (today's
  flow); **managed Mizan AI routing + credit metering require §A/§C above** and
  are deferred to the cloud track.

## Deferred to later milestones (tracked, not in M1)

- Backend chokepoint gates for holdings/asset-class/CSV/market-refresh (no
  single command today — enforced in the frontend for now).
- Managed-AI provider wiring once the cloud AI endpoint + credit ledger exist.
- Web-mode account-creation gate parity.
