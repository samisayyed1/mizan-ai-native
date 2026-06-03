/**
 * Bar chart primitive — ADR 0019 §"The five primitives".
 *
 * Comparison / ranking. Recharts `BarChart` with horizontal or vertical
 * orientation, configurable. Supports divergent palettes for
 * gain/loss surfaces.
 *
 * Per-primitive extension of [`ChartProps<BarDatum>`].
 */

import { useMemo } from "react";
import {
  Bar as RechartsBar,
  BarChart,
  Cell,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from "recharts";

import type { BarDatum, ChartProps, PaletteKey } from "./types";

const NEUTRAL_VAR = "var(--muted-foreground)";

function fillForBar(palette: PaletteKey, index: number, value: number): string {
  switch (palette) {
    case "divergent": {
      // Negative = chart-loss; positive = chart-gain; zero = neutral.
      if (value > 0) return "var(--chart-gain, var(--chart-2))";
      if (value < 0) return "var(--chart-loss, var(--chart-1))";
      return NEUTRAL_VAR;
    }
    case "mono":
      return "var(--chart-1)";
    case "sequential":
    case "categorical":
    default: {
      const slot = (index % 5) + 1;
      return `var(--chart-${slot})`;
    }
  }
}

export interface BarProps extends ChartProps<BarDatum> {
  /**
   * `"vertical"` (the default — bars rise from the x-axis) or
   * `"horizontal"` (bars extend right from the y-axis). Horizontal is
   * preferred when category labels are long (top-10 lists).
   */
  orientation?: "vertical" | "horizontal";
  /** Show the category-axis labels? Defaults true. */
  showAxisLabels?: boolean;
}

export function Bar({
  data,
  width,
  height = 240,
  palette = "categorical",
  animationMs = 300,
  ariaLabel,
  onTap,
  className,
  orientation = "vertical",
  showAxisLabels = true,
}: BarProps) {
  const safe = useMemo(
    () => data.filter((d) => Number.isFinite(d.value) && d.label !== undefined),
    [data],
  );

  if (safe.length === 0) {
    return (
      <div
        role="img"
        aria-label={ariaLabel}
        className={className}
        data-testid="chart-bar-empty"
        style={{
          width: width ?? "100%",
          height,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--muted-foreground)",
          fontSize: 13,
        }}
      >
        No data
      </div>
    );
  }

  // Recharts uses `layout="vertical"` for horizontal-bar charts (the
  // category axis is Y) and `"horizontal"` for vertical-bar charts. The
  // ADR uses the visually-natural names; we translate here.
  const rechartsLayout = orientation === "horizontal" ? "vertical" : "horizontal";

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      className={className}
      data-testid="chart-bar"
      style={{ width: width ?? "100%", height }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={safe} layout={rechartsLayout} margin={{ top: 8, right: 8, bottom: 8, left: 8 }}>
          {orientation === "vertical" ? (
            <>
              <XAxis
                dataKey="label"
                hide={!showAxisLabels}
                tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
              />
              <YAxis hide tick={{ fontSize: 11 }} />
            </>
          ) : (
            <>
              <XAxis type="number" hide />
              <YAxis
                type="category"
                dataKey="label"
                hide={!showAxisLabels}
                tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                width={90}
              />
            </>
          )}
          <RechartsBar
            dataKey="value"
            isAnimationActive={animationMs > 0}
            animationDuration={animationMs}
            onClick={onTap ? (entry) => onTap(entry as unknown as BarDatum) : undefined}
            radius={[4, 4, 0, 0]}
          >
            {safe.map((d, i) => (
              <Cell key={`${d.label}-${i}`} fill={fillForBar(palette, i, d.value)} />
            ))}
          </RechartsBar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
