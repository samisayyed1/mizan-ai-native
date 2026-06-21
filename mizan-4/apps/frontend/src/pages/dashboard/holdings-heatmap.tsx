/**
 * HoldingsHeatmap — Yahoo-Finance-style P&L heatmap for the dashboard.
 *
 * Renders one rectangle per equity holding (stocks + ETFs + mutual
 * funds — anything classified into the "equities" panel): area scales
 * with current market value (so big positions visually dominate),
 * background tint scales with unrealized-gain percent (green for
 * winners, red for losers, neutral for flat). One glance answers:
 * "what's working, what isn't, and where am I most concentrated?"
 *
 * Cash, alternative assets, sukuks, ULIPs, CPF/provident funds,
 * private equity and anything non-equity are excluded — each has its
 * own dedicated panel tile where return % is expressed in the right
 * units (YTM for sukuks, surrender value for ULIPs). Mixing them into
 * a green/red P&L grid alongside equities is apples-to-oranges.
 *
 * Click a tile → opens the holding's detail page. Mode toggle switches
 * between "Total" (lifetime unrealized P&L) and "Day" (today's change).
 *
 * Visual treatment (UX-9):
 *   * Soft, brand-tonal colour ramp tuned for dark backgrounds — the
 *     previous palette was straight emerald/red Tailwind defaults, which
 *     read industrial. Closer to Robinhood / Apple Stocks now.
 *   * Tile typography includes the symbol, percent change, AND market
 *     value (was just symbol + pct). Big tiles read like trading cards.
 *   * Refined tooltip with bigger title + clear gain framing.
 *   * Honest empty state with a "no day-change data yet" message when
 *     Day mode has no data, instead of silently rendering all-grey
 *     tiles that look broken.
 *   * Container uses the unified Card primitive (rounded-xl + shadow-sm
 *     from UX-7).
 */

import { Card, CardContent, CardHeader, CardTitle } from "@mizan/ui/components/ui/card";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Button } from "@mizan/ui/components/ui/button";
import { HoldingType, isAlternativeAssetKind } from "@/lib/constants";
import { Holding } from "@/lib/types";
import { classifyHolding } from "@/components/asset-class-panels/taxonomy";
import { cn, formatCompactAmount } from "@mizan/ui";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ResponsiveContainer, Treemap, Tooltip as ChartTooltip } from "recharts";

interface HoldingsHeatmapProps {
  holdings: Holding[];
  isLoading: boolean;
  baseCurrency: string;
}

type ReturnMode = "day" | "total";

interface HeatmapDatum {
  name: string;
  size: number; // market value (base currency) — drives tile area
  changePct: number; // signed pct (drives tile colour)
  changeAmount: number;
  symbol: string;
  holdingId: string;
  assetId: string;
  // Recharts' Treemap DataType requires an index signature.
  [key: string]: string | number;
}

const MAX_TILES = 16;

/**
 * Build the asset-detail route for a given asset id. Exported for
 * direct testing — the Treemap onClick handler is hard to drive in
 * jsdom (it depends on SVG measurements via ResponsiveContainer), so
 * the unit test exercises this helper and the integration test in
 * Playwright covers the click-through end to end.
 *
 * Returns `null` when the input doesn't have an `assetId` field —
 * conservative behaviour: refuse to navigate rather than land the
 * user on a broken route.
 */
export function holdingDetailRouteFromNode(node: unknown): string | null {
  if (!node || typeof node !== "object") return null;
  const assetId = (node as { assetId?: unknown }).assetId;
  if (typeof assetId !== "string" || assetId.length === 0) return null;
  return `/holdings/${encodeURIComponent(assetId)}`;
}

/**
 * Smooth divergent gradient — Finviz / Bloomberg Terminal HEAT bar.
 *
 * Replaces the previous 5-bucket palette with a continuous HSL
 * interpolation keyed on |changePct|. Magnitude controls saturation +
 * lightness; sign picks the hue. The result:
 *
 *   - 0%        → desaturated slate-blue (honest "flat", reads as
 *                  no change, never confused with a small gain)
 *   - 0–2%      → softly tinted, desaturated; small moves whisper
 *   - 2–5%      → richer saturation; mid-range positions shout
 *   - 5–10%+    → deep, fully-saturated; tail-end positions dominate
 *                  the chart
 *
 * Magnitude is clamped at 10% (0.10) so a 50% mover doesn't blow
 * past the palette — Finviz uses the same convention so the eye
 * can compare across days without recalibrating.
 *
 * Reference: open Finviz.com on a Friday afternoon. That's the bar.
 */
function tileColor(changePct: number): string {
  const abs = Math.abs(changePct);

  // True neutral for sub-0.1% — slate-blue, not green-tinted.
  if (abs < 0.001) return "hsl(218, 12%, 28%)";

  // Normalize magnitude into [0, 1] capped at 10%. Square-root the
  // ramp so small moves get visibly more saturated quickly, then the
  // top end saturates more gradually — matches human magnitude
  // perception and the Finviz / Bloomberg palette shape.
  const tFactor = Math.min(1, abs / 0.1);
  const t = Math.sqrt(tFactor);

  // Saturation rises from 22% → 70%, lightness drops from 40% → 30%.
  // Empirically the most legible white-text contrast across both
  // hues lands in this band; sat above 75% reads as Tailwind-cartoon.
  const saturation = 22 + Math.round(t * 48);
  const lightness = 40 - Math.round(t * 10);

  // Hue: 142 = forest green, 358 = deep red (both well below
  // primary-color saturation so they don't clash with the brand
  // accent in dark mode).
  const hue = changePct > 0 ? 142 : 358;

  return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
}

/**
 * Recharts Treemap passes the leaf datum's properties at the TOP LEVEL
 * of the content callback's props — NOT wrapped in a `payload` object
 * (that's the Tooltip's convention, not Treemap's). The previous
 * implementation pulled from `props.payload?.symbol` and got undefined
 * for every leaf, so the early-return-null guard fired on every tile
 * and the entire heatmap rendered blank.
 *
 * Source: Recharts treemap.tsx renderContentItem builds props as
 * `{ ...nodeProps, depth, x, y, width, height, root, ...nodeData }`,
 * where nodeData spreads our HeatmapDatum fields directly.
 */
interface TreemapTileProps {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  depth?: number;
  // Fields spread from HeatmapDatum at render time:
  symbol?: string;
  changePct?: number;
  size?: number;
  // Some Recharts versions also expose the original datum under
  // `payload` — kept as a fallback so tile rendering survives either
  // shape.
  payload?: Partial<HeatmapDatum>;
  currency: string;
}

function TreemapTile(props: TreemapTileProps) {
  const { x = 0, y = 0, width = 0, height = 0, depth = 0, currency } = props;

  // Skip the root container rect (depth 0) and any zero-area children.
  if (depth === 0 || width <= 0 || height <= 0) {
    return null;
  }

  // Read from top-level (modern Recharts) then fall back to `payload`
  // (older versions) — covers both API shapes.
  const symbol = props.symbol ?? props.payload?.symbol ?? "";
  const changePct = props.changePct ?? props.payload?.changePct ?? 0;
  const size = props.size ?? props.payload?.size ?? 0;

  const fill = tileColor(changePct);

  // Three label tiers based on tile size — keep wide tiles readable on
  // mobile (where every tile is narrower) and don't clip on dense maps.
  const showSymbol = width > 48 && height > 26;
  const showPct = width > 80 && height > 46;
  const showValue = width > 110 && height > 70;

  const symbolFontSize = Math.min(16, Math.max(11, Math.min(width, height) / 4.5));

  // Heatmap label truncation guard. SVG text doesn't honour CSS
  // `text-overflow: ellipsis`, so we have to clip in JS before
  // rendering. Rough char-budget at the current font size — ~0.55 of
  // the symbol height per glyph (typical sans-serif average advance).
  // The seeder now writes tickerised display_codes (EMAAR29, T-May22,
  // BAJAJ-F, BUKIT-SG…), but third-party imports and CSVs still
  // produce long names — this clip is the safety net for those too.
  const maxLabelChars = Math.max(3, Math.floor((width - 8) / (symbolFontSize * 0.55)));
  const displaySymbol =
    symbol.length > maxLabelChars ? `${symbol.slice(0, Math.max(1, maxLabelChars - 1))}…` : symbol;

  const pctText =
    Math.abs(changePct) < 0.0001
      ? "0.0%"
      : `${changePct >= 0 ? "+" : ""}${(changePct * 100).toFixed(1)}%`;

  return (
    <g className="mizan-heatmap-tile" style={{ cursor: "pointer" }}>
      <rect
        x={x + 1}
        y={y + 1}
        width={Math.max(0, width - 2)}
        height={Math.max(0, height - 2)}
        rx={4}
        ry={4}
        style={{
          fill,
          stroke: "rgba(255, 255, 255, 0.05)",
          strokeWidth: 1,
          transition: "stroke 150ms ease, filter 150ms ease",
        }}
      />
      {showSymbol && (
        <text
          x={x + width / 2}
          y={y + height / 2 - (showPct ? (showValue ? 14 : 8) : 0)}
          textAnchor="middle"
          fill="white"
          fontSize={symbolFontSize}
          fontWeight={650}
          style={{ letterSpacing: "-0.01em", pointerEvents: "none" }}
        >
          {displaySymbol}
        </text>
      )}
      {showPct && (
        <text
          x={x + width / 2}
          y={y + height / 2 + (showValue ? 4 : 10)}
          textAnchor="middle"
          fill="rgba(255, 255, 255, 0.94)"
          fontSize={12}
          fontWeight={500}
          style={{ pointerEvents: "none" }}
        >
          {pctText}
        </text>
      )}
      {showValue && (
        <text
          x={x + width / 2}
          y={y + height / 2 + 20}
          textAnchor="middle"
          fill="rgba(255, 255, 255, 0.72)"
          fontSize={11}
          fontWeight={400}
          style={{ pointerEvents: "none" }}
        >
          {formatCompactAmount(size, currency)}
        </text>
      )}
    </g>
  );
}

interface TreemapTooltipProps {
  active?: boolean;
  payload?: { payload: HeatmapDatum }[];
  currency: string;
  mode: ReturnMode;
}

function TreemapTooltip({ active, payload, currency, mode }: TreemapTooltipProps) {
  if (!active || !payload || payload.length === 0) return null;
  const d = payload[0].payload;
  if (!d) return null;
  const pctText =
    Math.abs(d.changePct) < 0.0001
      ? "0.00%"
      : `${d.changePct >= 0 ? "+" : ""}${(d.changePct * 100).toFixed(2)}%`;
  const amountText = `${d.changeAmount >= 0 ? "+" : ""}${formatCompactAmount(d.changeAmount, currency)}`;
  return (
    <div className="bg-popover text-popover-foreground border-border/70 rounded-xl border px-3.5 py-2.5 text-xs shadow-md">
      <div className="text-foreground text-sm font-semibold tracking-tight">{d.name}</div>
      <div className="text-muted-foreground mt-1 flex items-center gap-2 text-[11px] uppercase tracking-wider">
        <span>{d.symbol}</span>
        <span>·</span>
        <span>{formatCompactAmount(d.size, currency)}</span>
      </div>
      <div
        className={cn(
          "mt-2 font-medium",
          d.changePct >= 0 ? "text-success" : "text-destructive",
        )}
      >
        {pctText} <span className="text-muted-foreground font-normal">{amountText}</span>
      </div>
      <div className="text-muted-foreground/70 mt-1 text-[10px] uppercase tracking-wider">
        {mode === "total" ? "Total return" : "Today"}
      </div>
    </div>
  );
}

export function HoldingsHeatmap({ holdings, isLoading, baseCurrency }: HoldingsHeatmapProps) {
  const navigate = useNavigate();
  const [mode, setMode] = useState<ReturnMode>("total");

  const data = useMemo<HeatmapDatum[]>(() => {
    if (!holdings) return [];
    return holdings
      .filter((h) => h.holdingType !== HoldingType.CASH)
      .filter((h) => !(h.assetKind && isAlternativeAssetKind(h.assetKind)))
      // Heatmap is a stocks-only view — sukuks, ULIPs, CPF, PE etc.
      // already have their own dedicated panel tiles where return % is
      // expressed in the right units (YTM for sukuks, surrender value
      // for ULIPs). Mixing them into a green/red P&L grid alongside
      // equities is apples-to-oranges. Reuses the same `classifyHolding`
      // predicate the dashboard tile grid uses so the two stay in sync.
      .filter((h) => classifyHolding(h) === "equities")
      // Tile size: market value if known, else cost basis. A
      // freshly-seeded position whose quote hasn't fetched yet has
      // marketValue.base = 0; without the cost-basis fallback the
      // whole heatmap shows "No holdings to chart" at cold start
      // even though Equities tile + Holdings page already display
      // the positions correctly. Falling back keeps the visual
      // consistent across all dashboard surfaces.
      .filter((h) => {
        const mv = h.marketValue?.base ?? 0;
        const cb = h.costBasis?.base ?? 0;
        return mv > 0 || cb > 0;
      })
      // Drop aggregate rollup pseudo-assets ("US Stocks", "SG Stocks")
      // — they have no per-position cost basis so they always render as
      // a flat 0.0% gain tile, and because they're aggregates they're
      // the BIGGEST tiles on screen. The hero visual of the dashboard
      // ends up dominated by two uninformative grey rectangles. Drop
      // them so the heatmap shows real positions (HSBC +10.8%, SPUS,
      // ETFs, CPF tranches) where the % colour actually means something.
      .filter((h) => h.instrument?.metadata?.is_rollup !== true)
      .map((h) => {
        const symbol = h.instrument?.symbol ?? h.id;
        const name = h.instrument?.name?.trim() || symbol;
        const changePct =
          mode === "total" ? (h.unrealizedGainPct ?? 0) : (h.dayChangePct ?? 0);
        const changeAmount =
          mode === "total"
            ? (h.unrealizedGain?.base ?? 0)
            : (h.dayChange?.base ?? 0);
        // Size by market value, falling back to cost basis when no
        // live quote has landed yet (cold-start). Without this the
        // tile would be 0 area and effectively invisible.
        const marketSize = h.marketValue?.base ?? 0;
        const costSize = h.costBasis?.base ?? 0;
        const size = marketSize > 0 ? marketSize : costSize;
        return {
          name,
          symbol: symbol.split(".")[0],
          size,
          changePct,
          changeAmount,
          holdingId: h.id,
          assetId: h.instrument?.id ?? h.id,
        };
      })
      .sort((a, b) => b.size - a.size)
      .slice(0, MAX_TILES);
  }, [holdings, mode]);

  // Detect mode having no actual change data — every tile would be
  // neutral slate, which reads as a rendering bug. Surface honestly.
  // Now covers BOTH modes: Total-mode tiles also flat-line when
  // there are no quotes for the underlying assets (cold-start or
  // an exchange the quote scheduler doesn't cover, like LSE-listed
  // Sharia ETFs that ride on TwelveData). The Day-mode-only check
  // missed this; Total-mode users saw a slate-blue grid with no
  // explanation and assumed the heatmap was broken.
  const everyTileIsFlat =
    data.length > 0 &&
    data.every((d) => Math.abs(d.changePct) < 0.0001 && Math.abs(d.changeAmount) < 0.01);
  const dayModeHasNoData = mode === "day" && everyTileIsFlat;
  const totalModeHasNoData = mode === "total" && everyTileIsFlat;

  if (isLoading) {
    return (
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">Heatmap</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          {/* Shimmer skeleton — sweeps a soft highlight across a
              treemap-shaped grid so the loading state previews the
              final composition instead of an opaque block. */}
          <div className="relative h-[260px] w-full overflow-hidden rounded-xl">
            <div className="grid h-full w-full grid-cols-4 grid-rows-3 gap-1">
              {Array.from({ length: 12 }).map((_, i) => (
                <Skeleton key={i} className="h-full w-full rounded-md" />
              ))}
            </div>
            <div
              className="pointer-events-none absolute inset-0"
              style={{
                background:
                  "linear-gradient(110deg, transparent 30%, rgba(255,255,255,0.06) 50%, transparent 70%)",
                animation: "mizan-heatmap-shimmer 1.6s linear infinite",
              }}
            />
            <style>{`
              @keyframes mizan-heatmap-shimmer {
                0%   { transform: translateX(-100%); }
                100% { transform: translateX(100%); }
              }
            `}</style>
          </div>
        </CardContent>
      </Card>
    );
  }

  // True empty state — no tradeable holdings at all.
  if (data.length === 0) {
    return (
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">Heatmap</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          <div className="flex h-[200px] flex-col items-center justify-center gap-3 text-center">
            {/* Subtle line-art tile grid — monochrome, evokes the
                heatmap shape without claiming any specific data. */}
            <svg
              width="64"
              height="48"
              viewBox="0 0 64 48"
              fill="none"
              className="text-muted-foreground/40"
              aria-hidden="true"
            >
              <rect x="1" y="1" width="36" height="28" rx="3" stroke="currentColor" strokeWidth="1" />
              <rect x="39" y="1" width="24" height="14" rx="3" stroke="currentColor" strokeWidth="1" />
              <rect x="39" y="17" width="24" height="12" rx="3" stroke="currentColor" strokeWidth="1" />
              <rect x="1" y="31" width="20" height="16" rx="3" stroke="currentColor" strokeWidth="1" />
              <rect x="23" y="31" width="14" height="16" rx="3" stroke="currentColor" strokeWidth="1" />
              <rect x="39" y="31" width="24" height="16" rx="3" stroke="currentColor" strokeWidth="1" />
            </svg>
            <div>
              <p className="text-foreground text-sm font-medium">No holdings to chart</p>
              <p className="text-muted-foreground mt-1 max-w-[280px] text-xs leading-relaxed">
                Add a stock or ETF to see your equity portfolio at a glance — every tile is one
                position, sized by value, colored by performance.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="overflow-hidden">
      <CardHeader className="flex flex-row items-center justify-between gap-2 pb-2">
        <CardTitle className="text-md font-semibold tracking-tight">Heatmap</CardTitle>
        <div className="bg-muted/60 inline-flex rounded-full p-0.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setMode("total")}
            className={cn(
              "h-7 rounded-full px-3 text-xs font-medium",
              mode === "total" ? "bg-background shadow-sm" : "text-muted-foreground",
            )}
          >
            Total
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setMode("day")}
            className={cn(
              "h-7 rounded-full px-3 text-xs font-medium",
              mode === "day" ? "bg-background shadow-sm" : "text-muted-foreground",
            )}
          >
            Day
          </Button>
        </div>
      </CardHeader>
      <CardContent className="px-2 pb-3 pt-1">
        <style>{`
          .mizan-heatmap-tile:hover rect {
            stroke: rgba(255, 255, 255, 0.25) !important;
            filter: brightness(1.12) drop-shadow(0 2px 4px rgba(0, 0, 0, 0.4));
          }
        `}</style>
        <div className="relative h-[260px] w-full">
          <ResponsiveContainer width="100%" height="100%">
            <Treemap
              data={data}
              dataKey="size"
              stroke="transparent"
              fill="transparent"
              isAnimationActive={false}
              content={
                ((p: TreemapTileProps) => (
                  <TreemapTile {...p} currency={baseCurrency} />
                )) as unknown as React.ReactElement
              }
              onClick={(node: unknown) => {
                const route = holdingDetailRouteFromNode(node);
                if (route) navigate(route);
              }}
            >
              <ChartTooltip
                content={(props) => (
                  <TreemapTooltip
                    active={props.active}
                    payload={props.payload as { payload: HeatmapDatum }[]}
                    currency={baseCurrency}
                    mode={mode}
                  />
                )}
              />
            </Treemap>
          </ResponsiveContainer>

          {dayModeHasNoData && (
            <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/55 backdrop-blur-sm">
              <div className="text-center">
                <p className="text-foreground text-sm font-medium">
                  No day-change data yet
                </p>
                <p className="text-muted-foreground mt-1 text-xs">
                  Mizan needs two consecutive trading days of quotes to compute today's
                  change. Switch to Total for lifetime return.
                </p>
              </div>
            </div>
          )}
          {totalModeHasNoData && (
            <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/55 backdrop-blur-sm">
              <div className="text-center px-4">
                <p className="text-foreground text-sm font-medium">
                  Quotes are still loading
                </p>
                <p className="text-muted-foreground mt-1 text-xs max-w-[280px]">
                  Mizan needs at least one market quote per position to color the
                  heatmap. The quote scheduler runs in the background — refresh
                  in a moment, or open a position to enter a manual price.
                </p>
              </div>
            </div>
          )}
        </div>
        <div className="text-muted-foreground flex items-center justify-between px-2 pt-2 text-[10px] uppercase tracking-wider">
          <span className="flex items-center gap-1">
            <Icons.ChevronDown className="text-destructive h-3 w-3" />
            Losers
          </span>
          <span>Size = market value</span>
          <span className="flex items-center gap-1">
            Winners
            <Icons.ChevronUp className="text-success h-3 w-3" />
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

export default HoldingsHeatmap;
