# QA Pass NN — [Title]

**Date:** YYYY-MM-DD
**Triggered by:** [user report / agent telemetry / scheduled audit / bug ticket #]
**Severity:** Blocker / Major / Minor / Informational
**Tracks affected:** [A–K]

## Trigger

What surfaced the bug or motivated the pass. One paragraph; cite the exact symptom or signal that prompted the investigation. Examples: a user noticing a wrong number on screen, a Sentry alert spike, a routine audit, a contradicting test result.

## Hypothesis

The suspected root cause before investigation began. Captures what the team thought was happening; useful retroactively to verify whether the right intuition was applied.

## Procedure

Exact steps to reproduce + verify. Pasteable. Each step labeled. The next engineer should be able to redo the QA pass identically.

```
1. ...
2. ...
3. ...
```

## Findings

What was actually wrong. Cite specific file paths + line numbers. Quote the failing test output or the misleading screen value. Don't paraphrase — show.

## Fix

What was changed. PR link(s). Reference the file changes, the test additions, the new invariants. For multi-PR fixes, list each PR and what part it owned.

## Permanent test

What now guards against regression. The test name + location. The QA Pass discipline (working-agreement §6) requires every bug fix to leave a test behind.

## Permanent rule

What new constraint was added to the working agreement, if any. Could be a clippy lint, an ADR, a CI gate, a docs update. If the QA Pass was scoped enough to not warrant a permanent rule, note "N/A — see Finding for one-off fix."

## Why our existing checks didn't catch this

The most important section. Working-agreement §13 (Past Bugs) requires this self-reflection. The goal: figure out what test / monitor / review could have caught it earlier, then add that.

## Action items

| Item | Owner | Deadline |
|---|---|---|
| ... | | |

## Refs

- Spec section(s) consulted
- ADRs created or updated
- Working-agreement sections that govern the area
- Related QA Passes (linked by name)
