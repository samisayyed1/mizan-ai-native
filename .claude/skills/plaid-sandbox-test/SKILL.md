---
name: plaid-sandbox-test
description: Use when testing Plaid Link, public-token exchange, item creation, or webhook handling against the Plaid sandbox. Includes the canonical sandbox credential request, curl snippets for link-token + exchange, and the expected sandbox webhook shapes.
---

# Plaid sandbox end-to-end test

Plaid is **Gold-tier only** and used for live bank sync where supported
(US + CA today). Never propose Plaid for India/UAE/non-supported regions
— propose manual entry. See v3 §Plaid End-to-End Contract.

## Credential request (run first)

Required sandbox vars (must live in `mizan-connect/.env.local`, never
committed):

```
PLAID_CLIENT_ID=
PLAID_SECRET=
PLAID_ENV=sandbox
PLAID_PRODUCTS=auth,transactions,investments,liabilities
PLAID_COUNTRY_CODES=US,CA
PLAID_REDIRECT_URI=
PLAID_WEBHOOK_URL=
```

If any are missing, **stop** and ask Sami:

> Missing for Plaid sandbox: PLAID_CLIENT_ID, PLAID_SECRET,
> PLAID_REDIRECT_URI, PLAID_WEBHOOK_URL. Please paste the sandbox values
> or confirm I should update `.env.example` only and stub the integration.

## Smoke flow (≤ 5 min)

1. **Link token** — backend creates one for the test user:

   ```bash
   curl -X POST http://localhost:8080/v1/plaid/link-token \
     -H "Authorization: Bearer $JWT" \
     -H "Content-Type: application/json"
   ```

   Expect: `{ "link_token": "link-sandbox-..." }`.

2. **Link** — desktop opens Plaid Link with that token. In sandbox use
   institution **First Platypus Bank** with credentials `user_good` /
   `pass_good`. Plaid Link returns a `public_token`.

3. **Exchange** — desktop posts the public token; backend exchanges:

   ```bash
   curl -X POST http://localhost:8080/v1/plaid/exchange \
     -H "Authorization: Bearer $JWT" \
     -H "Content-Type: application/json" \
     -d '{"public_token":"public-sandbox-..."}'
   ```

   Expect: `{ "item_id": "...", "accounts": [...] }`. The access token
   stays server-side — never leaves Mizan Connect.

4. **Webhook** — Plaid sandbox fires `DEFAULT_UPDATE`, `INITIAL_UPDATE`,
   `HISTORICAL_UPDATE`. Verify the signature, write a `sync_runs` row
   per webhook delivery, surface failures in the Health Center.

5. **Reconnect flow** — trigger `ITEM_LOGIN_REQUIRED` via:
   ```bash
   curl -X POST https://sandbox.plaid.com/sandbox/item/reset_login \
     -H "Content-Type: application/json" \
     -d '{"client_id":"$PLAID_CLIENT_ID","secret":"$PLAID_SECRET","access_token":"..."}'
   ```
   Desktop must show "Reconnect required" with a button that mints a
   fresh link token in update mode.

## What to assert end-to-end

- Link token creation returns 200 within 2 s.
- Public-token exchange writes one row to `plaid_items` and N rows to
  `plaid_accounts`.
- `sync_runs(provider='plaid', status='succeeded')` exists.
- Health Center shows "Plaid: ok, last sync N min ago."
- Disconnect/reconnect cycles cleanly — no orphan rows, no duplicate
  accounts.
- A failed sync surfaces with a typed error code (`PLAID_RECONNECT_REQUIRED`
  etc.) and a `safeMessage` in the desktop UI.

## Never

- Never log access tokens.
- Never expose them to the frontend.
- Never commit `.env.local`.
- Never push Plaid for unsupported countries.
- Never silently mark a sync `succeeded` if Plaid returned an error.

## When done

Mark Phase G's Plaid sandbox validation step complete only if the full
list above passes end-to-end.
