# Runbook: GDPR / DPDP / CCPA User Data Export

Procedure for fulfilling a user's right-to-export request (sometimes called
"data portability" or "subject access request").

## When to run

- A user clicks "Export my data" in Settings → Privacy
- A user emails support requesting a copy of their data
- A regulator submits a data-subject access request on a user's behalf

## Prerequisites

- Verified identity of the requester (existing session auth is sufficient for
  in-app requests; out-of-band requests require email verification + ID match)
- Access to Mizan Connect admin endpoints (`MIZAN_ADMIN_TOKEN`)
- The request is logged in the support ticketing system with a deadline (30 days per GDPR, 30 days per India DPDP, 45 days per CCPA)

## Scope of the export

Per the working agreement §16.4 and §A20, the export covers **every table holding user data**:

**Desktop SQLite tables** (the user already has these — they live on their own machine):

- `accounts`, `holdings`, `activities`, `quotes`, `fx_rates`
- `daily_account_valuation`, `holdings_snapshot`, `net_worth_snapshot`
- `truth_ledger`, `sync_run_ledger`, `notifications`, `taxonomies`
- `user_memory` (when Track C ships)
- `news_items` (when Track D ships)
- `hawl_anchors` (when Track F ships)
- `projection_snapshots` (when Track C ships)
- `agent_audit_log` (when Track C ships)

**Mizan Connect Postgres** (user-specific rows extracted by `user_id`):

- `users`, `teams`, `team_members`
- `subscriptions`, `stripe_customers`, `stripe_events`
- `broker_connections`, `plaid_items`, `snaptrade_users`
- `sync_run_ledger`
- `audit_events`
- `user_memory_mirror` (Gold+ when Track C ships)
- `oauth_connections` (when Track J ships)
- `mcp_servers`, `mcp_call_log` (when Track K ships)
- `advisor_links` (when Track G ships)

## Steps

1. **Verify identity.**

   - In-app: existing session JWT is sufficient
   - Out-of-band (email / mail / regulator): require photo ID match against the registered email's identity; document verification in the ticket

2. **Trigger the export.**

   In-app path (the button users actually use):

   ```
   Settings → Privacy → "Export my data" → confirm
   ```

   Backend path (when needed for admin or regulator-driven requests):

   ```bash
   curl -X POST https://mizan-connect.fly.dev/v1/admin/user/$USER_ID/export \
     -H "Authorization: Bearer $MIZAN_ADMIN_TOKEN"
   ```

3. **Wait for completion.**

   The export worker:
   - Pulls every row tied to `user_id` across all Mizan Connect tables
   - Triggers the desktop to package its local SQLite tables (via a one-time export bundle)
   - Joins both into a single ZIP containing per-table JSON files
   - Encrypts the ZIP with a per-user passphrase generated for this export
   - Uploads to a time-limited (72h) signed URL

4. **Deliver to the user.**

   - In-app: email contains the signed URL + passphrase
   - Out-of-band: email contains both, or for regulator requests, deliver via the regulator's secure channel

5. **Log the export.**

   Write an entry in `admin_access_log` (or equivalent audit table) capturing:
   - User ID
   - Requester (self / admin / regulator)
   - Timestamp
   - Tables exported
   - Delivery method
   - Signed URL hash (not the URL itself — short-circuit possibility of internal leak)

## Verification

- Open the ZIP and confirm it contains JSON files for every expected table
- Spot-check one or two rows for completeness (right shape, right content)
- The signed URL works for the requester and expires after 72h
- The audit log entry is present

## Rollback / corrections

If the export contains data that shouldn't be there (e.g. another user's row leaked via a join bug):

- Treat as SEV-0 data leak — open incident per `incident-response.md`
- Invalidate the signed URL immediately
- Notify the user the export had an issue, regenerate cleanly
- Investigate the query that produced the leak; add a regression test

## Right-to-delete (related)

A user requesting deletion is a separate flow, also under GDPR/DPDP/CCPA. Per
the working agreement §16.4, the deletion is **cryptographic shredding via
per-user encryption key deletion**, not row-by-row hard delete. The 30-day
soft-delete recovery window applies. Procedure to be written as
`docs/runbooks/gdpr-delete.md` in Track J alongside the OAuth re-consent worker.

## Escalation

If the export fails repeatedly or returns inconsistent data:

- Notify Sami within 24 hours of failure
- The regulator's deadline does not pause for engineering issues; if a 30-day deadline is at risk, ship a manual export (pg_dump + sqlite export by hand) and document the deviation

## Related

- `docs/working-agreement.md` §16.4, §A20
- `docs/runbooks/incident-response.md`
- Future: `docs/runbooks/gdpr-delete.md`
