---
name: mizan-action-graph-validator
description: Use when authoring or reviewing FinancialIntentPlan or DraftActionGraph code in mizan-4/crates/ai/src/intent/. Enforces atomic commit, no silent mutation, no partial commits, edit-first dedup, currency + EMI-vs-principal validation.
---

# DraftActionGraph validation checklist

The intent planner is the keystone of Phase C and the moat of every
phase after. Bugs here mean the AI invents financial truth. Apply this
checklist before merging any change inside `crates/ai/src/intent/`.

## Status taxonomy (v3.1 addendum §7)

Every action node must move through exactly one of these states:

`Proposed → NeedsUserInput → Validated → Confirmed → Committed`
or any state → `Blocked | Failed | RolledBack`.

The UI must render the status. A `Committed` node is irreversible only
when the underlying ledger event has been written (Phase L⁺).

## Required invariants

1. **Atomic commit.** If any node in the graph is `Blocked` or `Failed`,
   no node commits. Either the whole graph applies, or none of it does.
   Implement via a single SQLite transaction wrapping every
   `commit_node` call; on the first error, the transaction rolls back
   and every node returns to `Validated` (not `Committed`).
2. **No silent mutation.** Every action node must produce a draft
   payload visible to the user before commit. The user clicks a
   confirm button per node (or "Confirm all" which simply iterates).
3. **Edit-first dedup.** Before drafting a `create_*` node, search the
   relevant core service for a matching `Example —` row. If found,
   downgrade to the equivalent `update_*` node and surface "We're
   editing your example row" in the confirm card.
4. **Currency validation.** Every monetary field carries an explicit
   `CurrencyCode`. If the model omitted it, the node enters
   `NeedsUserInput` — never assume base currency silently.
5. **EMI vs principal.** For `create_liability` / `update_liability`,
   the monthly payment field is **EMI**, not the liability balance.
   If the LLM puts the EMI in the balance field, the validator must
   catch it (heuristic: balance < 10× EMI → block, ask user).
6. **Net-worth impact preview.** Every node must compute and display
   its delta to net worth before commit (using the truth engine's
   `NetWorthSnapshot` from Phase O).
7. **Zakat impact preview.** For Gold users, show the zakat delta.
8. **Provider source labelling.** If the node's data came from Plaid
   or SnapTrade, label it. Never let an AI draft override a
   provider-sourced value without an explicit confirmation that
   says "this overrides your live provider data."

## Failure-mode tests

For any change to `intent_planner.rs` or `action_graph.rs`, write or
extend tests covering:

- Partial-graph failure → all-nodes rollback.
- Mixed-currency multi-node graph → each node validated independently.
- Duplicate-creation prevented when example row exists.
- LLM-confused-EMI-for-balance → blocked, NeedsUserInput.
- Net-worth + zakat deltas correct on a synthetic snapshot.

## When done

Have the `ai-tool-reviewer` subagent independently audit. Run
`mizan-pr-checklist` before push.
