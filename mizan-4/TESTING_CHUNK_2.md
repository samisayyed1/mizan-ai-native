# Chunk 2 — Manual smoke test

This is a one-time verification doc. Delete it after the smoke test passes and
you're ready to tag Chunk 2 as shipped.

## Pre-flight

You'll need three terminals.

### Terminal 1 — Mizan Connect backend (already done if it's still running)

```bash
cd /Users/samisayyed/Documents/mizan-connect
make dev
# Keep running. Verify:
curl -s http://localhost:8080/health | jq
# expect: { "status": "ok", "version": "0.1.0", ... }
```

### Terminal 2 — paste your Supabase anon key

Open `/Users/samisayyed/Documents/Mizan-4/.env` and replace
`__I_WILL_PASTE_THIS_MANUALLY__` on the `CONNECT_AUTH_PUBLISHABLE_KEY` line with
the real anon key from **Supabase Dashboard → Project Settings → API → "anon
public" key**.

It is the publishable key — starts with `sb_publishable_…` or is a JWT with
`"role":"anon"`. **Never** paste the `service_role` secret.

### Terminal 3 — start Mizan desktop

```bash
cd /Users/samisayyed/Documents/Mizan-4
pnpm install
pnpm tauri dev
```

The first run takes a few minutes (Rust release-debug build). When the app
window opens, the Mizan dashboard should appear.

## Test 1 — offline mode (CONNECT empty)

1. Stop the desktop app (Ctrl-C in Terminal 3).
2. Comment out (or set to empty) the four `CONNECT_*` variables in `.env`:
   ```
   CONNECT_AUTH_URL=
   CONNECT_AUTH_PUBLISHABLE_KEY=
   CONNECT_API_URL=
   CONNECT_OAUTH_CALLBACK_URL=
   ```
3. Run `pnpm tauri dev` again. Wait for the window.
4. Open the **Connect** nav entry (sidebar, secondary list).

**Expected:**

- App boots cleanly, no console errors related to Connect.
- The Connect page shows the empty-state hero (logo + "Optional" badge + feature
  grid + "Get Started with Connect" / "Login to your account" CTAs). This is
  `ConnectEmptyState` — it doesn't try to call any API.
- Clicking "Login to your account" navigates to `/settings/connect` and shows
  the new "Mizan Connect is disabled in this build" placeholder.
- DevTools Network tab: zero requests to `localhost:8080`.
- DevTools Console: zero errors, no `CONNECT_AUTH_URL is NOT set` runtime
  warning (the build-time warning at compile is OK).

5. Restore your `.env` values and continue.

## Test 2 — sign up + sign in + /v1/me round-trip

1. Restart `pnpm tauri dev` so the new env vars are baked in.
2. Open **Connect** in the sidebar → click **Login to your account** (or
   navigate to `/settings/connect` directly).
3. The login form renders. Open DevTools (Cmd-Opt-I or Right-click → Inspect).
4. Open the **Network** tab and filter by `localhost:8080`.

### Sign up

5. Click "Sign up" (or the equivalent toggle on the form).
6. Enter a fresh email + password (e.g. `you+chunk2@gmail.com` / a strong
   password).
7. Submit.

**Expected:**

- Supabase responds with a verification-email-required state. Check your inbox;
  click the verification link or use the magic-link prompt.
- After verification, the form transitions to the signed-in `ConnectedView`.

### /api/v1/user/me round-trip

8. Watch the Network tab as the form transitions. You should see:
   - Several requests to `https://jtdtfnusgloizwclhobf.supabase.co` (auth).
   - **One request to `http://localhost:8080/api/v1/user/me`** with
     `Authorization: Bearer eyJ…` header.

**Expected response (Chunk 1 backend, after the alias is added per the follow-up
prompt):**

```http
HTTP/1.1 200 OK
content-type: application/json
x-request-id: <uuid>

{
  "id": "<uuid>",
  "supabase_user_id": "<uuid>",
  "email": "you+chunk2@gmail.com",
  "display_name": null,
  "avatar_url": null,
  "created_at": "2026-…",
  "updated_at": "2026-…"
}
```

**Capture the request and response** here:

```
[paste the request line + response status + first few headers + JSON body]
```

### Connected view

9. The page now renders:
   - User card (email, sign-out button).
   - "Plans & billing — Coming soon" placeholder.
   - "Brokerage sync — Coming soon" placeholder.
   - "Device sync — Coming soon" placeholder.

DevTools Console: no errors, no failed fetches except possibly an expected 404
on `/api/v1/subscription/plans` (this is fine — it'll be a clean 501 once the
backend stub lands per the follow-up prompt).

### Sign-in (already-registered user)

10. Click **Sign out**.
11. The page returns to the login form.
12. Sign back in with the same email + password.
13. Network: another `/api/v1/user/me` 200 response.
14. JSON `id` should match the previous response — the upsert is idempotent.

### Sign-out cleanup

15. Click **Sign out** again.
16. Verify in DevTools → Application → Local Storage that the
    `sb-jtdtfnusgloizwclhobf-auth-token-code-verifier` key is removed (or at
    least cleared).
17. macOS users: open Keychain Access, search for `mizan_sync_refresh_token` —
    the entry should be gone after sign-out.

## Pass / fail summary

- [ ] Test 1: offline mode produces no Connect HTTP traffic and no errors.
- [ ] Test 2 sign-up: verification email sent, sign-in completes.
- [ ] Test 2 /api/v1/user/me: 200 with correct DTO shape (record above).
- [ ] Test 2 connected view: three "Coming soon" placeholders rendered.
- [ ] Test 2 idempotent re-sign-in: same `id` returned.
- [ ] Test 2 sign-out: keyring + localStorage cleared.
- [ ] DevTools console: zero unexpected errors throughout.

If any line fails, capture the error and the network panel screenshot before
filing.
