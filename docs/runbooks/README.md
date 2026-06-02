# Runbooks

Operational playbooks for recurring tasks. Per `docs/working-agreement.md` §16, every operational procedure that on-call or ops needs to run repeatably lives here.

## Format

Every runbook follows the standard structure:

1. **When to run** — trigger conditions, schedule, or symptoms
2. **Prerequisites** — credentials, access, tooling, lead time
3. **Steps** — exact commands, in order, with expected output
4. **Verification** — how to confirm the procedure worked
5. **Rollback** — what to do if it fails or needs reversing
6. **Escalation** — who to page when stuck

## Conventions

- File named `kebab-case-task.md`
- Update the runbook when reality changes — stale runbooks are worse than no runbooks
- Reviewed annually per `docs/working-agreement.md` §16
- Verify steps occasionally in a low-traffic window to catch drift

## Active runbooks

| Name | Purpose | Last reviewed |
|---|---|---|
| (none yet) | | |

## Planned (Track H follow-up)

Per `docs/plans/00-master-plan.md` Track H:

- `deploy.md` — Mizan Connect deploy procedure (the `--no-cache` lesson from v37)
- `updater-key-rotation.md` — Tauri updater signing key rotation
- `incident-response.md` — incident triage, comms, postmortem
- `gdpr-export.md` — GDPR / DPDP user data export procedure
- `key-rotation-quarterly.md` — provider encryption key rotation (Plaid, SnapTrade, etc.)
- `rollback-drill.md` — quarterly rollback drill (Track I)
- `supabase-lifecycle.md` — slow-query review, index audit, bloat monitoring (Track I)
