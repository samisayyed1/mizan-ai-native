# Mizan Shipping Checklist

**Bridge from "Mizan is built" to "Mizan is live to real users."**

This is the operator's playbook. The autonomous build is complete
(see `docs/PR-READY-readiness-declaration.md`). What remains can't
ship via code — it requires you to sign up for paid services, get
approved by KYC processes, vault secrets, provision code-signing
certificates, and step the canary rollout up over four weeks.

Pace yourself. Most steps run in parallel — the slow ones are
underwriting / KYC reviews you can't speed up. Work 2-4 hours/day
and you'll be live in ~5-6 weeks. Cost at launch is ~**$650/month**.

> **One rule that overrides everything else:** never paste a literal
> secret value into git, Slack, email, or any AI tool's chat
> window. Production secrets live in **Fly.io secrets vault** and
> in your chmod-600 `.env.fly` file. Nowhere else.

---

## Part 1 — Paid API Keys (~1-2 weeks)

Group A is mandatory before any canary traffic. Group B can
activate after launch as regional coverage expands; SGFinDex /
Setu / Tink / Basiq / Lean are explicitly post-launch per the
readiness declaration.

### Group A — Must have before any canary traffic

#### A1. Stripe (live mode)

- **What it's for**: Subscription billing + the Pay Zakat charity
  donation flow. Without this, no user can pay Mizan, and no user
  can route Zakat to Islamic Relief / Zakat Foundation / HHRD /
  partnership mosques.
- **Sign-up URL**: https://dashboard.stripe.com/register
- **Estimated monthly cost**: 2.9% + 30¢ per transaction (no
  monthly base). At ~100 paid users averaging $20/mo, ~$80/mo in
  Stripe fees. Pay Zakat donations route directly to charity
  Stripe Connect accounts and don't accrue Mizan fees.
- **KYC / business verification**: Stripe asks for the business
  entity (LLC, corp, or sole-trader), tax ID, owner ID
  verification, and the business's bank account for payouts.
  Maeve Models Ltd (your company) needs articles of association
  or equivalent. Plan for 1-3 business days from submission to
  approval.
- **Time from sign-up to credentials**: ~2-3 days assuming KYC
  passes first review.
- **Recommended starting tier**: Standard. Connect platform mode
  needs to be enabled separately for Pay Zakat charity routing —
  request via Stripe support after Standard activates.
- **Env vars**:
  - `STRIPE_SECRET_KEY` (must start with `sk_live_`)
  - `STRIPE_WEBHOOK_SECRET` (live webhook signing secret)
- **Vault command**:

  ```bash
  flyctl secrets set \
    STRIPE_SECRET_KEY="sk_live_..." \
    STRIPE_WEBHOOK_SECRET="whsec_..." \
    --app mizan-connect-production
  ```

- **Spending cap / rate limit**: Set a Stripe Radar fraud rule
  capping single-transaction value at $50,000 (covers the §23
  millionaire's annual Zakat without blocking legitimate use).
- **Risk if misconfigured**: Webhook verification failure → paid
  users silently lose their subscription (this was a past bug per
  CLAUDE.md §5; the Tier-1 `webhook-signature-failure` Sentry
  alert catches it).

#### A2. Anthropic (Claude API, production)

- **What it's for**: Claude is the AI agent. Every "what's my net
  worth this week?" / "compute Zakat across all schools" / Today's
  Signal goes through this. Without it, the agent surface is dead.
- **Sign-up URL**: https://console.anthropic.com/
- **Estimated monthly cost**: With 80%+ prompt-cache hit rate (the
  floor per CLAUDE.md §15.6), expect ~$0.50-$2.00 per active user
  per month. At ~100 active users → **~$100-200/mo**. The
  `ai-cost-spike` Tier-2 Sentry alert pages you if a user hits
  unbounded cost.
- **KYC / business verification**: Anthropic accepts standard
  business registration. The Workspace tier requires an Anthropic
  account contact for org-level spending caps.
- **Time from sign-up to credentials**: Same day for the API key;
  1-3 days to set up org-level spend limits if you go through
  sales.
- **Recommended starting tier**: Build tier with **$500/mo
  spending cap** configured in the dashboard. Risk #2 in the
  master plan flags AI cost runaway — caps are mandatory.
- **Env var**: `ANTHROPIC_API_KEY`
- **Vault command**:

  ```bash
  flyctl secrets set \
    ANTHROPIC_API_KEY="sk-ant-..." \
    --app mizan-connect-production
  ```

- **Spending cap / rate limit**: Org-level $500/mo cap in the
  Anthropic dashboard. Per-user rate limit handled by Mizan's
  agent dispatcher (existing per-action credit metering).
- **Risk if misconfigured**: Missing key → agent surface returns
  503 "service_unavailable" cleanly (already coded). Missing
  spending cap → runaway cost on a single bad actor.

#### A3. Supabase (production project)

- **What it's for**: Auth (Sign-in with Google / Apple / Email) +
  Postgres database hosting the cloud-side state (subscriptions,
  Mizan Connect rows, news_items_per_user, user_memory pgvector
  embeddings). Without this, no user can sign in.
- **Sign-up URL**: https://supabase.com/dashboard
- **Estimated monthly cost**: Pro tier **$25/mo** + per-GB storage
  + per-million-request egress. At launch ~$30-50/mo total.
- **KYC / business verification**: None for Pro tier — credit card
  on file.
- **Time from sign-up to credentials**: 5 minutes.
- **Recommended starting tier**: Pro ($25/mo) for production. Free
  tier is OK for staging. Upgrade to Team tier later if you need
  the audit log / SOC 2 attestation.
- **Env vars**:
  - `SUPABASE_URL` (project URL, e.g. `https://abc.supabase.co`)
  - `SUPABASE_ANON_KEY` (public — safe to expose to client)
  - `SUPABASE_SERVICE_ROLE_KEY` (server-side ONLY — NEVER exposed
    to client per CLAUDE.md §0)
- **Vault command**:

  ```bash
  flyctl secrets set \
    SUPABASE_URL="https://your-project.supabase.co" \
    SUPABASE_ANON_KEY="eyJ..." \
    SUPABASE_SERVICE_ROLE_KEY="eyJ..." \
    --app mizan-connect-production
  ```

- **Spending cap / rate limit**: Set a Supabase billing alert at
  $100/mo so an unexpected egress spike pages you before it
  becomes a problem.
- **Risk if misconfigured**: Service-role key leaked to client →
  every user's data is readable by every other user (CLAUDE.md §0
  hard rule). The pre-canary checklist greps the desktop tree to
  verify the service-role key never ships in the binary.

#### A4. Twelve Data (market data, production tier)

- **What it's for**: Real-time stock / ETF / forex / crypto quotes
  that feed the dashboard valuations + the §23 Today's Signal
  insights. Without this, holdings show stale prices.
- **Sign-up URL**: https://twelvedata.com/pricing
- **Estimated monthly cost**: **Pro tier $79/mo** (800
  requests/minute, real-time WebSocket, 50K credits/day).
- **KYC**: None — credit card on file.
- **Time from sign-up to credentials**: Instant.
- **Recommended starting tier**: Pro ($79/mo). Their Enterprise
  tier ($179/mo) only matters at >1000 active users.
- **Env var**: `TWELVE_DATA_API_KEY`
- **Vault command**:

  ```bash
  flyctl secrets set \
    TWELVE_DATA_API_KEY="..." \
    --app mizan-connect-production
  ```

- **Spending cap / rate limit**: Twelve Data tier already caps
  usage; configure a billing alert at $100/mo as a backstop.
- **Risk if misconfigured**: Missing key → dashboard tiles show
  "—" instead of values; the desktop falls back to cached
  prices. Annoying but not catastrophic.

#### A5. MetalpriceAPI (precious-metal spot prices)

- **What it's for**: Gold / silver / platinum / palladium spot
  prices. **Critical** because the Zakat Nisab threshold is
  computed from the silver spot price. Wrong Nisab → wrong Zakat
  number for every Muslim user.
- **Sign-up URL**: https://metalpriceapi.com/
- **Estimated monthly cost**: **Standard tier $30/mo** (10,000
  requests/month, real-time).
- **KYC**: None — credit card.
- **Time from sign-up to credentials**: Instant.
- **Recommended starting tier**: Standard ($30/mo). Their data
  refreshes every 60 seconds which is fine for Nisab (Zakat is
  computed annually per cohort, not by the second).
- **Env var**: `METALPRICEAPI_KEY`
- **Vault command**:

  ```bash
  flyctl secrets set \
    METALPRICEAPI_KEY="..." \
    --app mizan-connect-production
  ```

- **Spending cap**: Built into the tier.
- **Risk if misconfigured**: Wrong Nisab → wrong Zakat number for
  every user. The §23 reference user's Zakat would be off by
  ~$50-200 depending on metal-price drift. Sentry has no specific
  alert for this; pre-canary checklist verifies the key.

#### A6. Plaid (production)

- **What it's for**: US bank account aggregation (Chase / BofA /
  Wells Fargo / etc.). Lets the user connect bank accounts so
  cash balances + activities flow into the dashboard.
- **Sign-up URL**: https://dashboard.plaid.com/signup
- **Estimated monthly cost**: **Production = pay-per-API-call**.
  At ~100 users averaging 10 link events/mo + daily sync:
  ~$100-150/mo. Plaid's per-user pricing is roughly $0.30-0.50/mo
  fully loaded.
- **KYC / business verification**: **This is the slow one.** Plaid
  requires a production application review covering: company
  identity, intended use case, security posture (Mizan's read-
  only scope, token encryption, etc.), and a compliance Q&A.
  Plan for **1-2 weeks** review time. Submit the production
  application as early as possible.
- **Time from sign-up to credentials**: Sandbox credentials
  instantly. Production credentials after the 1-2 week review.
- **Recommended starting tier**: Production with the
  `transactions` + `auth` + `balance` products enabled. Skip
  `investments` until you need brokerage data via Plaid (you have
  SnapTrade for that).
- **Env vars**:
  - `PLAID_PRODUCTION_CLIENT_ID`
  - `PLAID_PRODUCTION_SECRET`
  - `PLAID_TOKEN_ENCRYPTION_KEY` (generate via
    `openssl rand -hex 32 | tr -d '\n'`)
- **Vault commands**:

  ```bash
  # Generate the encryption key locally first
  ENCRYPTION_KEY=$(openssl rand -hex 32 | tr -d '\n')
  echo "Save this key somewhere safe: $ENCRYPTION_KEY"

  flyctl secrets set \
    PLAID_PRODUCTION_CLIENT_ID="..." \
    PLAID_PRODUCTION_SECRET="..." \
    PLAID_TOKEN_ENCRYPTION_KEY="$ENCRYPTION_KEY" \
    --app mizan-connect-production
  ```

- **Spending cap**: Plaid bills per item-linked + per-call. Set
  a Plaid dashboard usage cap at 10x your launch projection so
  one bad actor with a script can't run up the bill.
- **Risk if misconfigured**: Missing encryption key → tokens
  stored in plaintext (CLAUDE.md §0 hard rule violation).
  Missing client_id → bank linking returns 401; user sees a
  reconnect prompt. The pre-canary checklist verifies key is
  set + matches `^[0-9a-f]{64}$`.

#### A7. SnapTrade (production)

- **What it's for**: Brokerage aggregation (Schwab / Fidelity /
  Robinhood / IBKR / Wahed / Saxo / etc.). Lets the user connect
  brokerage accounts so holdings + positions sync.
- **Sign-up URL**: https://snaptrade.com/
- **Estimated monthly cost**: Mid-tier pricing ~**$200-400/mo**
  at launch volume. Per-user pricing is tier-based.
- **KYC / business verification**: SnapTrade compliance review.
  Easier than Plaid but requires demonstrating read-only scope
  (Mizan never trades on behalf of users per ADR 0026) + your
  security practices (encryption-at-rest, no plaintext-token
  bright line). Plan for **3-7 business days**.
- **Time from sign-up to credentials**: Same-day for sandbox;
  3-7 days for production.
- **Recommended starting tier**: Their "Build" tier (~$199/mo)
  covers ~250 active users. Upgrade once you cross that.
- **Env vars**:
  - `SNAPTRADE_PRODUCTION_CLIENT_ID`
  - `SNAPTRADE_PRODUCTION_CONSUMER_KEY`
  - `SNAPTRADE_TOKEN_ENCRYPTION_KEY` (generate via openssl)
- **Vault commands**:

  ```bash
  ENCRYPTION_KEY=$(openssl rand -hex 32 | tr -d '\n')

  flyctl secrets set \
    SNAPTRADE_PRODUCTION_CLIENT_ID="..." \
    SNAPTRADE_PRODUCTION_CONSUMER_KEY="..." \
    SNAPTRADE_TOKEN_ENCRYPTION_KEY="$ENCRYPTION_KEY" \
    --app mizan-connect-production
  ```

- **Spending cap**: SnapTrade tier already caps it.
- **Risk if misconfigured**: Leaked `user_secret` → an attacker
  can enumerate the user's brokerage holdings (CLAUDE.md §16
  bright line). The encryption-key vault prevents this.

#### A8. Fly.io (production hosting)

- **What it's for**: Hosts the Mizan Connect Rust backend that
  the desktop calls. Subscriptions, news, OAuth, MCP gateway,
  Zakat compute all route through Fly. Without this, the desktop
  works offline-only.
- **Sign-up URL**: https://fly.io/app/sign-up
- **Estimated monthly cost**: **~$50-100/mo** for the production
  app (2 shared-cpu VMs in 2 regions + a Postgres extension if
  you don't use Supabase Postgres, which you do). Add ~$30/mo
  for outbound bandwidth at launch scale.
- **KYC**: Credit card on file. No business KYC.
- **Time from sign-up to credentials**: 10 minutes.
- **Recommended starting tier**: Pay-as-you-go. Add a $200/mo
  spending limit via `flyctl billing` to bound runaway scale.
- **Env var**: Fly itself doesn't have a secret — the deployment
  reads `FLY_API_TOKEN` from your local environment when you run
  `flyctl deploy`.
- **Vault**: Fly itself IS the vault. Everything in this
  document's `flyctl secrets set` commands goes here.
- **Risk if misconfigured**: Missing app or wrong region → desktop
  cloud surface fails with timeouts.

### Group B — Can activate after launch as regional coverage expands

These five providers cover regional banking aggregation and
unlock corresponding markets. Mizan ships without them. Activate
one at a time as you onboard users in the relevant geography.

#### B1. SGFinDex (Singapore)

- **What it's for**: Singapore SGFinDex / Singpass aggregation —
  the §23 reference user uses this for CPF + DBS + OCBC balances.
- **Sign-up URL**: https://www.singpass.gov.sg/sgfindex/
- **Estimated monthly cost**: SGFinDex is government-run; the
  cost is the **MAS license fee** to operate as a financial-info
  provider (~SGD 1,000-3,000 setup + annual fees).
- **KYC**: Mandatory MAS registration. Plan **2-4 weeks** for
  paperwork.
- **Time from sign-up to credentials**: After MAS approval, ~1
  week to get sandbox + production redirect_uri configured per
  ADR 0022.
- **Env vars**:
  - `SGFINDEX_CLIENT_ID`
  - `SGFINDEX_CLIENT_SECRET`
  - `SGFINDEX_REDIRECT_URI` (must match the registered one)
  - `SGFINDEX_TOKEN_ENCRYPTION_KEY`
- **Vault command**: same pattern as A6.
- **Risk if misconfigured**: SGFinDex redirect_uri mismatch →
  Singpass OAuth fails closed; user sees a redirect error.

#### B2. Setu (India)

- **What it's for**: India Account Aggregator framework — bank
  + EPF + GST data for Indian users.
- **Sign-up URL**: https://setu.co/data/account-aggregator
- **Estimated monthly cost**: ~₹10,000-30,000/mo
  (~$120-360/mo) depending on volume.
- **KYC**: RBI-licensed AA framework requires a registered
  Indian business entity. Plan **3-6 weeks** including the
  Indian incorporation timeline if you don't already have it.
- **Env vars**: `SETU_CLIENT_ID`, `SETU_CLIENT_SECRET`,
  `SETU_TOKEN_ENCRYPTION_KEY`.
- **Risk if misconfigured**: India users see no bank data; UK
  / SG / US users unaffected.

#### B3. Tink (EU PSD2)

- **What it's for**: European banking aggregation across 25
  EU countries.
- **Sign-up URL**: https://tink.com/
- **Estimated monthly cost**: ~€500-1500/mo. Visa-owned Tink
  pricing is by volume.
- **KYC**: PSD2 license check (you operate as a registered AISP
  or rely on Tink's umbrella). 2-4 weeks.
- **Env vars**: `TINK_CLIENT_ID`, `TINK_CLIENT_SECRET`,
  `TINK_TOKEN_ENCRYPTION_KEY`.
- **Risk if misconfigured**: EU users see no bank data.

#### B4. Basiq (Australia)

- **What it's for**: Australia CDR (Consumer Data Right) banking
  aggregation.
- **Sign-up URL**: https://basiq.io/
- **Estimated monthly cost**: ~AUD 250-800/mo (~$170-540/mo).
- **KYC**: CDR-accredited intermediary registration. 2-3 weeks.
- **Env vars**: `BASIQ_API_KEY`, `BASIQ_TOKEN_ENCRYPTION_KEY`.
- **Risk if misconfigured**: AU users see no bank data.

#### B5. Lean (UAE / KSA)

- **What it's for**: UAE + Saudi Arabia + Egypt banking
  aggregation. §23 reference user has KSA / UAE cash + Sukuks.
- **Sign-up URL**: https://leantech.me/
- **Estimated monthly cost**: ~$200-500/mo at launch.
- **KYC**: UAE CB-licensed; Mizan operates under their umbrella.
  ~2 weeks.
- **Env vars**: `LEAN_APP_TOKEN`, `LEAN_TOKEN_ENCRYPTION_KEY`.
- **Risk if misconfigured**: MENA users see no bank data.

---

## Part 2 — Code-Signing Certificates (~1 week)

Without these, users see scary "this app can't be verified" /
"Windows protected your PC" warnings on install. With these,
the app installs cleanly.

### macOS — Apple Developer ID

- **What it's for**: Signs the `.dmg` so macOS Gatekeeper +
  notarization let it install without warnings. Apple requires
  this for any app distributed outside the App Store.
- **Where to enroll**: https://developer.apple.com/programs/
- **Annual cost**: **$99/year** (individual or company).
- **KYC / business**: Apple D-U-N-S number lookup for company
  membership. Individual membership uses Apple ID + photo ID.
  Plan **1-3 business days**.
- **Time from enrollment to certificate in hand**: ~5 minutes
  after enrollment is approved.
- **Steps**:

  ```bash
  # 1. Enroll at https://developer.apple.com/programs/ ($99/yr)
  # 2. In Xcode → Settings → Accounts, add your Apple ID
  # 3. Manage Certificates → + → Developer ID Application
  # 4. Export the certificate as .p12 (set a strong password)
  # 5. Set up notarization credentials:
  xcrun notarytool store-credentials "mizan-notary" \
    --apple-id "you@example.com" \
    --team-id "ABCD123456" \
    --password "app-specific-password"
  # 6. Wire into the Tauri build pipeline:
  # mizan-4/apps/tauri/tauri.conf.json → bundle.macOS.signingIdentity
  # = "Developer ID Application: Your Name (ABCD123456)"
  # 7. Add notarization to the release script:
  # xcrun notarytool submit mizan.dmg \
  #   --keychain-profile "mizan-notary" --wait
  ```

- **Risk if misconfigured**: Users see "Mizan can't be opened
  because it is from an unidentified developer." They have to
  right-click → Open → confirm twice. Most users abandon at
  this screen.

### Windows — Azure Trusted Signing

- **What it's for**: Signs the `.msi` / `.exe` so Windows
  SmartScreen + Defender don't flag the install. Microsoft
  deprecated standalone EV code-signing certs in 2024; Azure
  Trusted Signing is the replacement.
- **Where to enroll**: https://learn.microsoft.com/en-us/azure/trusted-signing/
- **Annual cost**: **$0.099 per signature** (essentially free
  at Mizan's release cadence) + an Azure subscription with a
  $10-20/mo Standard tier minimum.
- **KYC / business**: Microsoft Partner verification + DUNS +
  domain ownership proof. Plan **3-7 business days**.
- **Time from enrollment to certificate**: After verification,
  same-day to provision the signing identity.
- **Steps**:

  ```bash
  # 1. Provision an Azure subscription if you don't have one
  # 2. Apply for Trusted Signing at the URL above
  # 3. After verification, create a Code Signing Account:
  az ts account create \
    --name mizan-prod \
    --resource-group mizan-rg \
    --sku Basic
  # 4. Create the Certificate Profile:
  az ts profile create \
    --name mizan-prod-profile \
    --account-name mizan-prod \
    --resource-group mizan-rg \
    --profile-type PublicTrust
  # 5. Wire into the Tauri build pipeline:
  # Set AZURE_TENANT_ID / AZURE_CLIENT_ID / AZURE_CLIENT_SECRET
  # as GitHub Actions secrets; the release workflow signs with
  # azuresigntool sign -kva https://eus.codesigning.azure.net/ \
  #   -kvc mizan-prod -kvi $AZURE_CLIENT_ID -kvs $AZURE_CLIENT_SECRET \
  #   -kvt $AZURE_TENANT_ID -tr http://timestamp.digicert.com \
  #   -td sha256 mizan.exe
  ```

- **Risk if misconfigured**: Windows SmartScreen blocks the
  install with "Microsoft Defender SmartScreen prevented an
  unrecognized app from starting." User has to click "More info"
  → "Run anyway." Conversion rate drops ~70% at this screen.

### Linux — GPG signing key

- **What it's for**: Signs the `.deb` / `.rpm` / `.AppImage` so
  the user's package manager can verify the publisher.
- **Where to generate**: Locally with GPG v2.4+
  (no external authority).
- **Annual cost**: **$0**. (You can pay for a Sectigo or
  DigiCert code-signing cert if you want X.509 trust chain, but
  most Linux users live in GPG-trust-on-first-use land.)
- **KYC**: None — self-generated.
- **Time from generation to ready**: ~10 minutes.
- **Steps**:

  ```bash
  # 1. Generate a production GPG key (offline, on a clean machine)
  gpg --full-generate-key
  # Choose: (1) RSA and RSA, 3072 bits, key expires 5y
  # Real name: Mizan Releases
  # Email: releases@maevemodels.co.uk
  # Comment: Production package signing key
  # Passphrase: STRONG, store in a password manager

  # 2. Export the public key + push to a public keyserver
  gpg --armor --export releases@maevemodels.co.uk > mizan-releases.asc
  gpg --keyserver keys.openpgp.org --send-keys <KEY_ID>

  # 3. Export the secret key (backup ONLY — keep offline)
  gpg --armor --export-secret-keys releases@maevemodels.co.uk > \
    mizan-releases-secret.asc.BACKUP

  # 4. Wire into the build pipeline (CI signs the .deb / .rpm):
  # Export GPG_PRIVATE_KEY (armored secret) + GPG_PASSPHRASE as
  # GitHub Actions secrets. Release workflow runs:
  # echo "$GPG_PRIVATE_KEY" | gpg --import
  # dpkg-sig --sign builder -k <KEY_ID> mizan.deb
  ```

- **Risk if misconfigured**: `apt install ./mizan.deb` shows
  "untrusted signature" warning. Power users notice; most don't.
  Lower impact than macOS / Windows but still good hygiene.

---

## Part 3 — Pre-Canary Checklist

The 10-item checklist from `docs/runbooks/secrets-inventory.md`,
reformatted for action.

### 3.1 — Every per-provider encryption key set in Fly vault

- **What to check**: All 10 encryption keys named in
  `secrets-inventory.md` are vaulted in Fly.io production.
- **How to check**:

  ```bash
  flyctl secrets list --app mizan-connect-production | \
    grep -E "(PLAID|SNAPTRADE|SETU|SGFINDEX|TINK|BASIQ|LEAN|CCXT|MCP|OAUTH)_TOKEN_ENCRYPTION_KEY"
  ```

- **Green**: All 10 names listed (or, for the Group B providers
  you haven't activated yet, the corresponding key may be
  missing — that's OK).
- **Red**: Required key missing. **Stop. Generate it with
  `openssl rand -hex 32` and vault it.**

### 3.2 — No secret committed to git history

- **What to check**: gitleaks-on-full-history scan returns zero
  findings.
- **How to check**:

  ```bash
  # Install gitleaks if needed:
  brew install gitleaks
  cd /Users/samisayyed/Documents/mizan-ai-native
  gitleaks detect --no-banner --redact --report-path /tmp/leaks.json
  ```

- **Green**: `no leaks found`.
- **Red**: Any finding. **Treat as a P0 incident.** Rotate the
  leaked secret immediately, follow
  `docs/runbooks/incident-response.md`, file the leak event
  for the post-launch security retro.

### 3.3 — Production market-data / payment / AI keys on correct tier

- **What to check**: Each key in the Fly vault is the production
  tier, not dev / test.
- **How to check**:

  ```bash
  # Twelve Data: visit https://twelvedata.com/account and confirm
  # the active subscription is "Pro" or higher
  # MetalpriceAPI: visit https://metalpriceapi.com/dashboard
  # Anthropic: visit https://console.anthropic.com/settings/plans
  ```

- **Green**: All four (Twelve Data, MetalpriceAPI, Anthropic,
  Stripe) on production tier.
- **Red**: Any on a dev tier. Upgrade in the provider dashboard
  before continuing.

### 3.4 — Stripe key starts with `sk_live_`

- **What to check**: `STRIPE_SECRET_KEY` is production, not test.
- **How to check**:

  ```bash
  flyctl secrets list --app mizan-connect-production | \
    grep STRIPE_SECRET_KEY
  # The DIGEST column won't show the value, but you'll see if
  # the secret is set. To verify the prefix, redeploy with the
  # check enabled:
  flyctl ssh console --app mizan-connect-production \
    --command 'echo "${STRIPE_SECRET_KEY:0:8}"'
  ```

- **Green**: `sk_live_`.
- **Red**: `sk_test_`. **Stop. Replace immediately.**

### 3.5 — Multi-secret webhook rotation tested in staging in past 7 days

- **What to check**: The Stripe webhook signing secret rotation
  procedure (5-case test pattern per CLAUDE.md §5) was exercised
  against staging in the past week.
- **How to check**:

  ```bash
  # Look for a recent staging deploy with both
  # STRIPE_WEBHOOK_SECRET and STRIPE_WEBHOOK_SECRET_PREV set
  flyctl secrets list --app mizan-connect-staging | grep STRIPE_WEBHOOK
  # Verify the multi-secret test passed:
  cd mizan-connect && cargo test --lib --features test-utils \
    stripe_webhook_multi_secret_rotation
  ```

- **Green**: Two webhook secrets present in staging + test
  passes.
- **Red**: Single secret only. **Run the rotation drill per
  `docs/runbooks/key-rotation-quarterly.md` against staging
  before continuing.**

### 3.6 — Service-role key never exposed to client

- **What to check**: The Supabase service-role key isn't compiled
  into the desktop binary.
- **How to check**:

  ```bash
  cd /Users/samisayyed/Documents/mizan-ai-native
  grep -r 'SUPABASE_SERVICE_ROLE' mizan-4/ | \
    grep -v 'safety:' | grep -v '.gitignore' | grep -v 'docs/'
  ```

- **Green**: Zero hits (or only the comments tagged with
  `// safety:`).
- **Red**: Any code reference. **Stop. Move the call to
  mizan-connect (server-side only).**

### 3.7 — Anthropic org-level spending limit configured

- **What to check**: Anthropic dashboard has a monthly spending
  cap configured (recommended $500/mo).
- **How to check**: Visit
  https://console.anthropic.com/settings/billing and confirm
  the spending limit shows a non-empty value.
- **Green**: Limit set + < or = your monthly budget.
- **Red**: No limit. **Set one before any canary traffic.** Risk
  #2 in the master plan is "AI cost runaway" — caps are
  mandatory, not optional.

### 3.8 — Each provider encryption key matches `^[0-9a-f]{64}$`

- **What to check**: All encryption keys are 32 bytes hex (the
  format `openssl rand -hex 32` produces).
- **How to check**:

  ```bash
  # SSH into the production app and verify:
  flyctl ssh console --app mizan-connect-production --command '
    for key in PLAID SNAPTRADE MCP OAUTH; do
      val=$(printenv "${key}_TOKEN_ENCRYPTION_KEY")
      if [ ${#val} -eq 64 ]; then
        echo "${key}: OK"
      else
        echo "${key}: WRONG LENGTH (${#val})"
      fi
    done
  '
  ```

- **Green**: All "OK".
- **Red**: Any "WRONG LENGTH". **Regenerate with
  `openssl rand -hex 32 | tr -d '\n'` and re-vault.**

### 3.9 — Updater + Apple Developer + Azure Trusted Signing not expiring in 90 days

- **What to check**: Code-signing material isn't about to expire.
- **How to check**:

  ```bash
  # Apple Developer: check expiry in Xcode → Settings → Accounts
  # Azure Trusted Signing: az ts profile show --name mizan-prod-profile
  # GPG: gpg --list-keys releases@maevemodels.co.uk
  # Tauri updater signing key: check creation date in
  # ~/.tauri/signing-key.json (rotation cadence 3 years per
  # ADR 0009; flag if older than 2.5 years)
  ```

- **Green**: All certs/keys valid for > 90 days from today.
- **Red**: Any < 90 days. **Renew now.** Letting a cert expire
  mid-release means broken auto-updates for existing users.

### 3.10 — Tier-1 Sentry alerts wired + fake-breach tested

- **What to check**: Every Tier-1 alert in
  `docs/runbooks/sentry-alerts.md` has fired at least once in
  staging via a manual fake-breach + the PagerDuty test page was
  received.
- **How to check**: Open Sentry → Alerts → filter to "Tier 1"
  label. Each rule should show a "Last triggered" timestamp
  within the past 7 days.
- **Green**: All 4 Tier-1 rules show recent triggers + a
  matching PagerDuty incident.
- **Red**: Any rule has never fired. **Run the fake-breach
  drill before canary.**

---

## Part 4 — Gate 3 Canary Procedure

### 4.1 — Pre-deploy command sequence (the day of)

Run these in order. Stop if any step fails.

```bash
# 1. Verify the production secrets vault is complete
flyctl secrets list --app mizan-connect-production

# 2. Verify all CI checks are green on main
gh pr list --state merged --limit 5

# 3. Run the load-test plan against staging
# (per docs/runbooks/load-test.md). Verify the report shows
# p99 latencies inside §A19 budgets.

# 4. Run the §23 Playwright E2E against staging
cd mizan-4/apps/frontend
MIZAN_E2E_BASE_URL=https://staging.mizan.app pnpm exec playwright test

# 5. Verify zero Tier-1 Sentry alerts in the past 24h on staging
# (visit Sentry → Alerts dashboard)

# 6. Verify Truth Ledger chain integrity nightly job passed
# for the last 7 nights against staging
```

### 4.2 — Canary deploy (5% of production traffic)

```bash
cd /Users/samisayyed/Documents/mizan-ai-native/mizan-connect

# Tag the release
git tag -a v1.0.0-canary -m "Mizan 1.0 canary (5%)"
git push --tags

# Deploy to production with the canary strategy
MIZAN_ALLOW_PRODUCTION=1 flyctl deploy \
  --strategy canary \
  --canary-percent 5 \
  --app mizan-connect-production \
  --image-label v1.0.0-canary

# This routes 5% of production traffic to the new release.
# Existing users on the old release are unaffected.
```

### 4.3 — Dashboards to watch in the first 24 hours

| Dashboard | Threshold | Action if breached |
|---|---|---|
| Sentry → mizan-connect-prod errors | Error rate > 2× rolling 24h baseline | Auto-rollback fires; investigate |
| Sentry → mizan-desktop-prod performance | p99 cold-start > 2.4s for 15min | Manual rollback consideration |
| Fly.io → Metrics → Concurrency | > 80% sustained 10min | Scale out (autoscaler should handle) |
| Supabase → Reports → Slow Queries | Any query > 1s p99 | File `key-rotation-quarterly.md::supabase-lifecycle` |
| Stripe Dashboard → Disputes | Any chargeback | Investigate; not a rollback signal |
| Monitoring dashboard → AI cost / hour | > 2× rolling 7d hourly avg, 30min | Audit top spenders + model routing |

### 4.4 — 7-day → 25% → 7-day → 100% → 14-day cadence

```bash
# Day 7: scale to 25% (assuming clean Sentry for 7 days)
MIZAN_ALLOW_PRODUCTION=1 flyctl deploy \
  --strategy canary \
  --canary-percent 25 \
  --app mizan-connect-production

# Day 14: scale to 100% (assuming clean Sentry for another 7 days)
MIZAN_ALLOW_PRODUCTION=1 flyctl deploy \
  --app mizan-connect-production

# Day 14-28: monitor at 100% for 14 days
# Goal: zero Tier-1 alerts, p99 within §A19, sync success > 95%
# per provider, zero Truth Ledger violations, zero rollback events
```

### 4.5 — Rollback command (if something goes red)

```bash
# Immediate rollback to the previous release
MIZAN_ALLOW_PRODUCTION=1 flyctl releases --app mizan-connect-production
# Note the previous release's ID
MIZAN_ALLOW_PRODUCTION=1 flyctl deploy \
  --image registry.fly.io/mizan-connect-production:<previous-tag> \
  --app mizan-connect-production

# File the incident:
# 1. Copy a screenshot of the firing Sentry alert
# 2. Open docs/runbooks/incident-response.md
# 3. Create a new entry in docs/audit/<YYYY-MM-DD>-canary-rollback.md
```

### 4.6 — Definition of "live" — 14 days at 100% clean

Mizan is **live to real users** when ALL of these hold for 14
consecutive days at 100% production traffic:

- [ ] Zero Tier-1 Sentry alerts fired
- [ ] p99 latencies stayed within every §A19 budget
- [ ] Sync success rate > 95% per provider
- [ ] Zero Truth Ledger chain-integrity violations
- [ ] Zero webhook signature failures
- [ ] Zero rollback events
- [ ] §23 Playwright passing against staging mirror of prod
- [ ] AI cost per active user stayed within Anthropic spending cap
- [ ] At least one real user paid Zakat through the Pay Zakat
      flow + received the receipt

When all eight are green for 14 consecutive days → file the
launch announcement, close PR-READY, archive this checklist into
`docs/audit/2026-launch-checklist-completed.md` with the actual
dates each box was checked.

---

## Part 5 — Total Estimated Cost at Launch

### Required at launch (Group A)

| Service | Monthly cost | Notes |
|---|---|---|
| Stripe | ~$80 | 2.9% + 30¢ per txn at ~100 paid users × $20 |
| Anthropic Claude | $100-200 | $500/mo cap configured |
| Supabase Pro | $25-50 | $25 base + per-GB egress |
| Twelve Data Pro | $79 | Real-time market data |
| MetalpriceAPI Standard | $30 | Precious-metal spot for Nisab |
| Plaid Production | $100-150 | Pay-per-API-call, ~$0.30-0.50/user |
| SnapTrade Build | $199 | Up to ~250 active users |
| Fly.io | $50-100 | Production app + bandwidth |
| **Group A subtotal** | **~$663-888/mo** | Mid-point: **~$775/mo** |

### Activate after launch (Group B)

| Service | Monthly cost | When |
|---|---|---|
| SGFinDex | SGD 100-300 (~$75-225) | When you onboard Singapore users |
| Setu (India) | ₹10K-30K (~$120-360) | When you onboard Indian users |
| Tink (EU) | €500-1500 (~$540-1620) | When you open EU |
| Basiq (AU) | AUD 250-800 (~$170-540) | When you open Australia |
| Lean (UAE/KSA) | $200-500 | When you open MENA |
| **Group B subtotal** | **~$1100-3200/mo** | Region-dependent |

### Code-signing certificates

| Cost | Frequency |
|---|---|
| Apple Developer Program | $99/yr → ~$8/mo |
| Azure Trusted Signing | ~$15/mo for low volume |
| Linux GPG | $0 |
| **Code-signing subtotal** | **~$23/mo** |

### **Total at launch (Group A only + code-signing):**

**~$800/month**

### **Total at full regional expansion (A + B + code-signing):**

**~$2,400/month**

These are launch-scale numbers (~100 active users). At 1,000+
active users expect 3-5× scaling on Plaid + SnapTrade + Anthropic
(the per-user-priced services); Twelve Data + MetalpriceAPI +
Supabase + Stripe + Fly tier up only at much higher volumes.

---

## Part 6 — Total Estimated Time From Today to Live

Assuming you work **2-4 hours/day** on this, with KYC reviews
running in parallel:

### Week 1 — Submit applications (everything in parallel)

| Day | Activity | Time |
|---|---|---|
| Mon | Apply: Plaid Production, SnapTrade Production, Stripe, Anthropic, Supabase, Twelve Data, MetalpriceAPI, Apple Developer, Azure Trusted Signing | 4-6 hrs |
| Tue-Fri | Receive Stripe, Anthropic, Supabase, Twelve Data, MetalpriceAPI keys (same-week approvals). Vault them in Fly. | 1-2 hrs/day |
| Sat | Generate all 10 per-provider encryption keys via `openssl rand -hex 32`. Vault them. | 1 hr |
| End of week | Sandbox-vault complete: 5 of 7 Group A providers active | — |

### Week 2 — KYC clears + canary prep

| Day | Activity | Time |
|---|---|---|
| Mon-Wed | Apple Developer approves (1-3 days). Configure notarization. | 1-2 hrs |
| Mon-Fri | Azure Trusted Signing approves (3-7 days). Configure signing identity. | 1-2 hrs |
| Wed | SnapTrade compliance review clears (3-7 days). Vault SnapTrade prod keys. | 30 min |
| Thu-Fri | Wire signing into the Tauri build pipeline; cut a signed staging build; install on Mac + Windows test machines; verify no warnings. | 2-3 hrs |
| Weekend | Run staging load test (per `docs/runbooks/load-test.md`). | 2-3 hrs |
| End of week | Code-signing live + signed staging build verified | — |

### Week 3 — Plaid clears + pre-canary

| Day | Activity | Time |
|---|---|---|
| Mon-Wed | Plaid Production review clears (1-2 weeks from Week 1). Vault Plaid prod keys. | 1 hr |
| Thu | Wire all 10 Sentry Tier-1 alerts. Run fake-breach drill. Verify PagerDuty pages. | 2-3 hrs |
| Fri | Run §23 Playwright against staging. Fix any failing assertions. | 2-4 hrs |
| Weekend | Run pre-canary checklist (Part 3 above). All 10 items green. | 2-3 hrs |
| End of week | All 7 Group A providers active + monitoring live | — |

### Week 4 — Canary at 5%

| Day | Activity | Time |
|---|---|---|
| Mon | `flyctl deploy --strategy canary --canary-percent 5`. Tag release. Watch dashboards for 4 hours. | 4-5 hrs |
| Tue-Sun | Monitor 30 min/day. Respond to any Sentry alerts. | 30 min/day |

### Week 5 — Scale to 25%

| Day | Activity | Time |
|---|---|---|
| Mon | If Week 4 ended clean: `flyctl deploy --canary-percent 25`. Watch dashboards. | 2-3 hrs |
| Tue-Sun | Monitor 30 min/day. | 30 min/day |

### Week 6 — Scale to 100% + 14-day watch

| Day | Activity | Time |
|---|---|---|
| Mon | If Week 5 clean: `flyctl deploy` (no canary flag = 100%). | 2-3 hrs |
| Tue-end of Week 7 | Monitor daily. 14 days at 100% clean = launch. | 30 min/day |

### **Total calendar time from today to live: ~6 weeks**

The bottleneck is the Plaid Production review (1-2 weeks) +
the 14-day clean window at 100% (mandatory per Goal v3 launch
discipline). Everything else runs in parallel.

If you push 4+ hours/day and Plaid clears in the first review,
you can compress to ~5 weeks. If anything regresses during
canary and you need to rollback + fix, add 1-2 weeks per
incident.

---

## When you finish

When all 14 days at 100% are clean:

1. File the launch announcement on the blog + social.
2. Open `docs/audit/2026-launch-checklist-completed.md` and copy
   this file with every checkbox dated + signed.
3. Switch the production monitoring dashboard from "canary watch"
   to "steady state" view.
4. Schedule the first **quarterly secrets rotation** for the
   last week of the next quarter per
   `docs/runbooks/key-rotation-quarterly.md`.
5. Schedule the first **quarterly rollback drill** per
   `docs/runbooks/rollback-drill.md`.
6. Take a day off. You earned it.

---

## Cross-reference index

- `docs/PR-READY-readiness-declaration.md` — what's shipped
- `docs/runbooks/secrets-inventory.md` — secret-by-secret detail
- `docs/runbooks/sentry-alerts.md` — alert definitions
- `docs/runbooks/load-test.md` — load test plan
- `docs/runbooks/monitoring-dashboard.md` — dashboard spec
- `docs/runbooks/deploy.md` — deploy procedure
- `docs/runbooks/incident-response.md` — incident response
- `docs/runbooks/rollback-drill.md` — quarterly drill
- `docs/runbooks/key-rotation-quarterly.md` — quarterly rotation
- `docs/adr/0022-news-module-personalization.md` — SGFinDex redirect_uri
- `docs/adr/0025-oauth-connector-framework.md` — OAuth catalog
- `docs/adr/0026-mcp-capability-architecture.md` — MCP scope discipline

*Six weeks from today to a real user paying their Zakat through
Mizan. Walk it one day at a time.*
