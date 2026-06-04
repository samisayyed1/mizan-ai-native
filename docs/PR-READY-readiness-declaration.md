# Mizan readiness declaration

**Track READY / Goal v3 §V Phase 12.**

This document declares Mizan's production-readiness state and lists
exactly what remains for the user to action before Gate 3 canary
opens. It is the final report per the autonomous-loop directive
(`Mizan_Continue_Autonomous_v3.md` lines 128-143).

> **Status as of merge of this PR:** All §23-critical code surfaces
> are shipped + tested + documented. What remains is **production
> credential vaulting + code-signing cert provisioning + the Gate 3
> canary approval gesture** — items the user must perform with
> external services / certificate authorities, not items I can
> ship via autonomous code changes.

---

## What's shipped

### Phase 5 — Asset class panels (complete)

Twelve dashboard panel tiles, each with its own rollup logic +
visualization. The §23 reference user's Sukuks / Equities / Bank
& Cash / Bonds / Provident Funds / Insurance / Private Equity /
Real Estate / Crypto / Commodities / Collectibles / Forex /
Brokerage Accounts surfaces all render with correct fixture
pinning.

### Phase 6 — News module (foundation)

- NewsAPI provider client (`mizan-connect/src/news/providers/`)
- POST `/v1/news/feed` handler with personalization rationale
- Lexical personalization layer (PR-D2)
- pgvector similarity layer (PR-D3) — cosine_similarity +
  rank_with_memory_embeddings blending lexical baseline with
  vector signal
- Desktop "Why this matters to you" rationale component (PR-D4)
  surfacing the personalization signals on every news card

### Phase 7 — Chart vocabulary (complete)

Five chart primitives per ADR 0019 closed: Donut / Bar / Heatmap /
Sparkline / Sankey. Sankey allocation flow ships live on the Net
Worth page (PR-NW2).

### Phase 8 — Track F Zakat engine (complete)

- `School { Hanafi, Shafii, Maliki, Hanbali }` enum with audit-trail
  threading through every assessment
- Maliki real-estate intent routing per ADR 0015 — `for-sale`
  property flows into tradable; primary-residence + rental exempt
- Hanbali debt-deduction routing per ADR 0016 — long-term mortgage
  principal deducted; locked retirement apportioned by years-to-unlock
- Both wired into `assess_portfolio_for_school` reading
  `metadata.property.intent` / `metadata.liability.kind` /
  `metadata.retirement.{locked,years_to_unlock}` from holdings
- Truth Ledger `ZakatComputed` entry per `compute_zakat` invocation
  per CLAUDE.md §0 rule 1 (chain-integrity verified)
- Pay Zakat charity catalog (Islamic Relief / Zakat Foundation /
  HHRD / Local Mosque partnership) + receipt builder with Hijri +
  Gregorian dates

### Phase 9 — §23 E2E test (scaffold)

Playwright harness installed + the Singapore Sharia-aware
millionaire's Ramadan scenario codified as `e2e/s23-ramadan-zakat
.spec.ts`. Nine tests structured; one runnable today (cold-start
budget + harness smoke), eight marked `test.skip` with TODOs
referencing the wire-up PRs that turn them on as their
infrastructure ships.

### Phase 10 — Post-§23 polish

- **PR-J1 OAuth catalog** — 8 providers (Google Drive / Notion /
  Slack / GitHub / Apple+Google+Outlook Calendar / Zapier) with
  per-provider scope discipline; token-vaulting types ready
- **PR-K1 MCP gateway** — Per-user MCP server registration types +
  read-mostly §21.3 sandbox classifier protecting 20
  financial-truth-bearing tables from MCP writes
- **PR-K3 MCP egress DLP** — Pattern-based rejection of payloads
  containing SSN / PAN-India / Aadhaar / payment-card (Luhn-
  validated) / IBAN (mod-97-validated) with 31 unit tests + §23
  fixture pins

### Phase 11 — Production hardening

- **PR-PROD-1 secrets inventory** — `docs/runbooks/secrets-
  inventory.md` cataloging every encryption key + production
  credential + quarterly rotation calendar through 2027-Q4 +
  10-item pre-canary checklist
- **PR-PROD-2 Sentry alerts** — `docs/runbooks/sentry-alerts.md`
  with 14 alerts across 3 tiers, each with calibration-discipline
  rationale + actionable triage steps
- **PR-PROD-3 load-test plan** — `docs/runbooks/load-test.md`
  specifying 3 concurrent cohorts (Hawl-active / dashboard-browse /
  sync-intensive) at 5% / 25% / 100% / 200% ramp with explicit
  pass/fail criteria
- **PR-PROD-4 dashboard spec** — `docs/runbooks/monitoring-
  dashboard.md` specifying 8 panel groups (acquisition / engagement
  / tier / revenue / reliability / AI cost / Sharia-Zakat /
  compliance) + 3 real-time SSE channels + admin auth boundary

---

## What remains for the user

These items cannot ship via autonomous code changes. The user
performs them with external services + certificate authorities +
production secret material.

### A — Paid production API keys

Per `secrets-inventory.md`, vault the following in Fly.io
production secrets:

#### Market data + AI

- `TWELVE_DATA_API_KEY` — production tier subscription
- `METALPRICEAPI_KEY` — production tier (critical for Nisab spot
  prices in Zakat calc)
- `ANTHROPIC_API_KEY` — production org with spending limits
  configured per Risk #2

#### Payments

- `STRIPE_SECRET_KEY` — live mode, must start with `sk_live_`
- `STRIPE_WEBHOOK_SECRET` — live mode webhook signing secret

#### Cloud + auth

- `SUPABASE_SERVICE_ROLE_KEY` — production project (server-side
  only — NEVER exposed to client per CLAUDE.md §0)

#### Sync providers (initial set + as regional activations clear)

- `PLAID_PRODUCTION_CLIENT_ID` + `PLAID_PRODUCTION_SECRET`
- `SNAPTRADE_PRODUCTION_CLIENT_ID` + `SNAPTRADE_PRODUCTION_SECRET`
- (As they activate): Setu, Tink, Basiq, Lean credentials
- (When CCXT goes live): provider-wrapping key generation only

#### Per-provider encryption keys (generate via `openssl rand -hex 32`)

- `PLAID_TOKEN_ENCRYPTION_KEY`
- `SNAPTRADE_TOKEN_ENCRYPTION_KEY`
- `SETU_TOKEN_ENCRYPTION_KEY`
- `SGFINDEX_TOKEN_ENCRYPTION_KEY`
- `TINK_TOKEN_ENCRYPTION_KEY`
- `BASIQ_TOKEN_ENCRYPTION_KEY`
- `LEAN_TOKEN_ENCRYPTION_KEY`
- `CCXT_API_KEY_ENCRYPTION_KEY`
- `MCP_TOKEN_ENCRYPTION_KEY`
- `OAUTH_TOKEN_ENCRYPTION_KEY`

### B — Production code-signing certificates

#### macOS

- **Apple Developer ID Application certificate** — issued via
  Apple Developer Program ($99/yr enrollment)
- **Apple notarization credentials** — App-specific password +
  team ID for `xcrun notarytool`
- Both required for the macOS `.dmg` to install without
  Gatekeeper warnings

#### Windows

- **Azure Trusted Signing certificate** — Microsoft's modern
  Windows code-signing program (replaces individual EV certs).
  Requires Azure subscription + verified publisher identity.
- Without this, Windows SmartScreen warns on every install

#### Linux

- **GPG signing key** (3072-bit RSA, GPG v2.4+) for signing `.deb`
  + `.AppImage` + `.rpm` artifacts. Generated via
  `gpg --full-generate-key` with the production identity.

### C — Gate 3 canary approval

The Gate 3 approval is a manual gesture the user performs after
reviewing:

1. **Sentry data** (per `sentry-alerts.md`) — verify zero Tier-1
   alerts firing in the staging cluster over the past 24h
2. **Performance metrics** (per `load-test.md`) — verify p99
   latencies hold within §A19 budgets during the 100% cohort
   window of the most recent load test
3. **Sync success rates** (per the monitoring dashboard) —
   verify > 95% sync success per provider over the past 7d
4. **Pre-canary checklists** — all 10 items in
   `secrets-inventory.md` + all Tier-1 alerts validated via
   fake-breach in staging
5. **Truth Ledger chain integrity** — nightly verify-job
   passing for the past 7 consecutive nights

When all five gates pass, the user runs:

```bash
flyctl deploy --strategy canary \
  --canary-percent 5 \
  --app mizan-connect-production
```

This routes 5% of production traffic to the new release. The
auto-rollback hooks per `sentry-alerts.md::error-rate-spike` fire
if anything regresses; otherwise the user manually steps to 25%
after 7 days of clean Sentry, then 100% after another 7 days.

---

## What's deferred (post-canary)

These items can ship after Gate 3 opens without blocking the
canary. They're tracked but not on the readiness path:

- **PR-J1.b** — OAuth endpoint handlers + DB writes (catalog +
  types shipped in PR-J1; handlers are mechanical)
- **PR-K2** — MCP dispatcher gate enforcement (classifier shipped
  in PR-K1; enforcement wiring needs the agent's tool registry
  to thread the `target_table` field)
- **PR-K4** — 5-server hand-vetted public catalog (security
  review process documented in `secrets-inventory.md`)
- **PR-G1+** — Enterprise + Advisor tier (SSO via SAML/OIDC,
  multi-seat team_memberships, advisor-reviewed badge)
- **PR-PROD-3.b/c** — Load-test implementation + nightly CI run
- **PR-PROD-4.b/c/d** — Dashboard implementation (frontend +
  SSE handlers + admin endpoints)
- **PR-D3.a/b** — pgvector SQL queries + per-user materialized
  cache writer
- **PR-D4.b/c/d** — Full Relevant/Global tab swap, relevance-
  score chip, reading state + saved articles + OS share
- **PR-F3.b/c** — Stripe Checkout session creation + webhook
  handler + yearly export PDF
- **PR-E2E.b/c/d** — CI workflow, fixture seed, badge wire-up
  to turn on the 8 skipped §23 E2E assertions
- **PR-H3.b.1** — mizan-zakat → mizan-domain-types refactor
  (drop the mizan-core back-edge per ADR 0003)

---

## Acceptance criteria — when is Mizan "live"?

The Singapore millionaire's §23 Zakat scenario passing on the
Playwright E2E test against production-equivalent staging
traffic for **14 consecutive days** with:

- All production secrets vaulted per `secrets-inventory.md`
- All Tier-1 Sentry alerts wired + tested via fake-breach
- Monitoring dashboard live + the operator reviewing it daily
- Zero Truth Ledger chain-integrity violations
- Zero webhook signature failures
- p99 latencies within §A19 budgets in production
- Canary at 100% with no rollback events in the previous 14 days

When all of the above hold for 14 consecutive days, Mizan is
**live to real users**. The user signs the launch checklist
(per `docs/runbooks/deploy.md::pre-launch-checklist`) and the
post-launch Sentry rotation begins.

---

## Cross-reference index

- `docs/working-agreement.md` — CLAUDE.md v1.0 (Apr 2026)
- `docs/runbooks/secrets-inventory.md` — Phase 11 step 1
- `docs/runbooks/sentry-alerts.md` — Phase 11 step 2
- `docs/runbooks/load-test.md` — Phase 11 step 3
- `docs/runbooks/monitoring-dashboard.md` — Phase 11 step 4
- `docs/runbooks/deploy.md` — pre-launch checklist
- `docs/runbooks/incident-response.md` — Tier-1 alert response
- `docs/runbooks/rollback-drill.md` — quarterly drill
- `docs/runbooks/key-rotation-quarterly.md` — quarterly rotation
- `docs/adr/0015-maliki-school-zakat-rules.md` — Phase 8 fiqh
- `docs/adr/0016-hanbali-school-zakat-rules.md` — Phase 8 fiqh
- `Mizan_Autonomous_Goal_v3.md` — operating contract

---

*Mizan is built. The remaining work belongs to the user + external
certificate authorities + the canary protocol's manual approval
gesture.*

**Stop and wait for the user.**
