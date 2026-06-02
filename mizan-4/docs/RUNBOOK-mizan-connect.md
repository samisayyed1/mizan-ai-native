# Mizan Connect — Ops runbook

Operational playbook for the cloud (`mizan-connect` Axum service on
Fly.io). Covers normal deploys, secret rotation, the admin
break-glass surface, and the rare-but-real incident shapes the
service has actually hit in production.

---

## Quick reference

| Thing | Where |
| --- | --- |
| Production URL | `https://mizan-connect.fly.dev` |
| Live status | `curl -sS https://mizan-connect.fly.dev/ready` |
| Fly app name | `mizan-connect` |
| Region | `sin` (Singapore) |
| Fly tokens (chmod 600, gitignored) | `~/Documents/mizan-ai-native/.env.fly` |
| Source repo path | `~/Documents/mizan-ai-native/mizan-connect/` |
| Postgres | Supabase (hosted) — DATABASE_URL secret |

---

## Normal deploy

```bash
cd ~/Documents/mizan-ai-native/mizan-connect
# 1. Local sanity
cargo check
cargo test --lib

# 2. Load Fly token + deploy
set -a && source ~/Documents/mizan-ai-native/.env.fly && set +a
FLY_ACCESS_TOKEN="$FLY_API_TOKEN_PRIMARY" fly deploy --remote-only

# 3. Verify health
curl -sS https://mizan-connect.fly.dev/health     # status: ok
curl -sS https://mizan-connect.fly.dev/ready      # all 4 components healthy
```

Fly typically takes 2–4 minutes for remote-only deploys. Version
increments by one per successful deploy; current is `v38` as of
the 2026-05-28 readiness pass.

**Rollback**: `FLY_ACCESS_TOKEN=… fly releases -a mizan-connect`
shows recent versions; `fly deploy --image
registry.fly.io/mizan-connect:deployment-<id>` redeploys a known-
good image.

---

## Secret rotation (Stripe webhook — zero-downtime)

The webhook signature secret can now be rotated without any
in-flight events failing. As of `5014c71` the verifier accepts a
comma-separated list and tries each in turn.

```bash
# Step 1 — deploy with both old + new secrets active
FLY_ACCESS_TOKEN=… fly secrets set \
  STRIPE_WEBHOOK_SECRET="whsec_OLDvalue,whsec_NEWvalue" \
  -a mizan-connect
# (fly auto-redeploys; wait for /ready 200)

# Step 2 — switch the Stripe Dashboard endpoint's signing secret
#         to whsec_NEWvalue
#   Stripe Dashboard → Developers → Webhooks → <endpoint> →
#   Reveal signing secret → … → Rotate or set NEW.

# Step 3 — once Stripe Dashboard shows NEW as the active secret AND
#         the last delivery has gone through cleanly, deploy with
#         just the new secret:
FLY_ACCESS_TOKEN=… fly secrets set \
  STRIPE_WEBHOOK_SECRET="whsec_NEWvalue" \
  -a mizan-connect
```

The Stripe-side propagation usually takes <30s; the cutover window
where both secrets need to be live can be as short as a single
minute.

### Diagnosing high webhook 401 rate

If `fly logs -a mizan-connect | grep "stripe/webhook" | grep "401"`
shows >10% failures over an hour:

1. **Check for duplicate endpoints.** In Stripe Dashboard →
   Developers → Webhooks, more than one endpoint pointing at
   `https://mizan-connect.fly.dev/v1/stripe/webhook` will each
   deliver with its own signature. Mizan absorbs this if both
   signing secrets are in `STRIPE_WEBHOOK_SECRET`; otherwise the
   wrong-secret deliveries 401.
2. **Check for stale `stripe listen` CLI.** A long-lived
   `stripe listen --forward-to https://mizan-connect.fly.dev/…`
   from a developer's laptop has its own signing secret. Either
   stop that session OR add its secret to the rotation list.
3. **Check Fly secret freshness.** `fly secrets list -a mizan-
   connect` shows hashes; if the hash on STRIPE_WEBHOOK_SECRET
   changed recently but Stripe Dashboard hasn't been updated,
   you're in the rotation window. See above.

---

## Admin / break-glass endpoint

Disabled by default. To enable for an ops session:

```bash
# Generate a strong one-time token
ADMIN_TOK=$(openssl rand -hex 32)
echo "$ADMIN_TOK"  # save somewhere safe — never commit

# Activate
FLY_ACCESS_TOKEN=… fly secrets set \
  MIZAN_ADMIN_TOKEN="$ADMIN_TOK" \
  -a mizan-connect

# Read a user's state
curl -sS -H "Authorization: Bearer $ADMIN_TOK" \
  https://mizan-connect.fly.dev/v1/admin/user/<user-uuid>

# Force-grant a tier (DELETE+INSERT — overwrites any existing
# subscription rows for that team)
curl -sS -X POST -H "Authorization: Bearer $ADMIN_TOK" \
  -H "Content-Type: application/json" \
  -d '{"tier":"gold","status":"active"}' \
  https://mizan-connect.fly.dev/v1/admin/user/<user-uuid>/subscription

# Disable when done
FLY_ACCESS_TOKEN=… fly secrets unset MIZAN_ADMIN_TOKEN -a mizan-connect
```

**Every admin grant is audit-logged** to Fly stdout (and Sentry
when configured) via `tracing::info!` with structured fields:
`{message:"admin: force-granted subscription", user_id, tier,
status}`. Greppable via `fly logs -a mizan-connect | grep "admin:
force-granted"`.

**Pre-condition the admin endpoints expect**: the user must
already exist in the `users` table (created by their first
Supabase JWT sync). If they've never signed in, the GET returns
404 and the POST creates a stranded subscription row with no
referent user. Don't run admin grants for ghost users.

---

## Stripe Customer Portal "no billing record"

Users granted via the admin endpoint don't have a
`stripe_customer_id` — they bypass the normal checkout flow. The
desktop's "Manage subscription" button surfaces this as a toast:
*"No Stripe billing record yet … run a fresh checkout."*

**To formalize an admin-granted user into Stripe**: have them
click any plan in the SubscriptionPlans grid, complete the test
checkout, then their admin-granted tier carries forward into the
new Stripe customer. The checkout handler uses ON CONFLICT
upserts so existing tier doesn't get clobbered to "silver" on the
way in.

---

## Postgres operations

The cloud connects to Supabase. The container is distroless, so
`fly ssh console` gives a shell with nothing in it — no `cat`,
no `psql`, no `printenv`.

**To run ad-hoc SQL**: use the Supabase Dashboard SQL editor
(easier) OR install psql locally + connect with the
`DATABASE_URL` value from Fly secrets (paste once into a
gitignored file, never commit).

The admin endpoint's read mode covers most diagnostic needs
without needing direct DB access — prefer it.

---

## Health endpoints

| URL | Use |
| --- | --- |
| `/health` | Liveness — service is running. Returns version + build_time + commit_sha. |
| `/ready` | Readiness — components healthy. Returns `db`, `jwks`, `plaid`, `billing` status. Always JSON. |
| `/v1/me` | Authenticated — returns the signed-in user's entitlements + subscription. Use during incident response to confirm a specific user is seeing the right tier. |

Wire these into a monitoring service (Pingdom, Better Uptime,
Uptime Kuma) with **5-minute interval, 2-minute timeout,
alert on first failure**.

---

## Incident: subscription writes failing on `team_id NOT NULL`

**Symptom**: Stripe webhook log shows `null value in column
"team_id" of relation "subscriptions" violates not-null
constraint`. User completes checkout but subscription doesn't
appear in Mizan.

**Cause**: Pre-`429a0de`/`a3ef626` bug — the checkout-session
and webhook upsert paths didn't write `team_id`. Migration
0005's NOT NULL constraint trips the INSERT.

**Fix**: Already shipped in v33+v34. If this re-appears, it
means a regression — the upsert SQL needs `team_id = $1`
alongside `user_id = $1` (the migration 0005 invariant is
`team_id == user_id`). Cross-reference
`crates/connect/src/billing/repository.rs` and
`crates/connect/src/billing/handlers.rs`.

**Recovery for an already-stuck user**: use the admin endpoint
to grant their intended tier directly. Their Stripe payment
will still be on their account; the next time they hit checkout
the row will reconcile through.

---

## Incident: cloud crash-loops on boot

**Symptom**: `fly status -a mizan-connect` shows machines in
restart loop. `fly logs` shows `missing required env var: …`.

**Cause**: a required env var got unset OR the binary added a
new required field.

**Fix path**:
1. Identify the missing var from the error message.
2. Set a placeholder value: `fly secrets set FOO="placeholder"
   -a mizan-connect`. Cloud should boot.
3. Decide whether the var should be truly optional or genuinely
   required. The `PLAID_REDIRECT_URI` case (commit `2635cfe`)
   showed the difference — Plaid only needs it for OAuth-required
   institutions, so making it `Option<String>` was correct.

---

## Useful one-liners

```bash
# Tail live logs filtered to errors
fly logs -a mizan-connect | grep -iE "ERROR|panic"

# Last 50 webhook deliveries with status
fly logs -a mizan-connect --no-tail | \
  grep "stripe/webhook" | \
  grep -oE 'status":[0-9]+' | sort | uniq -c

# Check secret freshness (shows hashes, not values)
fly secrets list -a mizan-connect

# Restart all machines (force the cluster to pick up env changes
# without a code redeploy)
fly machine restart -a mizan-connect

# Probe a specific user's subscription state (needs admin token)
curl -sS -H "Authorization: Bearer $ADMIN_TOK" \
  https://mizan-connect.fly.dev/v1/admin/user/<uuid> | jq
```
