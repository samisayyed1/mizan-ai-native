# Runbook: Quarterly Rollback Drill

A practiced rollback procedure for Mizan Connect — confirming that when an
incident demands it, the team can roll back cleanly under pressure.

## When to run

- **Calendar-scheduled**: first business day of each quarter
- Pre-release, when major architectural changes have landed since the last drill
- After any actual production rollback (to validate the new state)

## Prerequisites

- A pre-production / staging environment that mirrors prod schema
- `flyctl` authenticated
- A designated drill leader + observer
- 60 minutes blocked off (drills tend to expose gaps; allow for them)

## Steps

1. **Pick a recent prod deploy as the target.**

   ```bash
   fly releases --app mizan-connect | head -5
   ```

   Pick the version from ~1 week ago. This simulates "we discovered an issue several deploys later."

2. **Take a baseline.**

   - Capture current Sentry error rate
   - Capture current `fly status` output
   - Capture current Postgres migration head: `cargo sqlx migrate info --database-url $DATABASE_URL`
   - Note the current `app_version` in any user telemetry

3. **Execute the rollback against staging (NOT prod).**

   ```bash
   fly releases rollback <target-version> --app mizan-connect-staging
   ```

4. **Verify rollback.**

   - `fly status --app mizan-connect-staging` shows the rolled-back version live
   - `/v1/health` returns the expected (older) version
   - Smoke test 5 critical endpoints (per `deploy.md` step 6)
   - Confirm the DB schema state is compatible with the older binary

5. **Test forward-recovery.**

   - Re-deploy the most recent main: `fly deploy --remote-only --no-cache --app mizan-connect-staging`
   - Verify forward
   - This proves the rollback was reversible

6. **Time and record.**

   Capture:
   - Total wall-clock time from "decide to rollback" to "rollback verified"
   - Any step that was unclear or unexpected
   - Any tooling friction (missing access, slow command, wrong env var)

## Verification

- Staging successfully rolled back
- Staging successfully rolled forward to current main
- Time-to-verified-rollback under 10 minutes (target — adjust if reality differs)

## Output

Write a drill report at `docs/runbooks/drill-reports/YYYY-Q{N}-rollback-drill.md`:

- Date
- Leader + observer
- Target version
- Time elapsed
- Issues found
- Action items (with owners + deadlines)

## Escalation

- If rollback fails on staging: treat as a finding, not an incident. Fix the cause before the next prod deploy.
- If rollback succeeds on staging but a real prod incident requires rollback before the drill's findings are addressed: document the workaround used in the real incident postmortem.

## Related

- `docs/runbooks/deploy.md`
- `docs/runbooks/incident-response.md`
- `docs/working-agreement.md` §19.9
