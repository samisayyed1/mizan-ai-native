/**
 * Heatmap primitive — ADR 0019 §"The five primitives".
 *
 * Cross-cutting density. Recharts `Treemap`: rectangle area encodes
 * `size`, tint encodes `intensity` (positive = gain hue, negative =
 * loss hue, zero = neutral). The dashboard's `holdings-heatmap.tsx`
 * page-level component is the first downstream consumer; PR-A6 will
 * refactor it to compose this primitive.
 *
 * Per-primitive extension of [`ChartProps<HeatmapDatum>`].
 */

import { useMemo } from "react";
import { ResponsiveContainer, Treemap } from "recharts";

import type { ChartProps, HeatmapDatum, PaletteKey } from "./types";

const GAIN_VAR = "var(--chart-gain, var(--chart-2))";
const LOSS_VAR = "var(--chart-loss, var(--chart-1))";
const NEUTRAL_VAR = "var(--muted)";

function tileFill(palette: PaletteKey, intensity: number): string {
  // Clamp intensity to a sensible -1..+1 band so wild outliers don't
  // pop the saturation off-scale.
  const clamped = Math.max(-1, Math.min(1, intensity));
  switch (palette) {
    case "divergent": {
      if (clamped > 0) return GAIN_VAR;
      if (clamped < 0) return LOSS_VAR;
      return NEUTRAL_VAR;
    }
    case "sequential": {
      // Sequential: use chart-1 with opacity scaling.
      const opacity = 0.25 + Math.abs(clamped) * 0.75;
      return `color-mix(in oklch, var(--chart-1) ${Math.round(opacity * 100)}%, transparent)`;
    }
    case "mono":
      return "var(--chart-1)";
    case "categorical":
    default:
      return "var(--chart-1)";
  }
}

export interface HeatmapProps extends ChartProps<HeatmapDatum> {
  /**
   * Aspect ratio of the heatmap container. Treemap's algorithm reads
   * better at 16:9 than at 1:1 on dashboard surfaces. Defaults to 1.78.
   */
  aspectRatio?: number;
}

interface TreemapNode {
  name: string;
  size: number;
  intensity: number;
  label: string;
}

export function Heatmap({
  data,
  width,
  height,
  palette = "divergent",
  animationMs = 300,
  ariaLabel,
  onTap,
  className,
  aspectRatio = 1.78,
}: HeatmapProps) {
  const safe = useMemo(
    () =>
      data
        .filter((d) => Number.isFinite(d.size) && d.size > 0 && Number.isFinite(d.intensity))
        .map<TreemapNode>((d) => ({
          name: d.label,
          size: d.size,
          intensity: d.intensity,
          label: d.label,
        })),
    [data],
  );

  // Default container height: derive from width / aspect ratio when
  // both are absent so the responsive container has something to draw.
  const fallbackHeight = useMemo(
    () => (width ? Math.round(width / aspectRatio) : 240),
    [width, aspectRatio],
  );

  if (safe.length === 0) {
    return (
      <div
        role="img"
        aria-label={ariaLabel}
        className={className}
        data-testid="chart-heatmap-empty"
        style={{
          width: width ?? "100%",
          height: height ?? fallbackHeight,
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
      data-testid="chart-heatmap"
      style={{ width: width ?? "100%", height: height ?? fallbackHeight }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <Treemap
          data={safe}
          dataKey="size"
          stroke="var(--background)"
          fill={tileFill(palette, 0)}
          isAnimationActive={animationMs > 0}
          animationDuration={animationMs}
          onClick={
            onTap
              ? (entry: unknown) => {
                  const node = entry as TreemapNode;
                  onTap({
                    label: node.label,
                    size: node.size,
                    intensity: node.intensity,
                  });
                }
              : undefined
          }
          content={(props: unknown) => {
            const p = props as {
              x: number;
              y: number;
              width: number;
              height: number;
              payload?: TreemapNode;
              name?: string;
              size?: number;
              intensity?: number;
            };
            const node: TreemapNode | undefined =
              p.payload ??
              (p.name !== undefined && p.size !== undefined && p.intensity !== undefined
                ? { name: p.name, label: p.name, size: p.size, intensity: p.intensity }
                : undefined);
            const fill = node ? tileFill(palette, node.intensity) : NEUTRAL_VAR;
            return (
              <g>
                <rect
                  x={p.x}
                  y={p.y}
                  width={p.width}
                  height={p.height}
                  fill={fill}
                  stroke="var(--background)"
                />
                {p.width > 48 && p.height > 24 && node ? (
                  <text
                    x={p.x + p.width / 2}
                    y={p.y + p.height / 2}
                    textAnchor="middle"
                    dominantBaseline="middle"
                    fontSize={11}
                    fill="var(--foreground)"
                  >
                    {node.label}
                  </text>
                ) : null}
              </g>
            );
          }}
        />
      </ResponsiveContainer>
    </div>
  );
}
