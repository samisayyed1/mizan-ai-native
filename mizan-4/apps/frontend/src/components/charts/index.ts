/**
 * Shared chart primitives — Track A PR-A3 + Track NW PR-NW2.
 *
 * Implements the 5-primitive vocabulary from ADR 0019 (donut, bar,
 * heatmap, sparkline, Sankey). PR-A3 shipped the first four; PR-NW2
 * closes the vocabulary by adding Sankey alongside the Net Worth
 * deep-dive page.
 *
 * All primitives accept the [`ChartProps<T>`] base + per-primitive
 * extensions. Use these for any new chart surface across Tracks A–G.
 * The page-specific holdings-heatmap on the dashboard still owns its
 * page-level composition; PR-A6 will fold it onto this generic
 * primitive once the dashboard rewrite (PR-A4/A5) ships.
 */

export type { ChartProps, PaletteKey, DonutDatum, BarDatum, SparklineDatum, HeatmapDatum } from "./types";
export { Donut } from "./donut";
export { Bar } from "./bar";
export { Sparkline } from "./sparkline";
export { Heatmap } from "./heatmap";
export { Sankey } from "./sankey";
export type { SankeyNode, SankeyLink, SankeyDataset, SankeyProps } from "./sankey";
