# ADR 0009 — Updater Snapshot & Rollback Design

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** I (Cache Invalidation & Versioning Hardening) — PR-I4 / I5 / I6

## Context

Per `docs/working-agreement.md` §19.3:

> The Tauri auto-updater:
> - Signs manifests; signatures verified against the production public key bundled with the binary
> - Takes a pre-update DB snapshot (`mizan.db.pre-{old_version}`, retained 30 days)
> - Runs a post-install self-test on first launch (schema match, crypto round-trip, Twelve Data heartbeat, Mizan Connect heartbeat, Truth Ledger chain head verification)
> - Offers automatic rollback to the snapshot on self-test failure
> - Channels: stable / beta (Gold+ opt-in) / nightly (internal)

The existing updater at `mizan-4/apps/tauri/src/updater.rs` performs the check + download + install + restart. It does NOT yet:

1. Snapshot the SQLite DB before applying the new binary
2. Run a post-install self-test on first launch of the new version
3. Rollback to the snapshot if the self-test fails

This ADR specifies the design of those three pieces, landing as PR-I4 / I5 / I6 in sequence.

## Decision

### PR-I4 — Pre-update DB snapshot

Before `update.download_and_install(...)` completes (i.e. before the new binary starts), the updater copies `mizan.db` to a snapshot file at:

```
${OS_APP_DATA_DIR}/mizan.db.pre-{old_version}
```

For example, on macOS for a 3.4.1 → 3.5.0 update: `~/Library/Application Support/Mizan/mizan.db.pre-3.4.1`.

**Snapshot semantics:**

- Atomic copy via `std::fs::copy` (SQLite WAL/SHM files included via `sqlite3_backup_init` if WAL mode is active — see implementation note below)
- Snapshot retained 30 days (working agreement §19.3). A janitor task at next-app-launch deletes snapshots older than 30d.
- If a snapshot for the SAME `{old_version}` already exists (re-applying the same update), it is preserved — never silently overwritten. The post-install self-test pulls from the existing snapshot.

**Implementation note on WAL mode:** SQLite Write-Ahead Log mode keeps the active DB file in a partially-committed state — `mizan.db` alone is not the full state. The right copy primitive is the `sqlite3_backup_*` API (exposed via `rusqlite::Connection::backup`), not `fs::copy`. The implementation uses `backup` for WAL-mode safety; falls back to `fs::copy` only when WAL is explicitly off.

### PR-I5 — Post-install self-test

On first launch of the new binary, before the WebView is allowed to paint, run a `self_test::run` that checks:

| Check | Pass criterion |
|---|---|
| **Schema match** | `cargo run --bin sqlx-prepare` equivalent passes — every embedded query's expected schema matches the actual DB |
| **Crypto round-trip** | `SecretCipher::from_bytes(&ENCRYPTION_KEY).encrypt(plaintext)` → `.decrypt(ciphertext)` returns the original plaintext |
| **Twelve Data heartbeat** | HTTP GET to a known quote endpoint (`AAPL` price) returns 200 with parseable JSON |
| **Mizan Connect heartbeat** | HTTP GET to `https://mizan-connect.fly.dev/v1/health` returns 200 |
| **Truth Ledger chain head verification** | Read `truth_ledger` latest row, recompute `prev_hash || event_payload → blake3` and compare to `curr_hash` |

Each check has a 5-second timeout; the full self-test budget is 30 seconds.

Self-test results are stored at `${OS_APP_DATA_DIR}/.mizan/self_test_${new_version}.json` so subsequent launches can short-circuit (the test already passed for this version).

### PR-I6 — Auto-rollback on self-test failure

If any self-test check fails:

1. Surface a one-time modal: *"Mizan {new_version} failed startup self-test. Rolling back to {old_version}."*
2. Move the new binary aside: `Mizan.app/Contents/MacOS/Mizan` → `Mizan.app/Contents/MacOS/Mizan.failed-{new_version}` (preserved for diagnostic-bundle pickup, deleted after 7d if no support ticket references it)
3. Restore the snapshot: `mizan.db.pre-{old_version}` → `mizan.db` (via `rusqlite::Connection::restore` if WAL, else `fs::copy`)
4. Restart the app, which then re-launches from the prior binary (Tauri's restart path)
5. Send a structured event to Mizan Connect: `POST /v1/diagnostics/rollback` with the failing check name + version pair + redacted environment info

On Mizan Connect, the team sees rolled-back deployments in the monitoring dashboard (`docs/working-agreement.md` §15.5) and can investigate before promoting the failed version to a broader channel.

## Rationale

**Why a pre-update snapshot rather than relying on Tauri's installer rollback:**

Tauri's installer rollback handles the BINARY, not the DB. A new binary that ran a destructive migration before the self-test caught the issue would leave the DB in a state the old binary can't read. The pre-update DB snapshot is independent of binary state — it's the DB-side belt-and-braces.

**Why 30-day retention:**

Long enough for support cases where the user reports a regression days after upgrading. Short enough to bound disk usage (a 200MB DB × 1 retained = 200MB, acceptable). Working agreement §19.3 picks this number.

**Why a 5-check self-test (not more, not fewer):**

- **Schema match** catches migrations that landed but produced an unexpected state
- **Crypto round-trip** catches encryption key rotation gone wrong (working agreement §3.1)
- **Twelve Data heartbeat** catches the case where the new binary's `TWELVE_DATA_API_BASE` env default changed and is unreachable
- **Mizan Connect heartbeat** catches the case where the new binary's `MIZAN_CONNECT_URL` default changed
- **Truth Ledger chain head** catches the case where a tampering or corruption snuck through (working agreement §0 rule 1, §13)

Anything more is over-engineering and slows the first-launch experience. Anything less leaves real failure modes uncovered.

**Why surface a modal on rollback (not silent):**

Working agreement §17 ("Don't relax a security rule 'temporarily.' Temporary becomes permanent") generalizes to UX: silent failures train users to ignore the system. A surfaced rollback creates the right pressure — every rollback is a real bug we need to fix before the next release.

## Consequences

**Positive:**

- Failed releases can recover without user effort beyond restart
- Support tickets carry rolled-back context (`.failed-{version}` binary + Mizan Connect telemetry)
- The 30-day snapshot retention covers the "I noticed something a few days later" support window
- The rollback event in `/v1/diagnostics/rollback` feeds the monitoring dashboard's auto-rollback counter

**Negative:**

- Each snapshot uses disk equal to `mizan.db` size (typically 50-500 MB per user). 30-day retention multiplies if the user takes 5 updates in 30 days. Mitigation: snapshots older than the SECOND-most-recent are deleted regardless of age.
- Self-test adds ~5-15 seconds to the first launch of a new version. Mitigation: cached result means subsequent launches are unaffected.
- The `.failed-{version}` binary stays on disk 7 days. Mitigation: small footprint (Mizan binary ~80MB), and the diagnostic-bundle pickup window outweighs the cost.

**Follow-ups (tracked):**

- PR-I4: pre-update snapshot + 30d janitor + WAL-aware copy primitive
- PR-I5: self-test crate (`crates/self-test/`) with the 5 checks + 30s budget + cached result
- PR-I6: rollback path + `.failed-{version}` binary handling + `/v1/diagnostics/rollback` endpoint in Mizan Connect
- PR-I6.b: monitoring dashboard widget for rollback events (working agreement §15.5)

## Alternatives Considered

**Alternative A: Skip the snapshot; trust Tauri installer rollback alone.** Rejected — Tauri rollback handles binary, not DB. A destructive migration leaves the DB unreadable by the old binary.

**Alternative B: Run the self-test in the OLD binary BEFORE installing the new one (pre-flight).** Rejected — the new binary's checks must reflect the new binary's code. A schema-match check run by the old binary tests the old binary's schema expectations, not the new one's.

**Alternative C: Async self-test that lets the WebView paint immediately and rolls back on failure mid-session.** Rejected — too disruptive. A self-test failure mid-session means data the user just entered is at risk. Block before paint is the correct trade-off.

## References

- `mizan-4/apps/tauri/src/updater.rs` — existing updater (PR-I4/5/6 land here)
- `docs/working-agreement.md` §19.3
- `docs/runbooks/updater-key-rotation.md` — related runbook
- ADR 0001 — working agreement adoption
- ADR 0008 — cache policy (the eviction-on-version-mismatch path consumes this snapshot lifecycle)
