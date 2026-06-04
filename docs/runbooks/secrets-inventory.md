# Secrets inventory — Mizan production

**Track PROD PR-PROD-1 / Goal v3 §V Phase 11 step 1.**

Authoritative catalog of every secret Mizan production needs vaulted
before Gate 3 canary approval. Updated whenever a new provider lands
or a key rotation completes.

> **CRITICAL — read first.** This file lists *names + purposes*, never
> *values*. The values live in:
>
> - **Supabase + Fly.io secrets vault** (production) — accessed via
>   `flyctl secrets list -a mizan-connect-production`
> - **`mizan-connect/.env.fly`** (development, chmod 600, gitignored)
> - **`mizan-4/.env`** (desktop dev, chmod 600, gitignored)
>
> If you see a literal secret value committed to this file or any
> other repo file, treat it as a P0 incident: rotate the secret
> immediately + file an incident report per `incident-response.md`.

---

## Inventory schema

Every secret carries:

| Field | Meaning |
|---|---|
| **Name** | `SCREAMING_SNAKE_CASE` env var name used by mizan-connect / mizan-4 |
| **Purpose** | What the secret is for + which subsystem reads it |
| **Vault** | Where the production value is stored |
| **Owner** | Who can rotate it (typically `sami@maevemodels.co.uk`) |
| **Rotation cadence** | How often the secret should be re-keyed |
| **Next rotation** | Calendar entry — last-rotated-at + cadence |
| **Audit notes** | Any deviation from the rotation cadence or known caveats |

---

## Per-provider encryption keys

Every external provider that stores user tokens encrypts them at rest
with a per-provider AES-256-GCM key. Generation:
`openssl rand -hex 32 | tr -d '\n'`.

### `PLAID_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts Plaid access tokens stored in
  `user_plaid_connections.access_token_encrypted`.
- **Vault** — Fly.io secrets (`mizan-connect-production`).
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly + on any suspected compromise.
- **Next rotation** — 2026-09-04 (90 days from initial seed).
- **Audit notes** — Plaid access tokens are long-lived; rotation
  requires re-encrypting all stored tokens. Use
  `mizan-connect/scripts/rotate-encryption-key.sh PLAID_TOKEN_ENCRYPTION_KEY`
  which preserves the previous key as `PLAID_TOKEN_ENCRYPTION_KEY_PREV`
  for a 7-day overlap window.

### `SNAPTRADE_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts SnapTrade `user_secret` per CLAUDE.md §16
  bright line (never plaintext).
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — 2026-09-04.
- **Audit notes** — SnapTrade's `user_secret` is the auth credential
  for all read-only brokerage calls; a leak would let an attacker
  enumerate the user's holdings.

### `SETU_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts Setu (India Account Aggregator) consent
  artefacts.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending Setu production tier activation.
- **Audit notes** — Inactive until Track B PR-B5 ships the Setu
  cloud integration.

### `SGFINDEX_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts SGFinDex (Singapore Singpass) OAuth tokens.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending SGFinDex production credentials.
- **Audit notes** — Requires Singpass redirect_uri matching the
  production app per ADR 0022.

### `TINK_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts Tink (EU PSD2) OAuth tokens.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending Tink production credentials.

### `BASIQ_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts Basiq (Australia CDR) OAuth tokens.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending Basiq production credentials.

### `LEAN_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts Lean (UAE banking) OAuth tokens.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending Lean production credentials.

### `CCXT_API_KEY_ENCRYPTION_KEY`

- **Purpose** — Encrypts user-supplied crypto exchange API keys
  (Binance / Coinbase / Kraken / etc.) for CCXT read-only sync.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly. **Critical**: rotation must NOT
  rotate the user's exchange API key — only the wrapping key. Per
  ADR 0026, CCXT scopes are read-only at the exchange level.
- **Next rotation** — Pending production CCXT activation.

### `MCP_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts MCP server auth headers + bearer tokens
  per ADR 0014. Used by `mcp_servers.auth_credential_encrypted`.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending MCP gateway production deploy.
- **Audit notes** — A leak of the MCP credentials would let an
  attacker impersonate the user against their connected MCPs.

### `OAUTH_TOKEN_ENCRYPTION_KEY`

- **Purpose** — Encrypts OAuth provider tokens (Google Drive,
  Notion, Slack, GitHub, Calendars, Zapier) per ADR 0025.
- **Vault** — Fly.io secrets.
- **Owner** — sami@maevemodels.co.uk.
- **Rotation cadence** — Quarterly.
- **Next rotation** — Pending PR-J1.b production deploy.

---

## Production market-data + payment + AI keys

### `TWELVE_DATA_API_KEY` (production tier)

- **Purpose** — Real-time + historical market data (stocks, ETFs,
  forex, crypto).
- **Vault** — Fly.io secrets.
- **Rotation cadence** — Annual + on suspected compromise.
- **Next rotation** — 2027-06-05 (one year from initial seed).

### `METALPRICEAPI_KEY` (production tier)

- **Purpose** — Gold / silver / platinum / palladium spot prices.
  Critical for Nisab calculation.
- **Vault** — Fly.io secrets.
- **Rotation cadence** — Annual.
- **Next rotation** — 2027-06-05.

### `STRIPE_SECRET_KEY` (live mode)

- **Purpose** — Billing (subscription management) + Pay Zakat flow
  (charity-account routing per ADR 0038).
- **Vault** — Fly.io secrets.
- **Rotation cadence** — On any suspected compromise. Stripe doesn't
  enforce rotation but recommends annual audit.
- **Next rotation** — 2027-06-05.
- **Audit notes** — Test-mode key is in `mizan-connect/.env.fly` for
  staging; production key is Fly.io-vaulted only.

### `STRIPE_WEBHOOK_SECRET` (live mode)

- **Purpose** — Verifies inbound Stripe webhook signatures per
  CLAUDE.md §5 (mandatory webhook verification).
- **Vault** — Fly.io secrets.
- **Rotation cadence** — On webhook-endpoint URL change OR suspected
  compromise. Stripe supports multi-secret rotation (the 5-case
  pattern in our existing tests).
- **Next rotation** — On endpoint change.

### `ANTHROPIC_API_KEY` (production)

- **Purpose** — Claude model access for the managed AI proxy.
- **Vault** — Fly.io secrets.
- **Rotation cadence** — Annual + on suspected compromise.
- **Next rotation** — 2027-06-05.
- **Audit notes** — Set tight org-level spending limits (per
  CLAUDE.md §15 + Risk #2) to bound runaway AI cost.

### `SUPABASE_SERVICE_ROLE_KEY` (production project)

- **Purpose** — Server-side admin operations from mizan-connect
  (audit-log writes, user provisioning, etc.).
- **Vault** — Fly.io secrets.
- **Rotation cadence** — Annual.
- **Next rotation** — 2027-06-05.
- **Audit notes** — **NEVER expose to the client** (CLAUDE.md §0
  rule). Only the Mizan Connect Fly.io app reads this key.

---

## Updater / signing keys (separate from secrets)

These aren't in the secrets-rotation table because they're handled
under `docs/runbooks/updater-key-rotation.md`. Cross-referenced here
for completeness:

- **`MIZAN_UPDATER_SIGNING_KEY`** — Tauri updater signing per
  ADR 0009. Rotation: triennial; expired key kept for 6-month
  overlap.
- **`APPLE_DEVELOPER_ID_CERT`** — macOS code-signing + notarization
  per Apple Developer Program. Renewed annually.
- **`AZURE_TRUSTED_SIGNING_CERT`** — Windows code-signing per
  Azure Trusted Signing certificate program.
- **`LINUX_SIGNING_KEY`** — Debian/RPM signing key. GPG-based,
  3072-bit RSA per `docs/runbooks/deploy.md`.

---

## Pre-canary checklist

Run this checklist *before* approving the 5% Gate 3 canary cohort:

- [ ] Every secret in the per-provider encryption-key section above
      is set in Fly.io vault (`flyctl secrets list` shows each name).
- [ ] No secret has ever been committed to git history. Verify via
      `gitleaks detect --no-banner` on the full repo history (PR-H6
      CI gate covers ongoing commits, but a pre-canary re-verify is
      cheap).
- [ ] Each market-data / payment / AI key is on the correct
      production tier (not dev / test).
- [ ] `STRIPE_SECRET_KEY` starts with `sk_live_` (NOT `sk_test_`).
- [ ] Multi-secret webhook rotation tested in staging within the
      past 7 days (per CLAUDE.md §5 — past bug).
- [ ] `SUPABASE_SERVICE_ROLE_KEY` is set on Fly.io ONLY (no client
      exposure — verify via `grep -r 'SUPABASE_SERVICE_ROLE' mizan-4/`
      returns zero hits outside `// safety: ...` comments).
- [ ] `ANTHROPIC_API_KEY` org-level spending limit configured per
      CLAUDE.md §15 monitoring dashboard.
- [ ] Each provider encryption key matches `^[0-9a-f]{64}$` (32
      bytes hex; openssl-generated).
- [ ] Updater signing key + Apple Developer ID + Azure Trusted
      Signing all valid (not expired in the next 90 days).

When every box is ticked, file the Gate 3 canary request per
`docs/runbooks/deploy.md`.

---

## Quarterly rotation calendar

Recurring rotation entries (added to the operations calendar so the
last week of each quarter triggers the rotation drill per
`key-rotation-quarterly.md`):

| Quarter | Rotation date | Drill owner |
|---|---|---|
| 2026 Q3 | last week of Sep | sami@maevemodels.co.uk |
| 2026 Q4 | last week of Dec | sami@maevemodels.co.uk |
| 2027 Q1 | last week of Mar | sami@maevemodels.co.uk |
| 2027 Q2 | last week of Jun | sami@maevemodels.co.uk |
| 2027 Q3 | last week of Sep | sami@maevemodels.co.uk |
| 2027 Q4 | last week of Dec | sami@maevemodels.co.uk |

Each drill rotates every quarterly-cadence secret + verifies the
7-day-overlap pattern (new key live, previous key marked
`_PREV` for one week, then deleted). Drill output appended to
`docs/runbooks/key-rotation-quarterly.md` for the audit trail.

---

## Track PROD follow-ups

This runbook is the foundation; PR-PROD-2 / PR-PROD-3 / PR-PROD-4
build on top of it:

- **PR-PROD-2** — Sentry alert tuning per Spec §15.10. Alerts fire
  on error-rate / performance-budget / sync-success / churn / failed
  payment / AI cost spikes.
- **PR-PROD-3** — Load testing against production-scale fixture
  data (1000+ synthetic users, §A19 budget verification).
- **PR-PROD-4** — Monitoring dashboard live per Spec §15 with
  acquisition / engagement / tier distribution / revenue /
  reliability / AI cost / Sharia / Zakat / compliance metrics.

After all four PROD PRs ship + the canary checklist above passes,
Phase 12 PR-READY can declare the production-readiness state.
