# Runbook: Incident Response

The procedure for handling a production incident — from first alert to postmortem.

## When to run

- Sentry error rate exceeds 2× rolling 24h average sustained > 5 minutes
- A monitoring-dashboard alert fires (per working agreement §15.10)
- A user reports something broken that affects their financial data
- A security event is detected (failed-login anomaly, unknown sync, vault access)
- A scheduled job (sync run, daily brief, Zakat hawl notification) fails silently

## Prerequisites

- Access to: Sentry, Fly.io dashboard, Supabase dashboard, Stripe dashboard, Slack incident channel
- The current on-call rotation (TODO: maintain in `docs/runbooks/on-call-rotation.md`)
- The current escalation tree

## Severity classification

| Severity | Trigger | Response |
|---|---|---|
| **SEV-0** | Service completely down OR financial data corruption suspected OR security breach | Page immediately; all hands |
| **SEV-1** | A core feature broken for most users (sync, AI agent, billing) | Page on-call; team aware within 30 min |
| **SEV-2** | A feature broken for some users (specific provider, specific tier) | Acknowledge in incident channel; fix within next deploy cycle |
| **SEV-3** | Minor degradation visible to a few users | Open issue; fix in next sprint |

## Steps

1. **Triage (first 5 minutes).**
   - Open an incident channel in Slack: `#incident-YYYYMMDD-short-title`
   - Post the initial signal (Sentry link, user report, monitoring alert)
   - Classify severity (see above)
   - Assign an Incident Commander (IC) — usually whoever opened the channel
   - Assign a Scribe — keeps the timeline log

2. **Stabilize (first 15 minutes).**
   - **Is rollback the right move?** If a recent deploy correlates with the spike, roll back per `deploy.md` and verify the spike subsides.
   - **Is scaling the right move?** If load-related, scale up Connect (`fly scale count N`) or Supabase compute (in the Supabase dashboard).
   - **Is the user-facing impact mitigable?** Set a Mizan Connect feature flag to disable the broken surface while the fix lands (e.g. `feature_flags.broker_sync = false` if Plaid is down).

3. **Communicate.**
   - Internal: post status updates in the incident channel every 15 minutes minimum
   - External (if SEV-0 / SEV-1): publish to `status.mizan.app` and email affected users within 1 hour
   - Use neutral, factual language. Never speculate about root cause publicly until confirmed.

4. **Diagnose.**
   - Read Sentry stack traces and trace IDs
   - Pull request-IDs from the structured `tracing` logs
   - Reproduce in staging with the failing input where possible
   - **Never test diagnostic hypotheses in production by mutating state.**

5. **Resolve.**
   - Land the fix via the regular PR workflow (peer review still required even under pressure)
   - Use the deploy runbook (`deploy.md`) including the `--no-cache` step
   - Verify resolution against the original failing signal

6. **Close the incident.**
   - Confirm error rate returned to baseline
   - Update status page to resolved
   - Schedule the postmortem within 5 business days

## Verification

- Sentry error rate returned to ≤ pre-incident baseline for at least 30 minutes
- The original failing signal no longer reproduces
- Affected users notified of resolution

## Postmortem

Within 5 business days of closure, write `docs/postmortems/YYYY-MM-DD-short-title.md`:

- **Timeline** — every event with timestamps (from the Scribe's log)
- **Impact** — number of users, duration, revenue exposure if any
- **Root cause** — the technical reason
- **Why our checks didn't catch it** — the most important section; what test, what monitor, what review could have caught this earlier
- **Action items** — concrete, owned, with deadlines

Postmortems are **blameless**. The goal is to make the system more resilient,
not to assign fault. Per the working agreement past-bug discipline: when a
bug is fixed, a test is added that would have caught it.

## Escalation

- SEV-0 → notify Sami immediately + page secondary on-call
- SEV-1 → notify Sami within 30 min via Slack DM
- Security event → notify Sami immediately regardless of severity

## Related

- `docs/runbooks/deploy.md` — rollback procedure
- `docs/working-agreement.md` §15.10 — alerting calibration
- `docs/working-agreement.md` §8 — security boundaries
