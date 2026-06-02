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
| A5 | ⏸️ Pending | AI command bar — pinned top, does not collapse on scroll, connects to local dispatcher with optional escalation to `/v1/ai/agent` for Gold+ |
| A6 | ⏸️ Pending | Net worth strip — toggleable 24h/7d/30d/YTD/All deltas + sparkline + tap → Net Worth page |
| A7 | ⏸️ Pending | Heatmap tile-tap → asset detail (already partly implemented; verify and polish) |
| A8 | ⏸️ Pending | News strip placeholder (real wiring in Track D) |
| A9 | ⏸️ Pending | Asset class panel skeleton + 12 panels in fixed spec §3(e) order |
| A10 | ⏸️ Pending | Today's Signal card (reads from `crates/insights` rules, deduplicated against last 7d) |
| A11 | ⏸️ Pending | Quick action pull-up sheet (Add asset / Run Zakat / Generate report / Talk to Mizan) |
| A12 | ⏸️ Pending | Notification panel polish — alignment + scroll + day buckets + filter chips + sticky header + swipe actions per spec §9.1 |
| A13 | ⏸️ Pending | Remove separate Portfolio surface; sidebar update; redirect old `/portfolio` route with deprecation notice |
| A14 | ⏸️ Pending | Feature flag rollout (`dashboard_v2`): internal → beta opt-in → 25% → 100% over 4h, auto-rollback on Sentry spike |

## ADRs (planned)

- 0013 — Dashboard Information Architecture
- 0014 — Charting Vocabulary (donut/bar/heatmap/sparkline/Sankey)

## Definition of Done

- Reference user opens app → new dashboard top-to-bottom: AI bar pinned, NW strip, heatmap, news strip, 12 panels in spec §3(e) order, Today's Signal, quick action sheet
- Word "Break Down" / "Breakdown" appears in zero user-facing strings
- All chart types fit the donut+bar+heatmap+sparkline+Sankey vocabulary
- Performance budgets maintained (cold start < 1.2s, chart paint < 200ms cached)
- Notification panel matches spec §9 layout
- Sentry post-rollout error rate ≤ pre-rollout

## What's done this session (2026-06-02)

- PR-A1 — Single H2 "Breakdown" → "Composition" rename in `net-worth-content.tsx`
