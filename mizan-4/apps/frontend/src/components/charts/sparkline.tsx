/**
 * Sparkline primitive — ADR 0019 §"The five primitives".
 *
 * Trend strip. Lightweight: no axes, no grid, no tooltip. Used inline
 * with delta chips (Net Worth strip + Asset Class panels). The mono
 * palette is the conventional default — sparklines are ambient chrome.
 *
 * Per-primitive extension of [`ChartProps<SparklineDatum>`].
 */

import { useMemo } from "react";
import { Line, LineChart, ResponsiveContainer } from "recharts";

import type { ChartProps, PaletteKey, SparklineDatum } from "./types";

function strokeForSparkline(palette: PaletteKey, trend: number): string {
  switch (palette) {
    case "divergent":
      if (trend > 0) return "var(--chart-gain, var(--chart-2))";
      if (trend < 0) return "var(--chart-loss, var(--chart-1))";
      return "var(--muted-foreground)";
    case "mono":
      return "var(--foreground)";
    case "sequential":
    case "categorical":
    default:
      return "var(--chart-1)";
  }
}

export interface SparklineProps extends ChartProps<SparklineDatum> {
  /** Stroke width in CSS pixels. Defaults to 1.5. */
  strokeWidth?: number;
}

export function Sparkline({
  data,
  width,
  height = 32,
  palette = "mono",
  animationMs = 200,
  ariaLabel,
  onTap,
  className,
  strokeWidth = 1.5,
}: SparklineProps) {
  const safe = useMemo(
    () =>
      data.filter((d) => Number.isFinite(d.x) && Number.isFinite(d.y)).sort((a, b) => a.x - b.x),
    [data],
  );

  // Compute trend for divergent palettes: positive if last > first.
  const trend = useMemo(() => {
    if (safe.length < 2) return 0;
    return safe[safe.length - 1].y - safe[0].y;
  }, [safe]);

  if (safe.length < 2) {
    return (
      <div
        role="img"
        aria-label={ariaLabel}
        className={className}
        data-testid="chart-sparkline-empty"
        style={{
          width: width ?? "100%",
          height,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--muted-foreground)",
          fontSize: 10,
        }}
      >
        —
      </div>
    );
  }

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      className={className}
      data-testid="chart-sparkline"
      onClick={onTap ? () => onTap(safe[safe.length - 1]) : undefined}
      style={{ width: width ?? "100%", height, cursor: onTap ? "pointer" : undefined }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={safe} margin={{ top: 2, right: 2, bottom: 2, left: 2 }}>
          <Line
            type="monotone"
            dataKey="y"
            stroke={strokeForSparkline(palette, trend)}
            strokeWidth={strokeWidth}
            dot={false}
            isAnimationActive={animationMs > 0}
            animationDuration={animationMs}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
