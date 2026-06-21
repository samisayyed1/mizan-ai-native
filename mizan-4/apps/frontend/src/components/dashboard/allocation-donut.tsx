/**
 * AllocationDonut — premium donut chart of asset-class allocation,
 * rendered inside the Net Worth strip.
 *
 * Interaction model:
 *   - At rest, the centre label shows the total value + class count.
 *   - On hover (or focus) of a segment, the centre label morphs to
 *     show that segment's name + percentage + value. No floating
 *     tooltip — the centre IS the tooltip. Stays calm + uncluttered.
 *
 * Visual:
 *   - SVG donut, 14-unit ring on a 100×100 viewBox.
 *   - Each segment uses the taxonomy's pre-baked `color` so the same
 *     class shows the same colour everywhere in the app.
 *   - Hovered segment lifts via a 2-unit stroke-width bump (composited
 *     transform, not layout) so the focused class reads clearly.
 *
 * Accessibility:
 *   - The chart announces itself with an aria-label.
 *   - Each segment is a focusable button (keyboard nav round the ring).
 *   - The centre label is live-region polite, so a screen-reader user
 *     hears the segment name as they tab through.
 */
import { formatAmount } from "@mizan/ui/lib/utils";
import { useState } from "react";
import type { CategoryAllocation } from "@/lib/types";

export interface AllocationDonutProps {
  /** Categories from `allocations.assetClasses.categories`. Zero-value entries are filtered out. */
  categories: readonly CategoryAllocation[] | null | undefined;
  /** Display currency for absolute values. */
  baseCurrency: string;
  /** Skeleton state — render the ring shape with no segments. */
  isLoading?: boolean;
  /** Privacy mode hides the absolute value but keeps the chart + class names. */
  isPrivacyMode?: boolean;
  /** Outer diameter in CSS pixels (the card decides sizing). Default 168. */
  size?: number;
}

const RING_RADIUS = 42;
const RING_CIRC = 2 * Math.PI * RING_RADIUS;

/**
 * Distinct-hue palette per asset-class, keyed by the panel id the
 * `classifyHolding` taxonomy returns. The donut used to apply a pure
 * gold ladder which made every slice a shade of gold — Sami's
 * feedback 2026-06-21: "donut was all white, should shade right for
 * different asset classes." Each colour here is picked for (a) high
 * contrast against the dark surface, (b) semantic resonance with the
 * class, and (c) coherent with the Mizan warm-gold brand identity.
 *
 * Equities keep the brand gold-primary because they're the hero
 * class. Everything else gets a distinct hue in the same warm,
 * muted family so the donut reads as ONE chart, not a colour soup.
 */
const PANEL_COLOR: Record<string, string> = {
  equities:             "hsl(40 67% 60%)", // Mizan gold-primary
  "bonds-sukuks":       "hsl(195 35% 55%)", // muted teal — stable, fixed-income
  "bank-cash":          "hsl(155 30% 55%)", // sage green — liquidity
  "real-estate":        "hsl(20 50% 55%)",  // warm terracotta — bricks + mortar
  "private-equity":     "hsl(280 28% 58%)", // muted violet — illiquid
  "provident-funds":    "hsl(220 28% 58%)", // dusk blue — long horizon
  insurance:            "hsl(330 28% 60%)", // soft mauve — protection
  crypto:               "hsl(265 45% 62%)", // electric violet
  commodities:          "hsl(35 55% 50%)",  // bronze
  collectibles:         "hsl(345 35% 58%)", // dusty rose
  forex:                "hsl(180 30% 52%)", // sea-teal
  "brokerage-accounts": "hsl(31 38% 46%)",  // gold-deep — sub-aggregate
  other:                "hsl(31 28% 36%)",  // dimmest
};

// Long-tail fallback ladder for unknown categories — same warm gold
// family, sorted dark→light so smallest slices stay subdued.
const FALLBACK_LADDER = [
  "hsl(31 49% 64%)",
  "hsl(31 42% 52%)",
  "hsl(31 38% 46%)",
  "hsl(31 32% 41%)",
] as const;

/**
 * Resolve a colour for one allocation segment. We try in order:
 *   1. The backend's per-category color (the taxonomy already ships one).
 *   2. Our PANEL_COLOR map keyed by the categoryId.
 *   3. The fallback ladder wrapped by index.
 *
 * That preserves the donut's brand coherence while honouring whatever
 * the backend explicitly set, and never falls back to "white" on a
 * known category.
 */
function colorFor(
  categoryColor: string | undefined,
  categoryId: string,
  index: number,
): string {
  if (categoryColor && categoryColor.trim() && categoryColor !== "transparent") {
    return categoryColor;
  }
  const mapped = PANEL_COLOR[categoryId];
  if (mapped) return mapped;
  return FALLBACK_LADDER[index % FALLBACK_LADDER.length]!;
}

export function AllocationDonut({
  categories,
  baseCurrency,
  isLoading = false,
  isPrivacyMode = false,
  size = 168,
}: AllocationDonutProps) {
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  // Filter out empty / negative categories (liabilities sneak in as
  // negative-value rows; we keep them out of the donut because a
  // negative arc segment isn't a meaningful visual on a pie). Sort
  // descending so the largest segment leads.
  const segments = (categories ?? [])
    .filter((c) => c.value > 0 && c.percentage > 0)
    .sort((a, b) => b.percentage - a.percentage);

  // Cumulative offset so segments butt up around the ring. Each
  // segment gets a gold-ladder colour assigned by its sorted index —
  // largest slice = brightest tone, descending into deeper golds.
  let cum = 0;
  const arcs = segments.map((c, idx) => {
    const length = (c.percentage / 100) * RING_CIRC;
    const offset = -cum;
    cum += length;
    return { ...c, length, offset, color: colorFor(c.color, c.categoryId, idx) };
  });

  const hovered = hoveredId
    ? segments.find((s) => s.categoryId === hoveredId) ?? null
    : null;
  const totalValue = segments.reduce((s, c) => s + c.value, 0);
  const classCount = segments.length;

  return (
    <div
      className="relative shrink-0"
      style={{ width: size, height: size }}
      data-testid="allocation-donut"
    >
      <svg
        viewBox="0 0 100 100"
        className="block h-full w-full -rotate-90"
        role="img"
        aria-label="Net worth allocation by asset class"
      >
        {/* Background track — a calm gold-deep tone, much warmer than
            the neutral muted token. The arcs read as bright gold sat
            on a dimmer gold base, which feels engineered rather than
            "we forgot to colour the ring". */}
        <circle
          cx="50"
          cy="50"
          r={RING_RADIUS}
          fill="none"
          stroke="hsl(31 32% 41% / 0.18)"
          strokeWidth="14"
        />
        {/* Thin outer hairline — gold-cream at low opacity. Sits just
            outside the ring and gives the donut a premium "framed"
            edge without taking visual weight. */}
        <circle
          cx="50"
          cy="50"
          r={RING_RADIUS + 8}
          fill="none"
          stroke="hsl(40 67% 87% / 0.10)"
          strokeWidth="0.5"
        />

        {isLoading ? null : (
          arcs.map((a) => {
            const isHover = a.categoryId === hoveredId;
            return (
              <circle
                key={a.categoryId}
                cx="50"
                cy="50"
                r={RING_RADIUS}
                fill="none"
                stroke={a.color}
                strokeWidth={isHover ? 16 : 14}
                strokeLinecap="butt"
                strokeDasharray={`${a.length} ${RING_CIRC}`}
                strokeDashoffset={a.offset}
                onMouseEnter={() => setHoveredId(a.categoryId)}
                onMouseLeave={() => setHoveredId((id) => (id === a.categoryId ? null : id))}
                onFocus={() => setHoveredId(a.categoryId)}
                onBlur={() => setHoveredId((id) => (id === a.categoryId ? null : id))}
                tabIndex={0}
                role="button"
                aria-label={`${a.categoryName}: ${a.percentage.toFixed(1)} percent, ${formatAmount(a.value, baseCurrency)}`}
                style={{
                  cursor: "pointer",
                  transition: "stroke-width 150ms ease-out",
                }}
                data-testid={`allocation-donut-arc-${a.categoryId}`}
              />
            );
          })
        )}
      </svg>

      {/* Centre label — swaps content based on hover state. The container
          itself doesn't catch pointer events, so hover passes through to
          the arcs underneath. */}
      <div
        className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center px-4"
        aria-live="polite"
      >
        {hovered ? (
          <>
            <div className="text-[10px] font-semibold uppercase tracking-[0.12em] truncate max-w-full" style={{ color: "hsl(31 32% 55%)" }}>
              {hovered.categoryName}
            </div>
            <div className="mt-1 text-2xl font-semibold tabular-nums" style={{ color: "hsl(40 67% 87%)" }}>
              {hovered.percentage.toFixed(1)}%
            </div>
            <div className="text-muted-foreground mt-1 text-[11px] tabular-nums">
              {isPrivacyMode ? "•••" : formatAmount(hovered.value, baseCurrency)}
            </div>
          </>
        ) : (
          <>
            <div className="text-[10px] uppercase tracking-[0.12em]" style={{ color: "hsl(31 32% 55%)" }}>
              Allocation
            </div>
            <div className="mt-1 text-xl font-semibold tabular-nums" style={{ color: "hsl(40 67% 87%)" }}>
              {isPrivacyMode ? "•••" : compactValue(totalValue, baseCurrency)}
            </div>
            <div className="text-muted-foreground mt-1 text-[11px]">
              {classCount} {classCount === 1 ? "class" : "classes"}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

/**
 * Compact a large monetary value for the donut centre — $336K, $1.7M,
 * $1.2B. We don't use `formatAmount` here because the centre is tiny
 * and a full "$336,562.25" string would never fit. Stays in the user's
 * base currency.
 */
function compactValue(value: number, baseCurrency: string): string {
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  // Pick a single $ / £ / € symbol from formatAmount's standard output.
  // Falls back to base-currency code prefix on anything exotic.
  const sample = formatAmount(0, baseCurrency); // e.g. "$0.00"
  const symbol = /^[^\d-]+/.exec(sample)?.[0]?.trim() ?? `${baseCurrency} `;
  if (abs >= 1_000_000_000) return `${sign}${symbol}${(abs / 1_000_000_000).toFixed(1)}B`;
  if (abs >= 1_000_000) return `${sign}${symbol}${(abs / 1_000_000).toFixed(2)}M`;
  if (abs >= 1_000) return `${sign}${symbol}${Math.round(abs / 1_000)}K`;
  return `${sign}${symbol}${Math.round(abs)}`;
}
