/**
 * Sankey chart primitive — ADR 0019 §"The five primitives" / Track NW PR-NW2.
 *
 * Flow / decomposition. Renders directed flows from source nodes to
 * target nodes where the link width is proportional to the flow
 * value. The natural fit for "Net Worth → asset classes" on the
 * Net Worth page, and (later, in PR-NW2.b) "income → spending +
 * savings" cash flow.
 *
 * # Wraps Recharts `Sankey`
 *
 * Recharts 3.x exposes `Sankey` with the same composition model as
 * its other primitives. We wrap it the same way `donut.tsx` wraps
 * `PieChart` so dark+light modes share data → colour binding via the
 * `--chart-*` CSS variables.
 *
 * # Data shape
 *
 * Caller provides `nodes: SankeyNode[]` + `links: SankeyLink[]` where
 * link `source`/`target` are indices into the nodes array. This
 * mirrors the canonical d3-sankey shape so existing rollup helpers
 * translate cleanly.
 *
 * # Out of scope
 *
 * - Multi-layer flows beyond source → target (cash flow source →
 *   bucket → outcome lands in PR-NW2.b once activities are threaded)
 * - Hover tooltips with custom rationale (PR-NW2.a — wires the
 *   per-link "X% of Net Worth" label)
 */

import type React from "react";
import { useMemo } from "react";
import { ResponsiveContainer, Sankey, Tooltip } from "recharts";

import type { ChartProps, PaletteKey } from "./types";

/** Single Sankey node — a source OR target in the flow graph. */
export interface SankeyNode {
  /** Display label rendered next to the node. */
  name: string;
}

/** Single Sankey link — a flow between two nodes. */
export interface SankeyLink {
  /** Index into the `nodes` array of the source node. */
  source: number;
  /** Index into the `nodes` array of the target node. */
  target: number;
  /** Flow magnitude — link width is proportional to this. */
  value: number;
}

/** Combined dataset for Sankey rendering. */
export interface SankeyDataset {
  nodes: SankeyNode[];
  links: SankeyLink[];
}

/** Map palette key to the chart-* CSS variable family. */
function paletteVarFamily(palette: PaletteKey): string {
  // All palettes share the same chart-N tokens; the choice of which
  // tokens to weight differently is a future ADR. For PR-NW2 we
  // colour nodes by index modulo 5.
  switch (palette) {
    case "categorical":
    case "sequential":
    case "divergent":
    case "mono":
      return "--chart-";
  }
}

function paletteColor(palette: PaletteKey, index: number): string {
  const slot = (index % 5) + 1;
  return `var(${paletteVarFamily(palette)}${slot})`;
}

export interface SankeyProps extends Omit<ChartProps<never>, "data" | "onTap"> {
  /** Nodes + links for the flow graph. */
  dataset: SankeyDataset;
  /** Optional tap callback for nodes. */
  onNodeTap?: (node: SankeyNode, index: number) => void;
  /** Node padding (px) between adjacent nodes in a column. Default 8. */
  nodePadding?: number;
  /** Margin around the SVG. Defaults reasonable for labels. */
  margin?: { top?: number; right?: number; bottom?: number; left?: number };
}

/**
 * Renders a Recharts `<Sankey>` with the standard Mizan chart-palette
 * theming.
 */
export function Sankey_({
  dataset,
  palette = "categorical",
  width,
  height = 320,
  ariaLabel,
  className,
  nodePadding = 8,
  margin,
  onNodeTap,
}: SankeyProps) {
  // Memoise the recharts input so the chart doesn't re-render on
  // every parent render. The library's diff compares by reference.
  const data = useMemo(
    () => ({ nodes: dataset.nodes, links: dataset.links }),
    [dataset.nodes, dataset.links],
  );

  const handleNodeClick = (entry: { name?: string; index?: number }) => {
    if (!onNodeTap || entry.index === undefined) return;
    const node = dataset.nodes[entry.index];
    if (node) onNodeTap(node, entry.index);
  };

  return (
    <div
      className={className}
      role="img"
      aria-label={ariaLabel}
      style={{ width: width ?? "100%", height }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <Sankey
          data={data}
          nodePadding={nodePadding}
          margin={{
            top: margin?.top ?? 8,
            right: margin?.right ?? 96,
            bottom: margin?.bottom ?? 8,
            left: margin?.left ?? 8,
          }}
          node={(props: NodeProps) => (
            <SankeyNodeShape
              {...props}
              fill={paletteColor(palette, props.index ?? 0)}
              onClick={() =>
                handleNodeClick({ name: props.payload?.name, index: props.index })
              }
            />
          )}
          link={{
            stroke: paletteColor(palette, 0),
            strokeOpacity: 0.35,
            fill: "transparent",
          }}
        >
          <Tooltip />
        </Sankey>
      </ResponsiveContainer>
    </div>
  );
}

/** Re-export under the canonical name (avoiding the recharts collision). */
export { Sankey_ as Sankey };

/** Internal types for the custom node renderer. */
interface NodeProps {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  index?: number;
  payload?: { name?: string };
}

interface SankeyNodeShapeProps extends NodeProps {
  fill: string;
  onClick?: () => void;
}

function SankeyNodeShape({
  x = 0,
  y = 0,
  width = 0,
  height = 0,
  payload,
  fill,
  onClick,
}: SankeyNodeShapeProps): React.ReactElement {
  return (
    <g onClick={onClick} style={{ cursor: onClick ? "pointer" : "default" }}>
      <rect x={x} y={y} width={width} height={height} fill={fill} rx={2} />
      {payload?.name && (
        <text
          x={x + width + 6}
          y={y + height / 2}
          dy="0.35em"
          fontSize={11}
          fill="currentColor"
        >
          {payload.name}
        </text>
      )}
    </g>
  );
}
