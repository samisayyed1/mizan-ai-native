# Local Dev Setup — Mizan

This is a step-by-step guide to running the full Mizan stack on your own
machine: the Tauri desktop app, the Axum cloud backend (Mizan Connect),
and the Postgres database they share. By the end you'll be able to:

- Sign in via Supabase
- Connect a sandbox Plaid bank
- Run a Stripe test checkout
- Compute Zakat against live precious-metal spot prices
- Talk to the AI assistant

If anything in this guide doesn't work, the fix belongs in this file —
please update it as you go.

---

## Prerequisites

You need three toolchains. Don't worry about versions yet — the lockfiles
pin them and the install commands below pick up the right ones.

- **Rust** — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js 20+** — install via [nvm](https://github.com/nvm-sh/nvm) or
  `brew install node@20`
- **pnpm** — `npm install -g pnpm`
- **Docker** — for the local Postgres. [Get Docker Desktop](https://www.docker.com/products/docker-desktop/).
- **Stripe CLI** (only if you want to test billing webhooks) —
  `brew install stripe/stripe-cli/stripe`

Verify each:

```bash
rustc --version       # 1.95 or newer
node --version        # 20.x or newer
pnpm --version        # 9.x or newer
docker --version      # any recent version
stripe --version      # optional
```

---

## Step 1 — Clone the repo

```bash
git clone https://github.com/samisayyed1/mizan-ai-native.git
cd mizan-ai-native
```

The repo is a monorepo with three independent pieces:

- `mizan-4/` — Tauri desktop app + Rust crates
- `mizan-connect/` — Axum cloud backend
- `mizan-landing/` — Next.js marketing site (not needed for app dev)

---

## Step 2 — Copy the env templates

```bash
cp mizan-4/.env.example mizan-4/.env
cp mizan-connect/.env.example mizan-connect/.env
```

You'll fill these in over the next few steps. Both files are gitignored.

---

## Step 3 — Generate the encryption keys

Mizan Connect encrypts third-party tokens (Plaid access tokens, SnapTrade
secrets, etc.) at rest using AES-256-GCM. Each integration uses its own
key so a single key compromise doesn't burn the whole vault.

Generate one 32-byte hex key per slot. Run this once and paste each
output into the matching line of `mizan-connect/.env`:

```bash
for slot in PLAID SNAPTRADE BROKER; do
  echo "MIZAN_${slot}_TOKEN_ENCRYPTION_KEY=$(openssl rand -hex 32)"
done

# Forward-looking — for handlers not yet shipped. Generate now so the
# key never appears in commit history later.
for slot in SETU SGFINDEX TINK BASIQ LEAN CCXT_API_KEY MCP OAUTH; do
  echo "${slot}_TOKEN_ENCRYPTION_KEY=$(openssl rand -hex 32)"
done

# Admin token — gates /v1/admin/* endpoints.
echo "MIZAN_ADMIN_TOKEN=$(openssl rand -hex 32)"

# SnapTrade state-token signing secret.
echo "MIZAN_SNAPTRADE_STATE_SECRET=$(openssl rand -hex 32)"
```

**These are local-dev keys.** Never reuse them in production — Fly
deploys load production keys from `fly secrets set`, not from the
repo's `.env.example`.

---

## Step 4 — Start Postgres

```bash
cd mizan-connect
docker compose up -d postgres
```

The container listens on `localhost:5433` (note: not the default 5432 —
that port is reserved so a global Postgres on your machine doesn't
conflict). The default `DATABASE_URL` in `.env.example` already points at
this URL.

Run the migrations:

```bash
cd mizan-connect
cargo install sqlx-cli --no-default-features --features postgres
sqlx migrate run
```

If that succeeds you'll have 14 tables: `users`, `subscriptions`,
`broker_connections`, `audit_log`, `stripe_events`, `teams`,
`team_members`, `team_invites`, `plaid_items`, `plaid_accounts`,
`plaid_transactions`, `investment_holdings`, `investment_transactions`,
`advisor_links`, plus a handful of supporting tables.

---

## Step 5 — Sign up for third-party API keys

Skip any service you don't plan to test. Each is independent — the
desktop app degrades gracefully when a service is unconfigured.

### Supabase (required for sign-in)
1. Create a free project at https://supabase.com/dashboard
2. Settings → API → copy:
   - `URL` → `SUPABASE_URL` (in `mizan-connect/.env`) and
     `CONNECT_AUTH_URL` (in `mizan-4/.env`)
   - `anon public` key → `CONNECT_AUTH_PUBLISHABLE_KEY`
   - `service_role` key (⚠ secret) → `SUPABASE_SERVICE_ROLE_KEY` in
     `mizan-connect/.env` ONLY. Never put this in the desktop env.
3. Authentication → URL Configuration → add `http://localhost:1420` to
   the allowed redirect URLs.

### Stripe (test mode) — billing
1. Create an account at https://dashboard.stripe.com (no card needed for
   test mode).
2. Developers → API keys → copy:
   - Publishable test key (`pk_test_…`) — not needed in the env files
     (the desktop fetches it from `/v1/config/public`)
   - Secret test key (`sk_test_…`) → `STRIPE_SECRET_KEY`
3. Create Products + Prices for Silver Monthly, Silver Yearly, Gold
   Monthly, Gold Yearly. Copy each `price_…` ID into the matching
   `STRIPE_PRICE_*` slot.
4. For webhook testing, install Stripe CLI (see prereqs) and run:
   ```bash
   stripe listen --forward-to localhost:8080/api/v1/stripe/webhook
   ```
   It prints a `whsec_…` value — copy that into `STRIPE_WEBHOOK_SECRET`.

### Plaid (sandbox) — bank sync
1. Sign up at https://dashboard.plaid.com (sandbox is free).
2. Team Settings → Keys → copy `client_id` and `Sandbox` secret into
   `PLAID_CLIENT_ID` and `PLAID_SECRET`. Leave `PLAID_ENV=sandbox`.
3. To test the Link flow, use Plaid's documented test credentials:
   - Username: `user_good`
   - Password: `pass_good`

### Anthropic (optional — for the BYO-key AI path)
1. Get a key at https://console.anthropic.com/settings/keys
2. Paste into `ANTHROPIC_API_KEY` in `mizan-4/.env` (desktop only).
   The Connect cloud doesn't need this — it routes through its own
   managed-AI billing path.

### Twelve Data (optional — live equities feed)
1. Free signup at https://twelvedata.com
2. Paste API key into `TWELVE_DATA_API_KEY` in `mizan-connect/.env`.

### MetalpriceAPI (optional — Zakat Nisab spot prices)
1. Free tier at https://metalpriceapi.com (50 req/month).
2. Paste into `METALPRICEAPI_KEY` in `mizan-connect/.env`.

### NewsAPI (optional — news feed)
1. Free developer tier at https://newsapi.org/register
2. Paste into `MIZAN_NEWSAPI_KEY` in `mizan-connect/.env`.

### Resend (optional — outbound email for team invites)
1. Free at https://resend.com.
2. Paste API key into `RESEND_API_KEY` and a verified sender into
   `RESEND_FROM`.

### Sentry (optional — crash reporting)
1. Free at https://sentry.io.
2. Create one project for the Rust core, one for the Tauri frontend.
3. Paste the DSNs into `SENTRY_DSN`, `VITE_SENTRY_DSN` (desktop) and
   `SENTRY_DSN` (Connect).

---

## Step 6 — Run Mizan Connect

```bash
cd mizan-connect
cargo run
```

First boot is slow (~2 min) while it compiles. After that, `cargo run`
launches in under 5 seconds.

The server logs `listening on 0.0.0.0:8080`. Verify:

```bash
curl http://localhost:8080/health
# → {"status":"ok","version":"…","build_time":"…","commit_sha":"…"}
curl http://localhost:8080/ready
# → {"status":"ready","db":"ok","jwks":"ok",…}
```

If `/ready` reports `db: failing`, the Postgres container isn't up.
Re-run `docker compose up -d postgres`.

---

## Step 7 — Run the desktop app

In a new terminal:

```bash
cd mizan-4
pnpm install
pnpm tauri dev
```

First boot is ~3 minutes (Rust crate compilation). After that, `pnpm
tauri dev` rebuilds the Rust core in <30s and the frontend hot-reloads
instantly.

A window opens. Click **Sign in with Mizan Connect** → it should open
your Supabase login. After signing in you're back in the app at the
dashboard.

---

## Step 8 — Verify each integration

Walk through each provider you wired:

- **Sign in** → completes without a 500 from Connect. Check
  `audit_log` in Postgres:
  ```bash
  psql postgres://mizan:mizan@localhost:5433/mizan_connect \
    -c "SELECT event_type, created_at FROM audit_log ORDER BY id DESC LIMIT 5;"
  ```
  You should see rows.

- **Connect a Plaid bank** → Connect tab → Add bank → Plaid Link
  opens → pick any sandbox institution → use `user_good`/`pass_good`
  → accounts appear in the Bank & Cash panel. Verify
  `plaid_accounts` has rows.

- **Pay Zakat (test)** → Zakat tab → compute → Pay Zakat → Stripe
  Checkout opens in test mode → use card `4242 4242 4242 4242`, any
  future expiry, any CVC. After paying the webhook fires and the
  donation gets recorded (check `audit_log` for
  `billing.invoice_paid`).

- **AI assistant** → Click the assistant icon in the sidebar → type
  "What's my net worth?" → response should stream back. If you set
  `ANTHROPIC_API_KEY`, it uses your key; otherwise it routes through
  Mizan Connect's managed-AI path (which needs the cloud's own
  `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` set).

- **News feed** → sidebar → news widget shows real headlines if
  `MIZAN_NEWSAPI_KEY` is set, or a placeholder message if not.

- **Live prices** → dashboard heatmap → if `TWELVE_DATA_API_KEY` is
  set, tiles update during market hours.

- **Zakat Nisab** → Zakat tab → "Nisab today is $N" should match
  ~85g gold or ~595g silver at current spot (depends on the user's
  configured madhab).

---

## Common pitfalls

**`pnpm tauri dev` fails with "cannot find rustc"**: run
`source ~/.cargo/env` and try again.

**Connect returns 500 on every request**: most often this is a missing
encryption key. Check `cargo run` output for "MIZAN_PLAID_TOKEN_ENCRYPTION_KEY
required" — the message is verbose for a reason.

**Postgres connection refused**: the container exposes port 5433, not
the default 5432. Make sure `DATABASE_URL` in `mizan-connect/.env`
matches.

**Stripe webhook 400 with "invalid signature"**: the `whsec_…` from
`stripe listen` differs from the one in the Stripe Dashboard — use the
CLI's whenever you're running locally.

**Plaid Link opens but says "no items found"**: this is the sandbox
default for empty connections. Pick any institution from the list and
proceed with `user_good`/`pass_good`.

**Tauri window won't open on macOS**: System Settings → Privacy &
Security → grant the dev binary permissions.

---

## What's NOT covered here

- Production deployment (see `PRODUCTION_HANDOFF.md`)
- Code signing (Apple Developer ID, Windows Authenticode) — separate
  process owned by the release engineer
- Test fixtures for §23 (Singapore reference user) — see
  `mizan-4/e2e/`

If you find yourself reaching for those, you're past local dev and into
the release path.
