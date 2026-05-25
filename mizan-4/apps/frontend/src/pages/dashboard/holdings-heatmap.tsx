/**
 * HoldingsHeatmap — Yahoo-Finance-style P&L heatmap for the dashboard.
 *
 * Renders one rectangle per security holding: area scales with current
 * market value (so big positions visually dominate), background tint
 * scales with unrealized-gain percent (green for winners, red for
 * losers, neutral grey for flat). Mirrors the heatmap pattern the user
 * was asking for — a single glance at "what's working, what isn't".
 *
 * Cash and alternative assets (gold, property) are excluded so the
 * heatmap stays focused on tradeable securities where day/total-return
 * actually matter.
 *
 * Click a tile → navigates to the security's detail page.
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
  size: number; // market value (base currency)
  changePct: number; // signed pct (for color)
  changeAmount: number;
  symbol: string;
  holdingId: string;
  // Index signature required by recharts' Treemap DataType.
  [key: string]: string | number;
}

const MAX_TILES = 16;

/**
 * Bucket the absolute change-percent into a colour ramp. We bucket
 * instead of interpolating so adjacent tiles with small spreads stay
 * visually distinct rather than all bleeding into the same near-zero
 * grey — and so a single outlier doesn't compress the rest of the map.
 */
function tileColor(changePct: number): string {
  const abs = Math.abs(changePct);
  if (abs < 0.001) return "rgb(107, 114, 128)"; // grey, flat
  const sign = changePct > 0 ? "pos" : "neg";
  // Five buckets: <0.5%, <1%, <2.5%, <5%, ≥5%
  const bucket = abs < 0.005 ? 0 : abs < 0.01 ? 1 : abs < 0.025 ? 2 : abs < 0.05 ? 3 : 4;
  const greens = [
    "rgb(22, 101, 52)",   // emerald-800
    "rgb(21, 128, 61)",   // green-700
    "rgb(22, 163, 74)",   // green-600
    "rgb(34, 197, 94)",   // green-500
    "rgb(74, 222, 128)",  // green-400 — biggest winner
  ];
  const reds = [
    "rgb(127, 29, 29)",   // red-900
    "rgb(153, 27, 27)",   // red-800
    "rgb(185, 28, 28)",   // red-700
    "rgb(220, 38, 38)",   // red-600
    "rgb(239, 68, 68)",   // red-500 — biggest loser
  ];
  return sign === "pos" ? greens[bucket] : reds[bucket];
}

/**
 * Custom Treemap content — Recharts gives us x/y/width/height for each
 * rectangle plus the original datum. We draw our own rect + label so
 * we can theme it and skip labels on tiles too small to read.
 */
interface TreemapTileProps {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  name?: string;
  changePct?: number;
  symbol?: string;
}

function TreemapTile(props: TreemapTileProps) {
  const { x = 0, y = 0, width = 0, height = 0, changePct = 0, symbol = "" } = props;
  const fill = tileColor(changePct);
  // Hide labels on too-small tiles to avoid clipping
  const showLabel = width > 56 && height > 30;
  const showPct = width > 76 && height > 44;
  const pctText =
    Math.abs(changePct) < 0.0001
      ? "0.0%"
      : `${changePct >= 0 ? "+" : ""}${(changePct * 100).toFixed(1)}%`;
  return (
    <g>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        style={{ fill, stroke: "rgba(0,0,0,0.25)", strokeWidth: 1 }}
      />
      {showLabel && (
        <text
          x={x + width / 2}
          y={y + height / 2 - (showPct ? 8 : 0)}
          textAnchor="middle"
          fill="white"
          fontSize={Math.min(15, Math.max(11, Math.min(width, height) / 4))}
          fontWeight={600}
        >
          {symbol}
        </text>
      )}
      {showPct && (
        <text
          x={x + width / 2}
          y={y + height / 2 + 10}
          textAnchor="middle"
          fill="rgba(255,255,255,0.92)"
          fontSize={11}
          fontWeight={500}
        >
          {pctText}
        </text>
      )}
    </g>
  );
}

interface TreemapTooltipProps {
  active?: boolean;
  payload?: Array<{ payload: HeatmapDatum }>;
  currency: string;
}

function TreemapTooltip({ active, payload, currency }: TreemapTooltipProps) {
  if (!active || !payload || payload.length === 0) return null;
  const d = payload[0].payload;
  if (!d) return null;
  const pctText =
    Math.abs(d.changePct) < 0.0001
      ? "0.0%"
      : `${d.changePct >= 0 ? "+" : ""}${(d.changePct * 100).toFixed(2)}%`;
  return (
    <div className="bg-popover text-popover-foreground rounded-lg border px-3 py-2 text-xs shadow-md">
      <div className="font-semibold">{d.name}</div>
      <div className="text-muted-foreground mt-0.5">
        {formatCompactAmount(d.size, currency)}
      </div>
      <div className={cn("mt-1 font-medium", d.changePct >= 0 ? "text-success" : "text-destructive")}>
        {pctText} {d.changeAmount >= 0 ? "+" : ""}
        {formatCompactAmount(d.changeAmount, currency)}
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
      // Cash holdings (USD/EUR/etc.) don't have meaningful day/total
      // return — the heatmap is for tradeables only. Same reason we
      // exclude property/vehicle/collectible/metal: no daily P&L.
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
        };
      })
      .sort((a, b) => b.size - a.size)
      .slice(0, MAX_TILES);
  }, [holdings, mode]);

  if (isLoading) {
    return (
      <Card className="shadow-xs">
        <CardHeader className="pb-2">
          <CardTitle className="text-md font-semibold">Heatmap</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          <Skeleton className="h-[260px] w-full rounded-md" />
        </CardContent>
      </Card>
    );
  }

  if (data.length === 0) {
    return null;
  }

  return (
    <Card className="shadow-xs overflow-hidden">
      <CardHeader className="flex flex-row items-center justify-between gap-2 pb-2">
        <CardTitle className="text-md font-semibold">Heatmap</CardTitle>
        <div className="bg-muted/60 inline-flex rounded-full p-0.5">
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
        </div>
      </CardHeader>
      <CardContent className="px-2 pb-3 pt-1">
        <div className="h-[260px] w-full">
          <ResponsiveContainer width="100%" height="100%">
            <Treemap
              data={data}
              dataKey="size"
              stroke="rgba(0,0,0,0.25)"
              fill="rgb(107, 114, 128)"
              isAnimationActive={false}
              content={(p) => {
                // p.payload contains the original datum for that tile
                const datum = p as unknown as TreemapTileProps & { payload?: HeatmapDatum };
                return (
                  <TreemapTile
                    x={p.x}
                    y={p.y}
                    width={p.width}
                    height={p.height}
                    name={datum.payload?.name}
                    symbol={datum.payload?.symbol}
                    changePct={datum.payload?.changePct}
                  />
                );
              }}
              onClick={(node: unknown) => {
                const datum = (node as { holdingId?: string })?.holdingId;
                if (datum) navigate(`/holdings`);
              }}
            >
              <ChartTooltip
                content={(props) => (
                  <TreemapTooltip
                    active={props.active}
                    payload={props.payload as Array<{ payload: HeatmapDatum }>}
                    currency={baseCurrency}
                  />
                )}
              />
            </Treemap>
          </ResponsiveContainer>
        </div>
        <div className="text-muted-foreground flex items-center justify-between px-2 pt-2 text-[10px] uppercase tracking-wider">
          <span className="flex items-center gap-1">
            <Icons.ChevronDown className="h-3 w-3 text-red-500" />
            Losers
          </span>
          <span>Size = market value</span>
          <span className="flex items-center gap-1">
            Winners
            <Icons.ChevronUp className="h-3 w-3 text-green-500" />
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

export default HoldingsHeatmap;
