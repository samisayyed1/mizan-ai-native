/**
 * Donut chart primitive — ADR 0019 §"The five primitives".
 *
 * Composition / proportion. Recharts `PieChart` with `innerRadius > 0`
 * (per ADR 0019 §"Why donut not pie?"). Renders a centered label slot
 * via children for total / category info.
 *
 * Per-primitive extension of [`ChartProps<DonutDatum>`].
 */

import type React from "react";
import { useMemo } from "react";
import { Cell, Pie, PieChart, ResponsiveContainer } from "recharts";

import type { ChartProps, DonutDatum, PaletteKey } from "./types";

// Map palette keys to the semantic chart-* CSS variables declared in
// packages/ui/src/styles.css. Recharts accepts CSS-var strings as
// fill, so dark + light modes share the same data → color binding.
const PALETTE_CSS_VAR_PREFIX: Record<PaletteKey, string> = {
  categorical: "var(--chart-",
  sequential: "var(--chart-",
  divergent: "var(--chart-",
  mono: "var(--chart-",
};

/** Read color N (1-indexed) from a CSS var. Wraps around at slot 5. */
function paletteColor(palette: PaletteKey, index: number): string {
  const slot = (index % 5) + 1;
  return `${PALETTE_CSS_VAR_PREFIX[palette]}${slot})`;
}

export interface DonutProps extends ChartProps<DonutDatum> {
  /**
   * Inner radius as a ratio of outer radius (0..1). Defaults to 0.6 —
   * gives a center label slot that's prominent without overwhelming
   * the slices.
   */
  innerRadiusRatio?: number;
  /**
   * Optional centered label content. Rendered as an SVG-overlay element
   * absolutely centered within the donut hole. Use for total value /
   * category name.
   */
  centerLabel?: React.ReactNode;
}

export function Donut({
  data,
  width,
  height = 240,
  palette = "categorical",
  animationMs = 300,
  ariaLabel,
  onTap,
  className,
  innerRadiusRatio = 0.6,
  centerLabel,
}: DonutProps) {
  // Filter degenerate input. Recharts crashes on negative slice values;
  // surface a deliberate empty render instead.
  const safe = useMemo(
    () => data.filter((d) => Number.isFinite(d.value) && d.value >= 0),
    [data],
  );

  const total = useMemo(
    () => safe.reduce((sum, d) => sum + d.value, 0),
    [safe],
  );

  // Empty-state: ADR 0019 requires every primitive to render an honest
  // empty state rather than a blank chart.
  if (safe.length === 0 || total === 0) {
    return (
      <div
        role="img"
        aria-label={ariaLabel}
        className={className}
        data-testid="chart-donut-empty"
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

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      className={className}
      data-testid="chart-donut"
      style={{ width: width ?? "100%", height, position: "relative" }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <PieChart>
          <Pie
            data={safe}
            dataKey="value"
            nameKey="label"
            innerRadius={`${Math.round(innerRadiusRatio * 100)}%`}
            outerRadius="100%"
            paddingAngle={1}
            isAnimationActive={animationMs > 0}
            animationDuration={animationMs}
            onClick={onTap ? (entry) => onTap(entry as DonutDatum) : undefined}
          >
            {safe.map((d, i) => (
              <Cell key={`${d.label}-${i}`} fill={paletteColor(palette, i)} />
            ))}
          </Pie>
        </PieChart>
      </ResponsiveContainer>
      {centerLabel ? (
        <div
          data-testid="chart-donut-center-label"
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            pointerEvents: "none",
            textAlign: "center",
          }}
        >
          {centerLabel}
        </div>
      ) : null}
    </div>
  );
}
