# Runbook: Tauri Updater Signing Key Rotation

The Tauri auto-updater verifies update manifest signatures against the
production public key bundled with the desktop binary. Rotating the signing
key is rare but required: on key compromise, annual hygiene, or hardware
key migration.

## When to run

- Suspected compromise of the private signing key
- Annual hygiene (last week of December)
- Migrating from one HSM / token to another

## Prerequisites

- Access to the current signing key (or, if compromised, ability to revoke)
- Access to the build pipeline that bundles the public key into the desktop binary
- An imminent desktop release planned — rotation ships a new binary with the new public key embedded

## Important constraint

**The public key is embedded in the desktop binary at build time.** A rotated
key only takes effect for users who have updated to a binary built after the
rotation. Users on stale binaries continue to verify against the old public
key. This is by design — the auto-updater chain of trust must not break.

Implication: **you must keep the old key active long enough for users to
upgrade to a binary embedding the new public key.** The Tauri updater
manifest at `mizan.app/updates/latest.json` is signed with whatever key
matches the public key embedded in the user's current binary.

## Steps

1. **Generate the new key pair.**

   Follow [Tauri's official key generation procedure](https://tauri.app/v2/guides/distribution/updater/) — `tauri signer generate`. Store the private key in a hardware key (YubiKey or equivalent) or in a hardened secrets manager. **Never commit to git.**

2. **Embed the new public key in the desktop binary.**

   Update `mizan-4/apps/tauri/tauri.conf.json` `plugins.updater.pubkey` field to the new public key. Build the next desktop release.

3. **Sign the next release manifest with BOTH keys.**

   The Tauri manifest format supports signature arrays. Sign with the old
   key (so users on prior binaries can verify) AND the new key (so users on
   binaries with the new public key embedded can verify).

4. **Publish the release.**

   Follow the desktop release procedure (see Mizan release skill or `mizan-pr-checklist`). Both Mac and Windows updater manifests at `mizan.app/updates/latest.json` are signed with both keys.

5. **Monitor adoption.**

   Track via Sentry / analytics what fraction of installed users have updated to the new binary. Per working agreement §15.1, the monitoring dashboard surfaces "App version distribution."

6. **Drop the old key.**

   Once a sufficient fraction (target: 95%) of users are on binaries with
   the new public key, stop signing release manifests with the old key.
   The grace period is typically 30-90 days depending on update adoption.

7. **Securely destroy the old private key.**

   Wipe the hardware key, delete the secrets-manager entry, document the
   destruction in the audit log.

## Verification

- New release manifest verifies against both old and new public keys (run `tauri signer verify`)
- Users on prior binaries can still install the new release without "signature mismatch" errors (test on a stale build)
- Users on the new binary verify against the new key (test on a fresh install)

## Rollback

If the new binary fails signature verification on a real user device:

- Republish the manifest signed with only the old key
- Investigate the cause (key embedding mismatch, manifest format issue)
- Do not destroy the old key until the issue is resolved

## Escalation

If the old key is confirmed compromised AND users are at risk of malicious update:

- Treat as SEV-0 security incident
- Coordinate with Sami immediately
- Consider an emergency `mizan-update-emergency` channel that pushes a notification (not an update) telling users to reinstall from a verified source
- Republish all current channels with new key only; users on old binaries get an explicit "update required, reinstall from mizan.app" prompt

## Related

- `docs/runbooks/deploy.md`
- `docs/runbooks/incident-response.md`
- `docs/working-agreement.md` §19.3
- [Tauri Updater documentation](https://tauri.app/v2/guides/distribution/updater/)
