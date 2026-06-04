# Monitoring dashboard — Mizan production

**Track PROD PR-PROD-4 / Goal v3 §V Phase 11 step 4.**

Authoritative spec for the production monitoring dashboard per
Spec §15. Real-time SSE for hot metrics, admin endpoints with
constant-time bearer auth.

---

## Panels

Per Spec §15: 8 panel groups. Each panel below carries the data
source + the refresh cadence + the on-call escalation when the
panel goes red.

### 1. Acquisition

- **Daily signups** (last 30d, by tier intended) — Source:
  `users` row inserts. Refresh: hourly. Red: drop below
  rolling-30d average by > 30%.
- **Conversion funnel** (signup → 1st sync → 1st Zakat calc →
  paid tier) — Source: event log. Refresh: daily.
- **Acquisition channel** — Source: `users.referral_source`.
  Refresh: daily.

### 2. Engagement

- **DAU / WAU / MAU** — Source: `agent_audit_log` distinct
  user_id. Refresh: hourly.
- **Sessions per user per week** — Source: app-foreground
  events. Refresh: daily.
- **Feature heatmap** (which panels tapped, which insights
  expanded, which agent tools invoked) — Source: telemetry
  events. Refresh: daily.

### 3. Tier distribution

- **Active subscribers by tier** (Free / Silver / Gold /
  Enterprise / Advisor) — Source: `subscriptions` joined with
  `users`. Refresh: real-time (Stripe webhook).
- **Upgrade rate** (Silver → Gold) — Source: tier-change events.
  Refresh: daily.
- **Downgrade rate** — Source: same.

### 4. Revenue

- **MRR** (split by tier) — Source: Stripe MRR API.
  Refresh: real-time.
- **ARR** — Source: Stripe ARR.
- **Failed payments** (last 7d) — Source: Stripe webhook
  events. Refresh: real-time.
- **Net revenue retention** — Source: same. Refresh: monthly.

### 5. Reliability

- **Endpoint p50 / p99 latency** (per route) — Source: Mizan
  Connect tracing. Refresh: real-time SSE.
- **Sync success rate** (per provider) — Source:
  `sync_run_ledger`. Refresh: hourly.
- **Webhook reliability** (per integration) — Source: webhook
  receipt logs. Refresh: hourly.
- **Truth Ledger chain integrity** (latest verify result) —
  Source: `TruthLedger::verify()` nightly job.
  Refresh: nightly.

### 6. AI cost

- **Hourly AI spend** (last 24h) — Source: Anthropic billing
  API + per-call cost telemetry. Refresh: real-time SSE.
- **Cost per active user** — Source: same / DAU. Refresh: daily.
- **Prompt-cache hit rate** — Source: Anthropic API. Refresh:
  hourly. Target: ≥ 80% per CLAUDE.md §15.6.
- **Top spenders by user** — Source: per-user cost telemetry.
  Refresh: hourly. Red: any single user > $1/hr sustained.

### 7. Sharia / Zakat metrics

- **Sharia screening coverage** (% of holdings screened
  within 30d) — Source: `holdings.sharia_status` +
  `last_screened_at`. Refresh: daily. Target: ≥ 90%.
- **Zakat assessments per week** (by school) — Source:
  `truth_ledger` filtered to `ZakatComputed`. Refresh: daily.
- **Pay Zakat completion rate** — Source: `payment_intent`
  succeeded vs Zakat assessments started. Refresh: daily.
- **Total Zakat paid through Mizan** (cumulative) — Source:
  `zakat_receipts.amount` sum. Refresh: daily.
  *This is Mizan's headline mission metric.*

### 8. Compliance

- **Webhook signature verification rate** — Source: webhook
  receipt logs. Refresh: hourly. Target: 100%.
- **HS256 JWT rejection in production** — Source: auth audit
  log. Refresh: real-time. Target: zero rejections
  (production is RS256-only).
- **Truth Ledger chain length** — Source: same query as the
  chain-integrity verifier. Refresh: nightly.
- **Token encryption key inventory** — Source: cross-reference
  with `secrets-inventory.md`. Refresh: weekly. Red: any key
  past its rotation cadence.
- **gitleaks scan** (last full-history result) — Source:
  GitHub Actions artifact. Refresh: nightly.

---

## Real-time SSE channels

Three panels stream over SSE because polling them on a 1-min
schedule would miss the data they're meant to surface:

1. **Endpoint p50 / p99 latency** — per-route rolling 5-min
   windows. Subscriber: Reliability panel.
2. **Hourly AI spend** — sub-second deltas as Anthropic
   billing API webhooks fire. Subscriber: AI cost panel.
3. **Webhook signature failures** — every failure pushed
   immediately. Subscriber: Compliance panel + Tier-1
   `webhook-signature-failure` Sentry alert (overlap is
   intentional — the dashboard surfaces the count, Sentry
   pages on the rate).

---

## Admin auth

Dashboard sits behind the existing admin authentication
boundary per Spec §15:

- **Bearer token** in `Authorization: Bearer <token>` header
- **Constant-time comparison** via `subtle::ConstantTimeEq`
  per CLAUDE.md §16 (subtle-comparison rule)
- **Token rotation cadence** — quarterly, tracked in
  `secrets-inventory.md` as `ADMIN_DASHBOARD_BEARER_TOKEN`
- **IP allowlist** at the Fly.io level — only the operator's
  static IPs can reach `/admin/*`

Every admin endpoint also emits an `agent_audit_log` row so
admin access is traceable for support + compliance.

---

## Implementation surface

Lives under `mizan-connect/src/admin/` (already partially
shipped — see `admin/handlers.rs`):

- `mizan-connect/src/admin/dashboard.rs` — panel-data query
  module (one function per panel group)
- `mizan-connect/src/admin/sse.rs` — three SSE stream handlers
- `mizan-4/apps/frontend/src/pages/admin/dashboard.tsx` —
  React frontend rendering the 8 panel groups
- `mizan-4/apps/frontend/src/pages/admin/sse-client.ts` —
  EventSource wrapper for the 3 real-time channels

Charts use the existing 5-primitive vocabulary per ADR 0019
(`@mizan/ui` Donut / Bar / Sparkline / Heatmap / Sankey).

---

## Pre-canary validation

Before the 5% canary cohort goes live, the operator:

- [ ] Loads the staging dashboard + verifies every panel
      renders without errors
- [ ] Verifies admin bearer token rotation happened in the
      past 90 days
- [ ] Verifies SSE channels stream without disconnect for
      30 consecutive minutes
- [ ] Verifies the 8 panel groups have non-empty data (sentinels
      for empty: explicit "no data yet" instead of blank chart)
- [ ] Subscribes the operator's PagerDuty integration to the
      dashboard's outbound alert webhook (one final escalation
      path if everything else fails)

---

## Out of scope (deferred)

- **PR-PROD-4.b** — `admin/dashboard.rs` implementation. This
  runbook is the spec; the implementation is a follow-up PR.
- **PR-PROD-4.c** — Frontend dashboard React shell.
- **PR-PROD-4.d** — SSE stream handlers.

The spec lands first so the implementation PRs are reviewable
in isolation against this contract.
