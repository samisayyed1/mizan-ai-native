# Track E — Mizan Badge Expansion

**Status:** ADR 0011 + migration PR-E1.a shipped; PR-E1.b..E8 pending.
**Estimated sprints:** 1.5.
**Source:** `docs/plans/00-master-plan.md` → "Track E — Mizan Badge Expansion".

## Scope

**In:** 10 new origin variants + 8 new modifier badges (spec §8), badge ordering rules, hover popover content per modifier, AAOIFI screening worker on Mizan Connect.

**Out:** the new sync providers themselves (Track B); the audit-trail "click to verify Truth Ledger hash" UI (Track A Net Worth detail sub-feature); `'mcp'` modifier (Track K).

## PRs

| # | Status | Title |
|---|---|---|
| E1.a | ✅ Done | Migration: `holdings_metadata` side table per ADR 0011 |
| E1.b | ⏸️ Pending | Diesel `schema.rs` regenerated; new model module under `crates/storage-sqlite/src/holdings_metadata/` |
| E1.c | ⏸️ Pending | Repository methods (upsert, select-by-key, scan-by-origin, scan-by-sharia-status) |
| E1.d | ⏸️ Pending | Read-path left-join wiring in holdings render pipeline (defaults gracefully when metadata absent) |
| E1.e | ⏸️ Pending | Write-path wiring for Plaid + SnapTrade + manual + CSV (each writes its `origin`) |
| E2 | ✅ Done (2026-06-04) | `MizanBadge` component in @mizan/ui — origin chip + modifier stack in ADR-0023 severity order (stale > pending-reconciliation > ai-estimated > mixed-compliance > halal-screened > agent-modified > audit-trail > advisor-reviewed > mcp). 3-visible cap with `+N more` overflow chip. Semantic-token variants for dark/light parity. Origin + modifier type unions + `sortModifiersBySeverity` helper exported. 15 vitest tests covering ordering invariants + render branches + accessibility. |
| E3 | ✅ Done (2026-06-04) | `MizanBadgePopover` + `renderBadgePopoverContent` in @mizan/ui. Discriminated-union context type covers all 9 modifiers (8 ADR-0023 + `mcp` from Track K). Each popover surfaces Why / What data / Action per ADR 0023 §"Hover popover content". Consolidated to one file rather than per-modifier files since renderers are <20 LOC each; discriminated union catches modifier-context mismatch at compile time. Pure `renderBadgePopoverContent(context, nowMs)` exported for direct unit testing. 16 vitest tests covering all 9 modifier renderings + clamping + currency fallback + action wiring + runtime guard. |
| E4 | ⏸️ Pending | AAOIFI screening worker on Mizan Connect — debt-ratio / business-activity / interest-income screen per ADR 0012 (planned) |
| E5 | ⏸️ Pending | `find_sharia_status` agent tool in `crates/ai/src/tools/` (uses E4's endpoint) |
| E6 | ⏸️ Pending | Wire `'halal-screened'` / `'mixed-compliance'` badges into existing holdings list views |
| E7 | ⏸️ Pending | Wire `'stale'` / `'pending-reconciliation'` / `'audit-trail'` / `'agent-modified'` / `'advisor-reviewed'` placeholders |
| E8 | ⏸️ Pending | Wire `'ai-estimated'` badge (placeholder until Track B ships the AI estimation pipelines) |

## ADRs

- 0011 — Holdings Metadata Design ✅ written
- 0012 — AAOIFI Screening Criteria (planned, lands with PR-E4)

## Definition of Done

- All 10 origin + 8 modifier variants render correctly with dark/light parity via semantic tokens
- Severity ordering enforced
- Per-modifier popover content implemented
- AAOIFI screening endpoint live with golden-test fixtures for AAPL (unrated), SPUS (compliant), traditional banks (non_compliant)
- Visual regression suite green
- Component docs in `docs/components/mizan-badge.md`

## What's done this session (2026-06-02)

- ADR 0011 — Holdings Metadata Design (resolved JSON-in-portfolio-history reality)
- PR-E1.a — `2026-06-02-000001_holdings_metadata` migration (up.sql + down.sql with 6 indexes + composite PK + check constraints)
- Track E migration is now part of the broader 2026-06-02 foundation-migration batch (7 desktop + 3 cloud migrations total)
