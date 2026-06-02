# Runbook: Quarterly Encryption Key Rotation

Mizan Connect encrypts every external provider's access token at rest using
`SecretCipher::from_bytes(&{PROVIDER}_TOKEN_ENCRYPTION_KEY)`. Each key is
rotated quarterly. This is the procedure.

## When to run

- **Calendar-scheduled**: last week of March, June, September, December
- **Ad-hoc**: any time a key is suspected compromised (incident, dev laptop loss, leaked CI log)

## Prerequisites

- `flyctl` authenticated
- `openssl` available
- Mizan Connect deployable from current main (CI green)
- A maintenance window of ~30 minutes (rotation runs in a single transaction; no user impact expected, but a window is courtesy)

## Scope

Per the working agreement §3 and §15.4, every provider that holds an access token has its own encryption key:

| Provider | Env var | Storage location |
|---|---|---|
| Plaid | `PLAID_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| SnapTrade | `MIZAN_BROKER_SECRET_ENCRYPTION_KEY` | Fly secrets |
| Setu (when live) | `SETU_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| SGFinDex (when live) | `SGFINDEX_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| Tink (when live) | `TINK_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| Basiq (when live) | `BASIQ_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| Lean (when live) | `LEAN_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| CCXT crypto (when live) | `CCXT_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| OAuth providers (Track J, when live) | `OAUTH_{PROVIDER}_TOKEN_ENCRYPTION_KEY` | Fly secrets |
| MCP servers (Track K, when live) | `MCP_CREDENTIAL_ENCRYPTION_KEY` | Fly secrets |

Each rotates on its own quarterly cadence. Stagger them across the quarter to limit blast radius if any single rotation fails.

## Steps (per-key)

1. **Generate the new key.**

   ```bash
   openssl rand -base64 32
   ```

   Result: a 44-character base64 string decoding to exactly 32 bytes. Verify:

   ```bash
   echo "$NEW_KEY" | base64 -d | wc -c   # must print 32
   ```

2. **Stage the new key alongside the old.**

   The rotation pattern follows the multi-secret Stripe webhook precedent
   (working agreement §3.2): set both keys simultaneously, deploy, then drop
   the old once the re-encryption completes.

   ```bash
   fly secrets set \
     PLAID_TOKEN_ENCRYPTION_KEY_NEW="$NEW_KEY" \
     --app mizan-connect
   ```

3. **Deploy with the dual-key support code path active.**

   ```bash
   fly deploy --remote-only --no-cache --app mizan-connect
   ```

   Verify per `deploy.md` step 5.

4. **Run the re-encryption migration.**

   ```bash
   cargo run --release --bin rotate-keys -- \
     --provider plaid \
     --old-key-env PLAID_TOKEN_ENCRYPTION_KEY \
     --new-key-env PLAID_TOKEN_ENCRYPTION_KEY_NEW
   ```

   Behaviour: iterate every row in `provider_tokens` for that provider,
   decrypt with old key, re-encrypt with new key, single transaction.
   Failed rows logged and tx rolled back.

5. **Promote new key to primary, drop old.**

   ```bash
   fly secrets set \
     PLAID_TOKEN_ENCRYPTION_KEY="$NEW_KEY" \
     --app mizan-connect
   fly secrets unset PLAID_TOKEN_ENCRYPTION_KEY_NEW --app mizan-connect
   ```

6. **Re-deploy.**

   ```bash
   fly deploy --remote-only --no-cache --app mizan-connect
   ```

7. **Sample-verify**: pick 3 random rows from `provider_tokens` for that provider, decrypt with the new key — must succeed; decrypt attempt with the old key — must fail.

## Verification

- Sample-decrypt 3 rows with new key — pass
- Sample-decrypt 3 rows with old key — fail (expected)
- Sentry shows zero new "decryption failed" errors over the trailing 30 minutes
- One real sync run with the rotated provider completes end-to-end

## Rollback

If re-encryption fails mid-transaction:

- The single-tx pattern guarantees no partial state — old rows still decrypt with old key
- Fix the cause, re-run from step 4
- If the cause is the new key itself, regenerate and start over from step 1

If a re-encrypted row appears corrupt:

- The grace-period dual-key code path lets the old key decrypt fallback rows
- Halt promotion (skip step 5), investigate, do not drop the old key

## Escalation

If multiple provider tokens fail to decrypt with either key:

- Treat as SEV-0 (financial data inaccessible to sync flows)
- Open incident per `incident-response.md`
- Affected users see "Reconnect required" notifications — communicate via status page

## Related

- `docs/runbooks/deploy.md`
- `docs/working-agreement.md` §3.1, §3.2, §15.4
- `docs/adr/0001-adopt-working-agreement-v1.md`
