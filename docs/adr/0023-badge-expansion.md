# ADR 0023 — Mizan Badge expansion

| Status | ✅ Accepted (autonomous-execution authority — Track E foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/05-track-e.md](../plans/05-track-e.md), [ADR 0012 — AAOIFI Screening Criteria](0012-aaoifi-screening-criteria.md), [ADR 0021 — Asset Class Expansion](0021-asset-class-expansion-plan.md), [Spec §8 — Mizan Badge variants] |

## Context

Mizan today ships a `<Badge>` primitive (`mizan-4/packages/ui/src/components/ui/badge.tsx`) supporting a small set of origin variants (manual / synced / etc.) but no modifier system. The Mizan Evolution Spec §8 prescribes a **10-origin + 8-modifier** expansion so the provenance and confidence of every figure on the dashboard is visible at-a-glance.

This ADR locks the variant set, the severity ordering, the hover-popover content shape, and the AAOIFI screening worker on Mizan Connect so PR-E2..E8 land mechanically.

## Decision

### 10 origin variants

Per Spec §8:

| Variant | When | Source |
|---|---|---|
| `manual` | User typed it | Existing — kept |
| `synced` | Sync provider delivered it | Existing — kept |
| `imported` | CSV upload | Existing — kept (PR-H3.e crate extraction backs this) |
| `setu` | Setu Account Aggregator (IN) | NEW (PR-E1 migration shipped enum) |
| `sgfindex` | SGFinDex (SG) | NEW |
| `tink` | Tink (EU) | NEW |
| `basiq` | Basiq (AU) | NEW |
| `lean` | Lean (UAE) | NEW |
| `ccxt` | Crypto exchange via CCXT | NEW |
| `chain_reader` | Read-only blockchain explorer | NEW |
| `twelve_data` | Twelve Data quote feed | NEW |
| `metalprice_api` | Metalprice (commodities) | NEW |
| `bondevalue` | Bondevalue (bond quotes / metadata) | NEW |

The enum migration shipped in PR-E1 (task tracker #15); this ADR documents the per-variant rendering choices.

### 8 modifier badges

Per Spec §8, modifiers stack on top of the origin badge:

| Modifier | Meaning | Trigger | PR |
|---|---|---|---|
| `'stale'` | Data older than the policy's freshness window | Computed at read time vs `cache_policy::CACHE_POLICIES` TTL | PR-E7 |
| `'pending-reconciliation'` | Sync delivered a value that differs from prior; needs user review | Sync layer flag on the holding row | PR-E7 |
| `'ai-estimated'` | Value sourced from AI estimation (real estate / collectibles) | The `'ai-estimated'` writer per ADR 0021 | PR-E8 |
| `'halal-screened'` | AAOIFI-screened as compliant | Track E PR-E4 AAOIFI worker output | PR-E6 |
| `'mixed-compliance'` | AAOIFI returned ambiguous verdict | Same worker, intermediate verdict | PR-E6 |
| `'audit-trail'` | Truth Ledger hash available for verification | Computed when `truth_ledger_entry_id` exists on the row | PR-E7 |
| `'agent-modified'` | The most recent write came from the AI agent (not direct user action) | Set by the dispatcher when emitting a mutation | PR-E7 |
| `'advisor-reviewed'` | A linked Advisor (Track G) has signed off on this holding | Set by the Advisor flow | PR-E7 (skeleton — populated when Track G ships) |
| `'mcp'` | Came from an MCP tool's scratchpad | Track K | PR-E7 (skeleton — populated when Track K ships) |

### Severity ordering (when stacked)

When multiple modifiers apply to one row, the stack renders them in this fixed order (highest-severity first per Spec §8):

```
'stale' > 'pending-reconciliation' > 'ai-estimated' > 'mixed-compliance'
> 'halal-screened' > 'agent-modified' > 'audit-trail' > 'advisor-reviewed' > 'mcp'
```

Rendering caps at 3 visible modifiers — the rest collapse into a `+N more` chip that expands the full stack on hover/tap.

### Hover popover content (per modifier)

Each modifier has a dedicated popover renderer (`packages/ui/src/components/badge/popover-renderers/{modifier}.tsx`) that shows:
- **Why** the badge applies (1 sentence)
- **What data backs it** (e.g. for `'stale'`: "Last synced 14h ago; policy window is 6h")
- **Action** (e.g. for `'pending-reconciliation'`: "Tap to review the diff")

Popover budget: < 50ms hover-to-paint per Spec §17 + working-agreement §A19.

### AAOIFI screening worker

The AAOIFI compliance worker (Track E PR-E4) lives in `mizan-connect/src/sharia/aaoifi_worker.rs`:

1. **Input:** holding ID (deduplicates across users for cache amortization)
2. **Screening rules** per ADR 0012:
   - Debt ratio threshold (33%)
   - Business activity blacklist (alcohol, gambling, conventional banking, defence, pork, tobacco, adult entertainment)
   - Interest income threshold (5%)
3. **Verdict:** `compliant` / `non_compliant` / `mixed_compliance` (ambiguous)
4. **Cache:** per (holding_id, screening_date) — re-runs annually (AAOIFI standard refresh cadence) + on prospectus change
5. **Endpoint:** `GET /v1/sharia/status/:holding_id` returns the cached verdict

### What's NOT in this ADR

- The Truth Ledger explorer (clicking `'audit-trail'` opens a verifier UI) — that lives in the Net Worth deep-dive page; tracked separately.
- Per-user Sharia preference overrides — lives in `user_memory` per Track C; consulted by the badge layer at read time.

## Rationale

**Why so many origin variants (13 total)?**
Each represents a *distinct data path*. Conflating "synced" into one variant loses the visibility into which provider actually delivered the value — and that matters when one provider has an outage or returns stale data. Spec §8 explicitly enumerates the 13.

**Why fixed severity ordering (not user-customisable)?**
Predictability. If `'stale'` and `'audit-trail'` swap positions based on user setting, screenshots in support tickets become harder to interpret + the QA Pass test fixtures become version-dependent. Spec §8 §"Severity ordering" locks the order.

**Why cap at 3 visible modifiers?**
Visual density. A row with all 8 modifiers stacked is unreadable on a phone-sized viewport. 3 is the conventional choice (matches Stripe Dashboard's badge cap).

**Why is AAOIFI a cloud worker (not desktop)?**
Per ADR 0012: the screening is amortizable across users (the verdict for AAPL is the same for every Mizan user); cloud-cached + cross-user reuse cuts the per-user compute to zero after the first user triggers a refresh. Same logic as the ETF look-through worker per ADR 0021.

## Consequences

**Positive:**
- Every figure has visible provenance — supports the working-agreement §0 rule 4 "no data without a source"
- Severity ordering keeps the most-actionable info top-most
- Hover popover gives users a path from "what is this badge" to "what does it mean for me" without leaving the row

**Negative / accepted:**
- 8 modifier renderers + per-variant popovers = 8+ small components. Mitigation: shared base component + per-modifier props files; each renderer ≤ 100 lines.
- AAOIFI cache invalidation is a moving target (when does a company's debt ratio change enough to re-screen?). Mitigation: annual auto-refresh + manual prospectus-change trigger via the cache_eviction worker's manifest comment system (per ADR 0008).

**Risks:**
- AAOIFI screening accuracy depends on the underlying financial data. A wrong verdict could cause a user to invest in non-halal equity. Mitigation: ADR 0012 documents the exact rule values + the screening worker logs every verdict's input parameters for audit; users can override via `user_memory`.
- Popover content for some modifiers (e.g. `'advisor-reviewed'`) depends on Track G data not yet shipped. Mitigation: render the popover with a "Track G coming soon" placeholder until G ships.

## Alternatives considered

- **Fewer origin variants (group all bank providers as `bank_sync`)** — rejected; loses the per-provider provenance Spec §8 explicitly requires.
- **No modifier stack (single badge per row)** — rejected; some rows genuinely need multiple modifiers (e.g. an `'ai-estimated'` AAPL holding that's also `'stale'`).
- **Render modifiers in user-customisable order** — rejected per §"Why fixed severity ordering" above.

## Implementation map

| PR | What lands |
|---|---|
| **PR-E1** | ✅ Migration: new origin enum + `sharia_status` column (already shipped — task tracker #15) |
| PR-E2 | Badge primitive `modifiers[]` prop extension + severity ordering enforcement |
| PR-E3 | Per-modifier popover renderers (8 files; sub-batched if too large for one PR) |
| PR-E4 | AAOIFI screening worker (`mizan-connect/src/sharia/aaoifi_worker.rs`) + `GET /v1/sharia/status/:id` endpoint |
| PR-E5 | `find_sharia_status` AI tool (consumes Track E endpoint per ADR 0020 entry — tracked separately) |
| PR-E6 | Wire `'halal-screened'` / `'mixed-compliance'` badges into existing holdings list views |
| PR-E7 | Wire `'stale'` / `'pending-reconciliation'` / `'audit-trail'` / `'agent-modified'` / `'advisor-reviewed'` (placeholder) / `'mcp'` (placeholder) badges |
| PR-E8 | Wire `'ai-estimated'` badge (depends on PR-B12 / PR-B15 AI estimation pipelines) |

Each PR ≤ 500 lines per working-agreement §A21.
