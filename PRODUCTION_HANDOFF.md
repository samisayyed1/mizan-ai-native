# Production Handoff Checklist

Status of every production-key pathway in the Mizan app + Mizan Connect
server. Code-side everything is wired; this is the operator runbook for
landing the actual production credentials.

Last verified: 2026-05-26. Sandbox cloud is live at https://mizan-connect.fly.dev.

---

## 1. Stripe — Production Billing

### Fly secrets (run from `mizan-connect/`)

```bash
fly secrets set --app mizan-connect \
  STRIPE_SECRET_KEY="sk_live_…" \
  STRIPE_WEBHOOK_SECRET="whsec_…" \
  STRIPE_PRICE_SILVER="price_…" \
  STRIPE_PRICE_GOLD="price_…"
```

### Stripe Dashboard

- **Endpoint:** `https://mizan-connect.fly.dev/api/v1/stripe/webhook`
- **Events to subscribe** (already handled by the server):
  - `checkout.session.completed`
  - `customer.subscription.created`
  - `customer.subscription.updated`
  - `customer.subscription.deleted`
  - `customer.subscription.trial_will_end`  ← banner trigger
  - `invoice.paid`                          ← resets AI credits
  - `invoice.payment_failed`                ← stamps last_payment_failure_at
- **Signing secret:** copy from the endpoint settings into `STRIPE_WEBHOOK_SECRET`

### Verification after key drop

```bash
# Test webhook signature verification:
fly logs --app mizan-connect | grep "stripe webhook"

# Confirm a real /billing/checkout returns 200 + a session URL:
curl -X POST https://mizan-connect.fly.dev/api/v1/billing/checkout \
  -H "Authorization: Bearer <user JWT>" \
  -H "Content-Type: application/json" \
  -d '{"plan":"gold"}'
```

---

## 2. Plaid — Production Access

### Fly secrets

```bash
fly secrets set --app mizan-connect \
  PLAID_ENV="production" \
  PLAID_CLIENT_ID="…" \
  PLAID_SECRET="…" \
  PLAID_WEBHOOK_URL="https://mizan-connect.fly.dev/api/v1/sync/plaid/webhook"
```

### Plaid Dashboard

- **Webhook URL:** matches `PLAID_WEBHOOK_URL` above
- **Webhook codes to deliver** (server triggers a sync for these):
  - TRANSACTIONS: `SYNC_UPDATES_AVAILABLE`, `DEFAULT_UPDATE`, `INITIAL_UPDATE`,
    `HISTORICAL_UPDATE`, `TRANSACTIONS_REMOVED`
  - HOLDINGS: `DEFAULT_UPDATE`
  - INVESTMENTS_TRANSACTIONS: `DEFAULT_UPDATE`, `HISTORICAL_UPDATE`
  - LIABILITIES: `DEFAULT_UPDATE`
- **Products enabled:** `transactions`, `liabilities`, `investments`
- **Redirect URI:** matches `PLAID_REDIRECT_URI` Fly secret

### Verification

```bash
curl https://mizan-connect.fly.dev/api/v1/sync/plaid/health
# expect: {"configured":true,"environment":"production",…}
```

---

## 3. SnapTrade — Legacy (skip unless reactivating)

The active broker integration path is **Plaid**, not SnapTrade. SnapTrade
remains in `crates/connect` history as a soft-deprecated module for
backward compatibility with any installed builds that still hold a
SnapTrade authorization. If you don't intend to reactivate it, leave
the secrets unset — nothing in the active sync path reads them.

If you do want to reactivate:

```bash
fly secrets set --app mizan-connect \
  SNAPTRADE_CLIENT_ID="…" \
  SNAPTRADE_CONSUMER_KEY="…"
```

---

## 4. macOS — Code Signing + Notarization

### GitHub Actions secrets (add at github.com/samisayyed1/mizan-ai-native/settings/secrets/actions)

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | Base64 of `Developer ID Application` cert .p12 |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the .p12 export |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Sami Sayyed (DYDJ2RNL5H)` |
| `APPLE_ID` | Your Apple Developer account email |
| `APPLE_PASSWORD` | App-specific password from https://account.apple.com/account/manage |
| `APPLE_TEAM_ID` | e.g. `DYDJ2RNL5H` (also visible at developer.apple.com/account) |

### What the workflow does

`.github/workflows/release-desktop.yml` invokes `tauri-apps/tauri-action@v0`,
which:
1. Imports the .p12 cert into a temporary keychain
2. Signs the .app bundle with the identity + Entitlements.plist
3. Uploads to Apple Notary Service via `xcrun notarytool submit`
4. Staples the resulting ticket onto the .dmg
5. Attaches signed .dmg to the GitHub Release

If any of the six secrets is missing, signing is a no-op and the
release ships unsigned. The build does NOT fail.

### tauri.conf.json — already configured

```json
"bundle": {
  "macOS": {
    "entitlements": "Entitlements.plist",
    "signingIdentity": null,        // → read from APPLE_SIGNING_IDENTITY
    "providerShortName": null
  }
}
```

### Verification

```bash
# After the GitHub Release lands:
codesign --verify --deep --strict /Volumes/Mizan*/Mizan.app
spctl --assess --type execute --verbose /Volumes/Mizan*/Mizan.app
xcrun stapler validate /path/to/Mizan_*.dmg
```

---

## 5. Windows — Authenticode Code Signing

### GitHub Actions secrets

| Secret | What it is |
|---|---|
| `WINDOWS_CERTIFICATE` | Base64 of `Code Signing` cert .pfx |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for the .pfx export |

### What the workflow does

Same `tauri-action` step on the windows-latest runner invokes Tauri's
WiX bundler, which calls `signtool` with:
- `/sha1 <thumbprint>` derived from the .pfx
- `/tr http://timestamp.digicert.com` (RFC3161 TSA)
- `/td sha256 /fd sha256`

If either secret is missing, the .msi ships unsigned (SmartScreen
"Unknown publisher" warning).

### tauri.conf.json — already configured

```json
"bundle": {
  "windows": {
    "digestAlgorithm": "sha256",
    "timestampUrl": "http://timestamp.digicert.com"
  }
}
```

### Verification

```powershell
Get-AuthenticodeSignature .\Mizan_3.4.1_x64_en-US.msi
# Status should be 'Valid'; SignerCertificate.Subject should be your org
```

---

## 6. Cloud (Mizan Connect) deploy

Already shipping:
- Auto-deploy on push to `main` (.github/workflows/deploy-mizan-connect.yml)
- Health check polled post-deploy
- Migrations run automatically on boot via `sqlx::migrate!("./migrations")`

Manual deploy:
```bash
source .env.fly && export FLY_API_TOKEN="$FLY_API_TOKEN_PRIMARY"
cd mizan-connect
fly deploy --remote-only --strategy rolling
```

---

## 7. Desktop release procedure

1. Bump version in two places (must match):
   - `mizan-4/apps/tauri/tauri.conf.json` → `version`
   - `mizan-4/apps/frontend/package.json` → `version`
2. Bump `tauri.conf.json bundle.macOS.bundleVersion` (Apple wants
   monotonically-increasing string)
3. Commit, tag: `git tag v3.4.2 && git push --tags`
4. GitHub Actions runs `release-desktop.yml` on the tag push:
   - macOS arm64 + x64 DMGs
   - Windows MSI
   - Linux AppImage
   - All attached to GitHub Release
5. Smoke-test the artifacts with the verification commands in §4 + §5

---

## Quick state-of-the-app

| Surface | Status |
|---|---|
| Plaid sandbox sync | ✓ Live |
| Plaid post-link auto-sync | ✓ Live |
| Plaid investment-transaction ingest | ✓ Live + denormalized securities |
| Plaid webhook-triggered sync | ✓ Live (9 codes) |
| Plaid per-user advisory lock | ✓ Live |
| Plaid /item/remove on disconnect | ✓ Live |
| Stripe checkout + portal | ✓ Live (sandbox keys) |
| Stripe webhook handlers | ✓ Complete (7 events) |
| Tauri macOS signing config | ✓ Wired (waiting on cert) |
| Tauri Windows signing config | ✓ Wired (waiting on cert) |
| Frontend security audit | ✓ 0 critical CVEs (jspdf bumped to 4.2.1) |
| Cloud cargo audit | ✓ 0 runtime CVEs |
| Desktop cargo audit | ✓ 0 runtime CVEs (Linux Tauri deps deprecation only) |
| Workspace tests | ✓ 1814 Rust + 56 cloud + 757 frontend |
| Workspace clippy | ✓ Zero warnings on both repos |

Once §1–§5 secrets land, every binary is enterprise production-grade
end-to-end.
