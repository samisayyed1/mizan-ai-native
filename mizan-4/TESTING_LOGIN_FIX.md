# Mizan Connect login fix — manual smoke test

One-page runbook. Delete this doc once you've verified the new password sign-in
flow works against your real Supabase project.

## What changed

`MizanConnectProvider` already exposed `signInWithEmail(email, password)` and
`signUpWithEmail(email, password)` (both call
`supabase.auth.signInWithPassword(...)` and `supabase.auth.signUp(...)`). The
`LoginForm` was just hard-wired to the magic-link / OTP flow and never reached
for them. This change rewires the form to use password as the primary flow,
demotes magic-link to a "Use magic link instead" link, adds a Sign In / Sign Up
toggle, and translates Supabase's error messages to user-facing copy.

A small `resendConfirmation(email)` method was added to the provider for the
"Confirm your email first" path; nothing else in the auth infrastructure
changed.

## Pre-flight

1. **Wait out the 429.** If you've been hitting `/auth/v1/otp` repeatedly the
   magic-link rate limit lasts ~1 hour per IP. The new password flow hits a
   different endpoint (`/auth/v1/token?grant_type=password`), so you can test
   that immediately — but the "Use magic link instead" button will still be
   rate-limited until the window resets.
2. **Pre-create a test user in the Supabase dashboard.**
   - <https://supabase.com/dashboard/project/jtdtfnusgloizwclhobf/auth/users>
   - **Add user → Create new user**
   - Email: a real address you can read mail at
   - Password: anything ≥ 8 chars (e.g. `MizanTest2026!`)
   - ✅ **Auto Confirm User** — must be checked, otherwise sign-in fails with
     `Email not confirmed` and you'll need the resend flow to verify before
     signing in.
3. Confirm `apps/frontend/.env` has the correct Supabase keys
   (`CONNECT_AUTH_URL`, `CONNECT_AUTH_PUBLISHABLE_KEY`). Use the
   `sb_publishable_…` key — never the `sb_secret_…` service role key.

## Run

```bash
cd /Users/samisayyed/Documents/Mizan-4
pnpm tauri dev
```

When the window opens, navigate to **Settings → Mizan Connect** (or click
**Connect** in the sidebar and then **Sign in**).

## Tests

### Test 1 — happy-path password sign in

1. The form renders with **Sign in / Sign up** tabs at the top, **Sign in**
   selected by default.
2. Email + password fields below, with a `Sign in` button beneath.
3. A small **"Use magic link instead"** link sits under the button.
4. Enter the test user's email + password. Click **Sign in**.
5. Open DevTools → Network → filter for `auth/v1`.

**Expected on success:**

- One `POST /auth/v1/token?grant_type=password` returning **200** with a JSON
  body containing `access_token` + `refresh_token`. **NOT** `/auth/v1/otp`.
- Right after, one `GET /api/v1/user/me` to `localhost:8080` returning **200**
  with the user DTO.
- The form unmounts and `ConnectedView` renders with the user's email, the Mizan
  Connect "Connect a broker" button (Chunk 3 work), and the Plans/Device-sync
  placeholders.

### Test 2 — wrong password

1. Sign out, return to the form.
2. Enter the same email but a wrong password. Click **Sign in**.

**Expected:**

- Red alert reading **"Wrong email or password."** (translated from Supabase's
  `invalid login credentials`).
- No 401 toast spam, no console errors.

### Test 3 — sign up flow

1. Click **Sign up** tab.
2. Enter a fresh email + a password ≥ 8 chars. Click **Create account**.

**Expected (auto-confirm enabled in Supabase):**

- One `POST /auth/v1/signup` returning **200**.
- Green alert: "Account created. If your project requires email confirmation,
  check your inbox."
- If your Supabase project has email confirmation **disabled**, the provider
  also gets a session and `ConnectedView` renders.

**Expected (auto-confirm disabled):**

- The signup succeeds but no session is created. The form stays put, showing the
  green "check your inbox" message.
- If you then try to sign in, you'll see **"Confirm your email first."** with an
  inline **Resend confirmation email** button. Clicking it sends a fresh
  confirmation mail and shows "Confirmation email re-sent to …".

### Test 4 — too many attempts

1. Enter wrong password 5+ times in quick succession.

**Expected:**

- Eventually Supabase returns 429. Form alerts: **"Too many attempts. Try again
  in a minute."**

### Test 5 — magic link still works

1. From the password form, click **"Use magic link instead"**.
2. The form swaps to a single email field with a **Send magic link** button.
3. Enter your test email and click. Form transitions to the 6-digit OTP input —
   the existing magic-link UX is intact.
4. Click **← Back to password** to return to the password form.

## Pass / fail checklist

- [ ] Test 1: `grant_type=password` 200 + `/v1/user/me` 200 → ConnectedView.
- [ ] Test 2: wrong-password shows "Wrong email or password."
- [ ] Test 3: sign up creates the user; appropriate confirm-email message on
      auto-confirm-off projects.
- [ ] Test 3 cont.: "Resend confirmation email" works on the confirm-required
      path.
- [ ] Test 4: 429 shows "Too many attempts."
- [ ] Test 5: magic-link path is reachable and unchanged.

If any line fails, capture the network panel screenshot + the console output
before filing.
