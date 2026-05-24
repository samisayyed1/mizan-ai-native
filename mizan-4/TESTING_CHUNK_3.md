# Chunk 3 — Manual smoke test

End-to-end broker connection through real SnapTrade sandbox. One-time
verification doc — delete after the test passes.

## Pre-flight

You'll need three terminals.

### Terminal 1 — Mizan Connect backend (Chunk 3)

```bash
cd /Users/samisayyed/Documents/mizan-connect
make compose-up   # if Postgres is not already running on :5433

# Verify .env has all six SnapTrade vars set:
#   SNAPTRADE_CLIENT_ID=
#   SNAPTRADE_CONSUMER_KEY=
#   SNAPTRADE_API_BASE=https://api.snaptrade.com/api/v1
#   SNAPTRADE_REDIRECT_URI=http://localhost:8080/api/v1/sync/snaptrade/callback
#   MIZAN_BROKER_SECRET_ENCRYPTION_KEY=  (32 bytes base64)
#   MIZAN_SNAPTRADE_STATE_SECRET=        (≥ 32 bytes base64)

cargo run --release
# Expect:
#   "AES-256-GCM self-test passed"
#   "JWKS refreshed"
#   "listening" on 0.0.0.0:8080
```

### Terminal 2 — confirm the SnapTrade dashboard redirect URI

In <https://dashboard.snaptrade.com> add
`http://localhost:8080/api/v1/sync/snaptrade/callback` to the **Connection
Portal Redirect URIs** allowlist. Without this, the SnapTrade portal will
redirect to a non-allowed URL and the callback fails.

### Terminal 3 — desktop with the bypass flag

```bash
cd /Users/samisayyed/Documents/Mizan-4
# Make sure .env has the four CONNECT_* vars set (Chunk 2 work).
# CONNECT_AUTH_PUBLISHABLE_KEY must be your real Supabase anon key.

# CONNECT_BYPASS_PLAN_CHECK lets the broker-sync IPC run even though the
# Chunk-3 backend doesn't yet return team.plan in /api/v1/user/me.
# Drop this flag when Chunk 4 (Stripe) ships.
CONNECT_BYPASS_PLAN_CHECK=true pnpm tauri dev
```

First run takes a few minutes (Tauri release-debug build + the new
`mizan-connect` crate compile). When the window opens, you should see the Mizan
dashboard.

## Test 1 — empty state shows the live "Connect a broker" button

1. Open **Connect** in the sidebar (or click the Connect tab in
   `/settings/connect`).
2. If the user is signed in to Mizan Connect, the empty state should show a
   **gold gradient "Connect a broker" button** (the new one). The secondary
   "Sign in to Mizan Connect first" link only appears when the user is NOT
   signed in.

**Expected (signed in):**

- DevTools console: zero errors.
- Network tab (filter by `localhost:8080`): no traffic yet — the button hasn't
  been clicked.

## Test 2 — full broker connection round-trip

1. Click **Connect a broker**.
2. The button shows `Spinner — Opening portal...` for ~1 second.
3. Your default browser opens to a SnapTrade portal page (URL like
   `https://app.snaptrade.com/snapTrade/redeemToken?token=...`).
4. Watch the desktop's Network tab — there should be exactly one
   `POST /api/v1/sync/brokerage/login-portal` with `Authorization: Bearer eyJ…`
   returning **200** with body `{"url":"...", "expires_at":"..."}`.

5. In the browser, pick a sandbox-supported broker. **Robinhood** has a built-in
   paper-trading sandbox; SnapTrade also exposes a "TEST" broker that returns
   deterministic mock data.

6. Complete the broker login. SnapTrade redirects to
   `http://localhost:8080/api/v1/sync/snaptrade/callback?state=...&authorizationId=...`
   and the backend renders a minimal HTML success page that says "Broker linked
   — You can close this window and return to Mizan."

7. Close the browser window. Return to the Mizan desktop app.

8. Within 5–60 seconds, the broker connection should appear:
   - The **`Connect a broker`** button keeps showing
     `Spinner — Waiting for broker...` for the full 60 s polling window.
   - When the next 5-second poll fires, the empty state is replaced with the
     real `BrokerConnectionsCard` showing the broker logo and name, plus an
     Accounts card listing the broker's accounts.
   - In the Network tab you'll see ~12 calls to
     `GET /api/v1/sync/brokerage/connections` and the matching `/accounts` calls
     during the polling window.

**Capture in this doc:**

```
[ ] login-portal request line + 200 response body
[ ] timestamp of SnapTrade redirect → callback
[ ] which poll iteration first returned a non-empty connections array
[ ] broker name + account count visible in the desktop
```

## Test 3 — failure modes

### Rate limit (10/hr/user)

Click the "Connect a broker" button 11 times in quick succession. The 11th call
should produce a toast:
`Couldn't start broker connection: too many login-portal requests; try again later.`
(Status 429.)

### Backend down

Stop the backend (`Ctrl-C` in Terminal 1). Click the button. Toast:
`Couldn't start broker connection: error sending request…` or similar.

### Plan-gate fallback

Restart the desktop **without** `CONNECT_BYPASS_PLAN_CHECK`. Click **Connect a
broker** — the IPC under the hood (`broker_ingest_run` for sync, NOT
login-portal) will fail with `"Plan does not include broker sync"` once you try
to sync. Login-portal itself does NOT call `has_broker_sync()` so it will still
mint URLs — but `broker_ingest_run` won't run. This is the documented behavior
for Chunk 3 → Chunk 4 hand-off.

## Pass / fail summary

- [ ] Test 1: empty-state shows the new Connect-a-broker button; no console
      errors.
- [ ] Test 2: end-to-end portal → broker login → polling → connection visible in
      desktop.
- [ ] Test 2 network: exactly one `/login-portal` 200; subsequent `/connections`
      200s during polling.
- [ ] Test 3 rate-limit: 11th call returns 429 toast.
- [ ] Test 3 backend-down: clean error toast, no crash.

If any line fails, capture the error + a network screenshot before filing.

## Cleanup

After successful smoke:

- Stop both processes.
- Optional: revoke the test broker authorization with
  `DELETE /api/v1/sync/brokerage/connections/<id>` to free up your sandbox
  connection slot (SnapTrade free tier caps at ~5).
