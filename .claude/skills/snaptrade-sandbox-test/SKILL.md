---
name: snaptrade-sandbox-test
description: Use when testing SnapTrade brokerage sync against the SnapTrade sandbox. Covers user registration, portal URL creation, sandbox brokerage connect, position + cost-basis fetch, idempotency keys, and reconnect.
---

# SnapTrade sandbox end-to-end test

SnapTrade is **Gold-tier only** for brokerage sync. SnapTrade does not
have a separate sandbox host — sandbox keys hit `api.snaptrade.com`
directly. The difference is ~5 connection limit and some institutions
return mock data only. See v3 §SnapTrade End-to-End Contract.

## Credential request (run first)

```
SNAPTRADE_CLIENT_ID=
SNAPTRADE_CONSUMER_KEY=
SNAPTRADE_ENV=sandbox
SNAPTRADE_REDIRECT_URI=
SNAPTRADE_WEBHOOK_SECRET=
MIZAN_BROKER_SECRET_ENCRYPTION_KEY=   # base64, decodes to 32 bytes
MIZAN_SNAPTRADE_STATE_SECRET=         # base64, decodes to ≥ 32 bytes
```

If any are missing, **stop** and ask Sami by exact name. Never proceed
with stubs that pretend the sync worked.

## Smoke flow

1. **User register / load** — backend ensures a SnapTrade user exists:

   ```
   POST /v1/snaptrade/users (auth required)
   → { userId, userSecret }   # userSecret encrypted at rest via AES-256-GCM
   ```

2. **Portal URL** — backend creates a connect portal URL:

   ```
   POST /v1/snaptrade/login-portal
   body: { brokerage: "ROBINHOOD" }
   → { redirectUri: "https://snaptrade-portal/..." }
   ```

   Rate limit: 10/hour per local user (in-memory bucket today; Redis
   in Phase G).

3. **User connects** in the portal. SnapTrade redirects back to
   `SNAPTRADE_REDIRECT_URI` with a state token. Backend verifies the
   state-token JWT (HS256, 10-min TTL) and writes a `broker_connections`
   row.

4. **Sync accounts + positions**:

   ```
   POST /v1/snaptrade/sync (auth required)
   → { synced_accounts: N, synced_positions: M }
   ```

   Position records: symbol, quantity, cost basis (when available),
   currency, as_of timestamp. **Source = SnapTrade** on every row.

5. **Cost-basis absence** must be visible. If SnapTrade returns null
   for a position's cost basis, the holding is labelled "Cost basis
   missing — add manually" in the UI. Never invent.

6. **Idempotency** — replay the same sync; expect 0 new positions,
   N updates. Idempotency key:
   `brokerage_authorization_id + account_id + symbol + position_as_of`.

7. **Reconnect** — revoke the broker auth in SnapTrade's test mode and
   re-run sync. Expect a typed `SNAPTRADE_SYNC_FAILED` error with
   `safeMessage` "Broker authorization expired — reconnect required."

## What to assert

- `broker_connections` row exists per connected brokerage.
- `sync_runs(provider='snaptrade', status='succeeded')` written.
- Health Center: "SnapTrade: ok, last sync N min ago."
- Frontend per-broker tile shows last-synced timestamp.
- Removed brokerage accounts surface as "Account removed" — not
  silently deleted.
- Free + Silver users hitting the Connect Broker action see the
  upgrade gate ("Upgrade to Gold to sync brokers"), not a 500.

## Never

- Never store the SnapTrade `userSecret` unencrypted.
- Never expose `SNAPTRADE_CONSUMER_KEY` to the frontend.
- Never log `userSecret` in audit-event JSONB.
- Never silently delete a position that disappeared from sync —
  reconcile (Phase J).
