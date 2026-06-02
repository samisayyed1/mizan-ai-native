# ADR 0019 — Charting Vocabulary

| Status | ✅ Accepted (autonomous-execution authority — Track A foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [ADR 0018 — Dashboard Information Architecture](0018-dashboard-information-architecture.md), [docs/plans/01-track-a.md](../plans/01-track-a.md), Working-agreement §A19 (perf budgets) |

## Context

Mizan's existing surfaces use a mix of chart types inherited from Wealthfolio: pie charts (allocation), donut + bar mixes (Net Worth historical), polar/radar charts (some asset-detail pages), and Recharts' line + area for the headline historical chart. Across surfaces, the visual vocabulary is inconsistent — a user landing on the Equities panel sees a different chart paradigm than on the Real Estate panel.

The Mizan Evolution Spec §4 prescribes a **5-primitive charting vocabulary**: donut, bar, heatmap, sparkline, Sankey. Everything that surfaces a chart in Tracks A–G uses one of these five.

This ADR locks the choice + the shared API shape so PR-A3 (chart primitives) can ship the implementation against a stable contract.

## Decision

### The five primitives

| Primitive | Use case | File (lands in PR-A3) |
|---|---|---|
| **Donut** | Composition / proportion (`Allocation by asset class`) | `apps/frontend/src/components/charts/donut.tsx` |
| **Bar** | Comparison / ranking (`Top 10 holdings by market value`) | `apps/frontend/src/components/charts/bar.tsx` |
| **Heatmap** | Cross-cutting density (`Heatmap of asset class × 24h move`) | `apps/frontend/src/components/charts/heatmap.tsx` (extend existing) |
| **Sparkline** | Trend strip (`24h delta on Net Worth Strip`) | `apps/frontend/src/components/charts/sparkline.tsx` |
| **Sankey** | Flow (`Quarterly cash flow on Net Worth page`) | `apps/frontend/src/components/charts/sankey.tsx` |

### What's removed

- **Pie charts** — replaced by donut (visually similar but the center hole gives space for a center label per spec §4)
- **Polar / radar charts** — removed wholesale (they're conventional in fitness apps, not in wealth-mgmt UX; the spec's user research showed users misread radar comparisons)
- **3D charts** — removed (perf cost without information gain; out of scope for Track A's perf budget per working-agreement §A19)
- **Bubble charts** — removed (replaced by bar with size encoded as bar length; reduces visual decode load)

### What's kept

- **Historical line chart on Net Worth deep-dive page** — *not* in the dashboard heatmap. It's the deepest detail surface; line is the conventional choice (Recharts `LineChart`) and survives because it complements the 5-primitive vocabulary rather than competing with it.

### Shared component API (all five primitives)

Every chart primitive accepts:

```ts
type ChartProps<T> = {
  /** Data — the primitive picks the relevant fields */
  data: T[];
  /** Width in CSS pixels. If omitted, fills container. */
  width?: number;
  /** Height in CSS pixels. If omitted, defaults per primitive's aspect ratio. */
  height?: number;
  /** Semantic color tokens from packages/ui/src/lib/colors.ts.
      Each primitive maps these to its own color slots. */
  palette?: PaletteKey;
  /** First-paint animation in milliseconds. 0 disables. */
  animationMs?: number;
  /** Aria label for screen readers. */
  ariaLabel: string;
  /** Optional tap callback (mobile primary interaction). */
  onTap?: (item: T) => void;
};
```

Per-primitive specific props extend this base. Documented in PR-A3 with TypeDoc comments.

### Performance contract

Working-agreement §A19 budgets:
- Cold chart paint < 200ms cached, < 1s uncached
- 60fps during interactions (hover, zoom)
- Total bundle size for all five primitives combined: < 60KB gzipped

PR-A3 verifies via:
- Playwright performance trace on the dashboard composition test
- `pnpm run perf-budget` in CI (added with PR-A3)

## Rationale

**Why donut not pie?**
- The center hole gives space for a center label (total value, % of category, etc.) which the dashboard hero uses extensively
- Spec §4 user research showed donuts score higher on "I can quickly read this" than pies for the same data — the offset between angles is easier to read on the inner radius
- Cache size is identical (Recharts `Pie` with `innerRadius > 0` IS a donut)

**Why heatmap is its own primitive (not just "bar in grid")?**
- The dashboard's asset-class heatmap encodes TWO dimensions per tile (% of portfolio = size, 24h move = colour); a bar would need a 2D layout that doesn't generalize
- The existing implementation in the codebase is already a heatmap; we're extending not rewriting

**Why Sankey for cash flow (not stacked bar)?**
- Sankey shows flow + magnitude in one figure; stacked bar shows magnitude only and forces the user to mentally trace "where did the money go" between periods
- The Net Worth deep-dive page is where users actually want flow detail; the dashboard heatmap doesn't need it

**Why sparkline as a distinct primitive (not just "small line chart")?**
- Sparklines have specific design rules (no axes, no labels, single colour, fits in a button-sized area) that don't compose with the line-chart API
- Spec §4 specifies sparklines on every delta chip in the Net Worth Strip + Asset Class Panels; a unified component avoids 12 ad-hoc implementations

**Why semantic color tokens not direct CSS colors?**
- Dark/light mode parity is critical (working-agreement §A14); semantic tokens are the only way to keep dark/light identical without per-component overrides
- Existing `packages/ui/src/lib/colors.ts` already defines the token vocabulary; the primitives consume from there

## Consequences

**Positive:**
- Cross-surface consistency: every chart on Mizan uses one of the five primitives — users learn the vocabulary once
- Smaller bundle: removing pie / polar / radar / 3D variants cuts dependencies + tree-shake removes unused Recharts subcomponents
- Easier dark/light mode + accessibility: one set of primitives to audit + extend
- PR-A4 (remove old chart usages) becomes mechanical: every old chart maps to one of the 5 primitives

**Negative / accepted:**
- The "stacked area by asset class" on the Net Worth deep-dive page (per spec §12) lives just outside the 5-primitive set. Treated as a Bar primitive variant with stacked encoding — PR-A2.b documents this exception inside ADR 0018 + PR-A3 implements the variant
- Existing surfaces that use pie / radar need rework (PR-A4 inventory). The cost is one-time + documented as part of the Track A migration

**Risks:**
- Recharts is the existing chart library + assumed continuation. If a future ADR switches to ECharts or VictoryNative for perf reasons, the 5-primitive API shape (component props) survives — only the internals change. Recharts choice tracked separately in PR-A3's commit body.

## Alternatives considered

- **Visx / D3 directly** — rejected. Visx gives flexibility at the cost of more code per primitive; the spec doesn't need custom visual encodings beyond what Recharts offers. Library swap can happen later if needed without affecting the public API.
- **More primitives (treemap, scatter, candlestick)** — rejected for v1. Treemap conflicts with heatmap visually; scatter / candlestick are niche to the trading surface (Track B). If Track B genuinely needs them, ADR 0019.1 adds them post-rollout.
- **Fewer primitives (just bar + line)** — rejected. The dashboard heatmap is the primary navigation device — replacing it with bars would force a UX rewrite that's outside this ADR's scope.

## Implementation map

| PR | What lands |
|---|---|
| **PR-A2c (this ADR)** | ADR 0019 — locks the vocabulary; no code change |
| **PR-A3** | The 5 primitives shipped, each with TypeDoc + a Playwright smoke test |
| **PR-A4** | Audit + remove pie / polar / radar / 3D usages, replace with donut / bar |
| **PR-A6** | Net Worth Strip uses sparkline primitive |
| **PR-A7** | Dashboard heatmap (already exists, extended) |
| **PR-A9** | Asset Class Panels each use donut / bar / sparkline per spec §3(e) |
| **PR-A2.b** | Sankey on Net Worth deep-dive |

Each PR ≤ 500 lines per working-agreement §A21. Performance budget verified in PR-A3 + carried through to A14's feature-flag rollout.
