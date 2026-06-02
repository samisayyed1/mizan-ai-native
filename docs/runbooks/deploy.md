# Runbook: Deploy Mizan Connect to Fly.io

The procedure for shipping a new Mizan Connect version to production.

## When to run

- A PR merges to `main` that touches `mizan-connect/`
- A hotfix needs to go out faster than the next scheduled deploy
- Stripe / Plaid / SnapTrade / Supabase config rotated and Connect needs to pick it up

## Prerequisites

- `flyctl` installed and authenticated (`fly auth whoami` returns your account)
- `MIZAN_ALLOW_PRODUCTION=1` exported in the current shell (the root
  `.claude/settings.json` hook blocks `fly deploy` otherwise)
- `.env.fly` present at the repo root with `chmod 600` permissions
- Sentry release token in env if shipping a tagged release
- All CI gates green on the merge commit (clippy, tests, audit, deny)

## Steps

1. **Confirm the target binary actually changed.**

   ```bash
   cd /Users/samisayyed/Documents/mizan-ai-native
   git log --oneline -1 mizan-connect/
   ```

   If the most recent commit doesn't touch `mizan-connect/`, skip — nothing to ship.

2. **Run the migration check (no-op against the prod-mirror DB).**

   ```bash
   cd mizan-connect
   cargo sqlx migrate info --database-url "$DATABASE_URL_PROD_MIRROR"
   ```

   Any pending migrations land in step 4. If unsure about a migration's safety, stop and ask.

3. **Build locally first to catch compile errors before the remote builder.**

   ```bash
   cargo check --release
   ```

   Fix any error before continuing — the remote builder won't be more lenient.

4. **Deploy.**

   ```bash
   fly deploy --remote-only --no-cache
   ```

   **`--no-cache` is non-negotiable** when the binary actually changes — Fly's
   remote builder caches aggressively and silently shipped a stale binary at v37.
   That cost half a day to diagnose.

5. **Verify the new release went live.**

   ```bash
   fly status --app mizan-connect
   curl -fsSL https://mizan-connect.fly.dev/v1/health
   ```

   Expect HTTP 200 and a version in the response body matching the commit you deployed.

6. **Smoke test the critical endpoints.**

   ```bash
   # /v1/me with a known test JWT (sandbox account)
   curl -sH "Authorization: Bearer $TEST_JWT" https://mizan-connect.fly.dev/v1/me

   # Stripe webhook endpoint reachable (returns 400 on missing signature, NOT 502/503)
   curl -sX POST https://mizan-connect.fly.dev/v1/billing/webhook
   ```

7. **Watch Sentry for 5 minutes.** Any new error pattern appearing → rollback per below.

## Verification

- `fly status` shows the new release as `v{N+1}` and `running`
- `/v1/health` returns the expected version string
- `/v1/me` returns the test account's data without 5xx
- Sentry error rate over the trailing 5 minutes ≤ pre-deploy baseline

## Rollback

If verification fails or Sentry spikes:

```bash
fly releases --app mizan-connect | head -3
fly releases rollback <previous-version> --app mizan-connect
```

DB migrations are forward-only — if a migration is the cause, write a
corrective forward migration rather than attempting to reverse.

## Escalation

If rollback fails or the prior release is also broken:

- Page on-call (current rotation in `docs/runbooks/incident-response.md`)
- Open a P0 incident channel
- Consider scaling Connect to zero (`fly scale count 0 --app mizan-connect`) until a fix lands — desktop falls back to offline-mode banner per the working agreement §7.7

## Related

- `docs/runbooks/incident-response.md` — incident workflow
- `docs/runbooks/key-rotation-quarterly.md` — encryption key rotation procedure
- `docs/working-agreement.md` §19.8 — canary deploy strategy (5%/25%/100% over 4 hours)
