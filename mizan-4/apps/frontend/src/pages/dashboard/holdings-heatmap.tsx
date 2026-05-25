/**
 * HoldingsHeatmap — Yahoo-Finance-style P&L heatmap for the dashboard.
 *
 * Renders one rectangle per security holding: area scales with current
 * market value (so big positions visually dominate), background tint
 * scales with unrealized-gain percent (green for winners, red for
 * losers, neutral for flat). One glance answers: "what's working, what
 * isn't, and where am I most concentrated?"
 *
 * Cash and alternative assets (gold, property, vehicles) are excluded
 * so the heatmap stays focused on tradeable securities where percent
 * return actually matters.
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
 * Refined colour ramp. The previous palette was straight Tailwind green-
 * 400..emerald-800 / red-500..red-900 — saturated, "industrial". Tuned
 * down for premium feel:
 *   - Five magnitude buckets so adjacent positions stay visually distinct
 *     instead of bleeding into one wash when ranges are tight.
 *   - Slightly more luminous than the previous palette so tile text stays
 *     legible at all sizes.
 *   - Neutral is a true mid-grey, not a green-tinted one — so a flat
 *     holding reads as honest "no change" rather than washed-out winner.
 */
function tileColor(changePct: number): string {
  const abs = Math.abs(changePct);
  if (abs < 0.001) return "#3f4754"; // neutral slate
  const sign = changePct > 0 ? "pos" : "neg";
  const bucket = abs < 0.005 ? 0 : abs < 0.01 ? 1 : abs < 0.025 ? 2 : abs < 0.05 ? 3 : 4;
  const greens = [
    "#1f5236",
    "#1f6a3f",
    "#1f8546",
    "#1ea34c",
    "#19bf57",
  ];
  const reds = [
    "#5a1f25",
    "#7a2027",
    "#9c2229",
    "#c0252b",
    "#dc2a31",
  ];
  return sign === "pos" ? greens[bucket] : reds[bucket];
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

  const pctText =
    Math.abs(changePct) < 0.0001
      ? "0.0%"
      : `${changePct >= 0 ? "+" : ""}${(changePct * 100).toFixed(1)}%`;

  return (
    <g>
      <rect
        x={x + 1}
        y={y + 1}
        width={Math.max(0, width - 2)}
        height={Math.max(0, height - 2)}
        rx={4}
        ry={4}
        style={{
          fill,
          stroke: "rgba(255, 255, 255, 0.04)",
          strokeWidth: 1,
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
          {symbol}
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
  payload?: Array<{ payload: HeatmapDatum }>;
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
      .filter((h) => (h.marketValue?.base ?? 0) > 0)
      .map((h) => {
        const symbol = h.instrument?.symbol ?? h.id;
        const name = h.instrument?.name?.trim() || symbol;
        const changePct =
          mode === "total" ? (h.unrealizedGainPct ?? 0) : (h.dayChangePct ?? 0);
        const changeAmount =
          mode === "total"
            ? (h.unrealizedGain?.base ?? 0)
            : (h.dayChange?.base ?? 0);
        return {
          name,
          symbol: symbol.split(".")[0],
          size: h.marketValue?.base ?? 0,
          changePct,
          changeAmount,
          holdingId: h.id,
          assetId: h.instrument?.id ?? h.id,
        };
      })
      .sort((a, b) => b.size - a.size)
      .slice(0, MAX_TILES);
  }, [holdings, mode]);

  // Detect Day-mode having no actual day-change data — every tile would
  // be neutral grey, which reads as a rendering bug. Surface honestly.
  const dayModeHasNoData =
    mode === "day" &&
    data.length > 0 &&
    data.every((d) => Math.abs(d.changePct) < 0.0001 && Math.abs(d.changeAmount) < 0.01);

  if (isLoading) {
    return (
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-md font-semibold tracking-tight">Heatmap</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          <Skeleton className="h-[260px] w-full rounded-xl" />
        </CardContent>
      </Card>
    );
  }

  // True empty state — no tradeable holdings at all.
  if (data.length === 0) {
    return (
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-md font-semibold tracking-tight">Heatmap</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          <div className="text-muted-foreground flex h-[200px] items-center justify-center text-xs">
            Add a stock to see your portfolio heatmap.
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
                const assetId = (node as { assetId?: string })?.assetId;
                if (assetId) navigate(`/holdings/${encodeURIComponent(assetId)}`);
              }}
            >
              <ChartTooltip
                content={(props) => (
                  <TreemapTooltip
                    active={props.active}
                    payload={props.payload as Array<{ payload: HeatmapDatum }>}
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
