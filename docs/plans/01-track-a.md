# Track A — Dashboard Rewrite

**Status:** PR-A1 done (single "Breakdown" → "Composition" rename); PR-A2..A14 pending.
**Estimated sprints:** 2.
**Depends on:** Track E (badge variants for new panels) + Track I (cache versioning for new layouts).
**Source:** `docs/plans/00-master-plan.md` → "Track A — Dashboard Rewrite".

## Scope

**In:** Remove separate Portfolio surface; restructure dashboard per spec §3 (AI command bar pinned, net worth strip, heatmap, news strip placeholder, 12 asset class panels in fixed order, Today's Signal, quick action pull-up sheet); implement donut + bar charting vocabulary (spec §4); polish notification panel per spec §9.

**Out:** Asset class panel contents (Track B); real news (Track D); new badge variants (Track E first); Sankey on Net Worth page (sub-task — can ship in Track A end or slip).

## PRs

| # | Status | Title |
|---|---|---|
| A1 | ✅ Done | Rename single "Breakdown" H2 → "Composition" on Net Worth page |
| A2 | ⏸️ Pending | Net Worth page skeleton refinement (audit existing `mizan-4/apps/frontend/src/pages/net-worth/` against spec §12 requirements: large historical chart, stacked area, Sankey, liabilities, percentile) |
| A3 | ⏸️ Pending | Shared chart primitives — `components/charts/donut.tsx`, `bar.tsx`, extend existing `heatmap.tsx`, add `sparkline.tsx` |
| A4 | ⏸️ Pending | Remove pie/radar/polar/3D chart usages (audit + replace with donut/bar per the rule) |
| A5 | ✅ Done (2026-06-04 as PR-A6 per v3 Goal §V) | AI command bar — pinned sticky-top above the Net Worth strip; submit navigates to `/assistant?intent=command&prompt=<...>`; voice button routes to `/assistant?voice=1`; 4 §23-themed suggestion chips when empty. 9 vitest tests. Local-dispatcher inline path is C-track follow-up (C3.b + C4 series); the surface is live now. |
| A6 | ✅ Done (2026-06-04 as PR-A5 per v3 Goal §V) | Net Worth Strip — headline + 24h/7d/30d/YTD/All toggle + delta chip + sparkline + tap → /net-worth. 17 vitest tests cover window math + render. |
| A7 | ⏸️ Pending | Heatmap tile-tap → asset detail (already partly implemented; verify and polish) |
| A8 | ⏸️ Pending | News strip placeholder (real wiring in Track D) |
| A9 | ✅ Done (2026-06-04 as PR-A4 per v3 Goal §V) | Twelve-panel asset class skeleton on the dashboard. Fixed §3(e) order per ADR 0021. Classifier maps holdings via assetKind + classifications.assetType.key. Tap → `/holdings?panel=<id>` until Track B PR-B1..B7 swap routes panel-by-panel. |
| A10 | ⏸️ Pending | Today's Signal card (reads from `crates/insights` rules, deduplicated against last 7d) |
| A11 | ⏸️ Pending | Quick action pull-up sheet (Add asset / Run Zakat / Generate report / Talk to Mizan) |
| A12 | ⏸️ Pending | Notification panel polish — alignment + scroll + day buckets + filter chips + sticky header + swipe actions per spec §9.1 |
| A13 | ⏸️ Pending | Remove separate Portfolio surface; sidebar update; redirect old `/portfolio` route with deprecation notice |
| A14 | ⏸️ Pending | Feature flag rollout (`dashboard_v2`): internal → beta opt-in → 25% → 100% over 4h, auto-rollback on Sentry spike |

## ADRs (planned)

ADR numbers 0013 + 0014 from the original plan got reassigned during Track H
(0013 — API deprecation default per memory note `project-api-deprecation-default`;
0014 — MCP defaults per memory note `project-mcp-defaults`). Track A's ADRs
shift to:

- **0018 — Dashboard Information Architecture** (planned — lands with PR-A2 / PR-A5 cluster)
- **0019 — Charting Vocabulary (donut/bar/heatmap/sparkline/Sankey)** (planned — lands with PR-A3)

## PR-A2 audit — existing Net Worth page vs spec §12

Existing surface (as of 2026-06-03, audited in PR-A2):

| File | Lines | Role |
|---|---|---|
| `mizan-4/apps/frontend/src/pages/net-worth/net-worth-content.tsx` | 661 | Page composition: hero balance + category list + timeframe selector |
| `mizan-4/apps/frontend/src/pages/net-worth/net-worth-chart.tsx` | 288 | Historical line/area chart (Recharts) |

Coverage against spec §12 (Net Worth Page requirements):

| Spec §12 requirement | Status | Notes / gap |
|---|---|---|
| Large historical chart with timeframe selector (24h/7d/30d/YTD/All) | ✅ Present | `net-worth-chart.tsx` + `IntervalSelector` from `@mizan/ui` |
| Stacked area by asset class | ⏸️ Gap (PR-A3 prerequisite) | Current chart is single-series line. Stacked area needs the donut+bar primitive vocabulary shipped in PR-A3 |
| Sankey cash-flow diagram | ⏸️ Gap (PR-A2.b) | Not present. Per spec §4 it's optional in v1 — slipped to A2.b |
| Liabilities section | 🟡 Partial | `CATEGORY_COLORS.liabilities` exists but rendering is grouped with assets; spec wants a separate liabilities row with explicit total |
| Percentile / global comparison | ⏸️ Gap | Not present. Spec §12 calls for a small "you're in the Nth percentile globally" chip — needs a cloud endpoint (Mizan Connect) for the global distribution. Slipped to A2.c (needs cloud work) |
| "Composition" section (renamed from "Break Down") | ✅ Present | PR-A1 fix held |
| Vehicles toggle (exclude from net worth) | ✅ Present | `excludeVehiclesFromNetWorth` helper consumed |

**Recommendation:** PR-A2 ships as the AUDIT-ONLY PR (this PR). Code changes for the gaps land as:

- **PR-A2.a** — Liabilities separation (small): split the category list into Assets + Liabilities sections with explicit subtotals
- **PR-A2.b** — Sankey cash-flow primitive (depends on PR-A3 chart vocabulary)
- **PR-A2.c** — Percentile chip (depends on Mizan Connect `/v1/insights/global-percentile/:metric` endpoint — slipped to Track D adjacent work)

Existing perf budget verification (cold-start < 1.2s, chart paint < 200ms cached) — measured: ✅ within budget. To be re-measured at PR-A2.a ship.

## Definition of Done

- Reference user opens app → new dashboard top-to-bottom: AI bar pinned, NW strip, heatmap, news strip, 12 panels in spec §3(e) order, Today's Signal, quick action sheet
- Word "Break Down" / "Breakdown" appears in zero user-facing strings
- All chart types fit the donut+bar+heatmap+sparkline+Sankey vocabulary
- Performance budgets maintained (cold start < 1.2s, chart paint < 200ms cached)
- Notification panel matches spec §9 layout
- Sentry post-rollout error rate ≤ pre-rollout

## What's done

### 2026-06-02

- PR-A1 — Single H2 "Breakdown" → "Composition" rename in `net-worth-content.tsx`

### 2026-06-03

- PR-A2 — Net Worth page audit against spec §12 (this PR). Findings in the
  "PR-A2 audit" section above. Identifies 3 follow-up PRs (A2.a, A2.b, A2.c)
  + the dependency on PR-A3 (chart primitives) before stacked-area can ship.
- ADR-numbering correction: 0013/0014 → 0018/0019 (originals reassigned to
  API deprecation + MCP defaults during Track H).
