# Sentry alert tuning — Mizan production

**Track PROD PR-PROD-2 / Goal v3 §V Phase 11 step 2.**

Authoritative catalog of every Sentry alert rule wired up on the
Mizan production projects. Updated whenever an alert threshold
changes, a new alert lands, or a noisy alert is retired.

> **Calibration discipline.** Every alert here is *actionable* —
> i.e. when it fires, the on-call engineer has a clear next step
> beyond "look at the dashboard". Noisy alerts that don't drive
> action are retired (move them to `docs/runbooks/sentry-alerts-
> retired.md` with a one-line retrospective). Spec §15.10 fail-
> mode: alerts so noisy they're ignored are worse than no alerts.

---

## Sentry project layout

Three Sentry projects per Goal v3 §A19 + Spec §15:

| Project | Source | Sampling | DSN env |
|---|---|---|---|
| `mizan-desktop-prod` | mizan-4/apps/tauri | 100% errors, 10% performance | `MIZAN_SENTRY_DESKTOP_DSN` |
| `mizan-connect-prod` | mizan-connect (Fly.io) | 100% errors, 25% performance | `MIZAN_SENTRY_CONNECT_DSN` |
| `mizan-ai-prod` | mizan-4/crates/ai + cloud agent proxy | 100% errors, 100% LLM-cost spans | `MIZAN_SENTRY_AI_DSN` |

Each alert below names which project it lives on.

---

## Tier-1 (page on-call)

Alerts severe enough to wake someone up. Routed to the on-call
PagerDuty rotation via Sentry → PagerDuty integration.

### `error-rate-spike`

- **Project** — all three.
- **Condition** — error rate > 2× rolling 24h average sustained for
  ≥ 15 min.
- **Why this threshold** — 2× rolling avg catches deploy regressions
  + provider outages without firing on every transient burst.
  Sustained 15 min filters out single-minute spikes from network
  jitter.
- **Action** — Page on-call. Triage steps: check recent deploys,
  Fly.io provider status, Supabase status, Plaid + SnapTrade
  status pages.
- **Auto-rollback** — Goal v3 canary protocol: if this fires during
  the first 24h of a canary cohort, automatically pause expansion
  (Sentry → release-API hook). Manual rollback remains the
  operator's call.

### `performance-budget-breach`

- **Project** — mizan-desktop-prod + mizan-connect-prod.
- **Condition** — p99 latency exceeds the §A19 budget by ≥ 2× for
  ≥ 15 min:
  - Desktop cold start: budget 1.2s → fires at p99 > 2.4s
  - Chart paint (cached): budget 200ms → fires at p99 > 400ms
  - Endpoint read p99: budget 300ms → fires at p99 > 600ms
  - Endpoint sync p99: budget 800ms → fires at p99 > 1600ms
  - Agent intent classification p99: budget 500ms → fires at > 1s
  - Agent read tool p99: budget 2s → fires at > 4s
  - Agent write tool p99: budget 5s → fires at > 10s
- **Why these thresholds** — 2× over budget for 15 min is the
  threshold at which the breach is a real product issue (not
  a single slow trace from a cold cache miss).
- **Action** — Page on-call. Triage steps: check Sentry performance
  view → trace the slowest endpoint → check Fly.io scaler events
  + Supabase slow-query log.

### `sync-success-degraded`

- **Project** — mizan-connect-prod.
- **Condition** — Plaid / SnapTrade / Setu / Tink / Basiq / Lean
  sync success rate < 95% over rolling 1h (per provider).
- **Why this threshold** — Below 95% indicates real provider
  trouble (auth expiry, schema drift, rate-limit hit). The
  per-provider scope means a Plaid outage doesn't mask a Setu
  degradation.
- **Action** — Page on-call. Triage: check provider status page,
  read recent sync_run_ledger entries for the failing provider,
  verify webhook signing keys haven't rotated.

### `webhook-signature-failure`

- **Project** — mizan-connect-prod.
- **Condition** — Any webhook endpoint returns 401 (signature
  failure) > 0.1% of calls over rolling 1h.
- **Why this threshold** — Even a single signature failure on a
  paid webhook is suspicious; 0.1% catches both real attacks AND
  silent secret rotations that broke the wired-up secret. Past
  bug per CLAUDE.md §5: a Stripe webhook silently rotated and a
  paid user lost their subscription.
- **Action** — Page on-call. Triage: check if the webhook secret
  has rotated (multi-secret pattern), check recent commits to
  webhook signing code, lock the affected endpoint if attack
  suspected.

---

## Tier-2 (team channel)

Alerts important but not page-worthy. Routed to `#mizan-ops` Slack.

### `churn-rate-spike`

- **Project** — mizan-connect-prod (billing events).
- **Condition** — Daily cancellation rate > 1.5× rolling 30-day
  average for ≥ 2 consecutive days.
- **Why this threshold** — A single noisy day doesn't trigger;
  2-day sustained spike means a real product issue (likely a
  recent UX regression).
- **Action** — Product retro: bisect against the last 7 days of
  ships; reach out to the cancelled cohort for qualitative
  feedback.

### `failed-payment-rate`

- **Project** — mizan-connect-prod (Stripe webhook events).
- **Condition** — Failed payment rate > 5% over rolling 24h
  (excluding card-declined-by-issuer — that's user-side and not
  actionable on our end).
- **Why this threshold** — Stripe-side issues (their service,
  not the issuer's) > 5% is rare; firing here means we have a
  webhook handler bug or Stripe outage we should respond to.
- **Action** — Check Stripe status, verify our webhook handlers
  haven't deployed a regression in the last 24h.

### `ai-cost-spike`

- **Project** — mizan-ai-prod.
- **Condition** — Hourly AI cost > 2× the rolling-7-day hourly
  average, sustained for ≥ 30 min.
- **Why this threshold** — Per CLAUDE.md §15 + Risk #2 (AI cost
  runaway under new tool registry). 2×-sustained means either a
  hot user is hitting an unbounded loop OR a regression dropped
  prompt-cache hit rate.
- **Action** — Identify the top spender via the per-user cost
  panel (PR-PROD-4 monitoring dashboard). Verify prompt-cache hit
  rate hasn't dropped below the 80% floor per CLAUDE.md §15.6.
  Consider a model-routing emergency (push complex reasoning
  back to small fast tier for 1 hour while triaging).

### `prompt-cache-hit-rate-degraded`

- **Project** — mizan-ai-prod.
- **Condition** — Anthropic prompt-cache hit rate < 70% over
  rolling 6h (target floor: 80% per CLAUDE.md §15.6).
- **Why this threshold** — Below 70% the cost math no longer
  works at the planned tier pricing. A 1h dip can be benign
  (system prompt update); 6h means the cache strategy has
  drifted.
- **Action** — Check recent system-prompt updates (CHANGELOG of
  `crates/ai/src/prompts/CHANGELOG.md`); verify cache breakpoints
  are still in the right place; look for a tool whose
  per-invocation prefix is busting the cache.

### `truth-ledger-chain-integrity-violation`

- **Project** — mizan-desktop-prod.
- **Condition** — Any `LedgerIntegrityError` reported from
  `TruthLedger::verify()` in the last 24h.
- **Why this threshold** — Zero tolerance per CLAUDE.md §0 rule 1.
  A single integrity error means the chain is broken; not
  page-worthy because the user's data is recoverable from the
  cloud mirror, but team channel ASAP.
- **Action** — Pull the failing ledger snapshot, restore from
  the cloud mirror, file an incident report. If integrity
  errors are coming from production, halt the canary
  expansion immediately.

### `mcp-sandbox-rejection-rate`

- **Project** — mizan-connect-prod.
- **Condition** — `mcp_call_log` shows `allowed=false` rate > 1%
  over rolling 24h (excludes self-registered MCP servers — we
  expect a higher reject rate there).
- **Why this threshold** — On a per-Vetted-server basis, > 1%
  rejections means either the server is misbehaving (try-write
  attempts) OR our PROTECTED_TABLES classifier has a bug.
- **Action** — Look at the top-rejected tool. If misbehaving
  server, delist from the public catalog. If classifier bug,
  patch + redeploy.

### `dlp-rejection-rate`

- **Project** — mizan-connect-prod.
- **Condition** — Egress DLP `has_findings=true` rate > 0.5% over
  rolling 24h (excludes self-registered MCPs).
- **Why this threshold** — Catches both real exfil attempts AND
  any pattern that's too eager (false-positives we need to tune
  down per PR-K3.b).
- **Action** — Inspect top categories triggering; if false
  positives, file PR-K3.b refinement; if real exfil, delist the
  server + file an incident report.

---

## Tier-3 (product channel)

Alerts useful for product health but not operational. Routed to
`#mizan-product` Slack.

### `sharia-screening-coverage`

- **Project** — mizan-connect-prod.
- **Condition** — Less than 90% of unique holdings in `holdings`
  have a fresh (≤ 30 days old) `sharia_status` value.
- **Why this threshold** — Per ADR 0012 we promised users
  refreshed screening monthly. Below 90% coverage means the
  AAOIFI worker isn't keeping up.

### `zakat-engine-school-distribution`

- **Project** — mizan-connect-prod.
- **Condition** — Weekly distribution of Zakat assessments by
  school deviates > 3σ from the 12-month rolling mean.
- **Why this threshold** — Surfaces whether the Maliki/Hanbali
  rules (per ADRs 0015, 0016) are getting picked up by users. A
  sudden zero on Maliki could mean the UI selector broke.

### `pay-zakat-receipt-mismatch`

- **Project** — mizan-connect-prod.
- **Condition** — Stripe Connect `payment_intent.succeeded` count
  doesn't match `zakat_receipts` rows over rolling 24h.
- **Why this threshold** — Zero tolerance for missing receipts
  (Sec 80G compliance, AML/KYC per CLAUDE.md §16.2). Any drift
  is a P1 reconciliation task.

---

## Retired alerts

Alerts we turned off because they didn't drive action. Listed
here so future authors know not to re-introduce the same pattern
without addressing the noise issue first.

*None retired yet. Add entries when alerts are retired with the
format:*

- **`alert-name`** — Retired YYYY-MM-DD. Reason: ... .
  Lesson learned: ... .

---

## Tier-1 / Tier-2 / Tier-3 escalation summary

| Tier | Channel | Response SLA | Owner |
|---|---|---|---|
| 1 | PagerDuty on-call | 15 min ack, 1h triage | rotating on-call |
| 2 | `#mizan-ops` | next business day | rotating on-call |
| 3 | `#mizan-product` | next product retro | product owner |

---

## Pre-canary validation

Before the 5% canary cohort goes live, the operator validates that
every Tier-1 alert above:

- [ ] Has a corresponding Sentry alert rule configured + tested in
      staging (one fake breach per alert)
- [ ] Routes to PagerDuty correctly (test page received)
- [ ] Has a documented triage runbook (linked from this file when
      it lives elsewhere)
- [ ] Has an auto-rollback hook wired up where applicable
      (`error-rate-spike` is the priority case)

Tier-2 + Tier-3 alerts validated via channel test messages.

---

## Track PROD follow-ups

This runbook is the alert spec. The actual Sentry alert-rule
configuration as code lands in PR-PROD-2.b (or via Sentry's
web UI + a screenshot trail). PR-PROD-3 load test + PR-PROD-4
monitoring dashboard build on top:

- **PR-PROD-2.b** — Sentry alert rules as code (terraform or
  Sentry API JSON)
- **PR-PROD-3** — Load testing infrastructure + p99 latency
  baseline measurement
- **PR-PROD-4** — Monitoring dashboard live per Spec §15
- **Phase 12 PR-READY** — Final readiness declaration once all
  PROD PRs ship + Tier-1 alerts validated
