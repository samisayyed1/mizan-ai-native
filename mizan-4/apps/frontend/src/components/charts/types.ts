/**
 * Shared chart-primitive types — Track A PR-A3 / ADR 0019.
 */

/**
 * Color-token palette key. Maps to semantic tokens declared in
 * `packages/ui/src/styles.css` (`--chart-1` ... `--chart-5`) so dark
 * and light modes share the same data → color binding.
 *
 * - `categorical` — qualitative slices (donut allocation, bar comparison)
 * - `sequential` — heatmap intensity, sparkline trend
 * - `divergent` — gains-vs-losses (red-to-green for P&L surfaces)
 * - `mono` — single-hue for ambient chrome (header sparklines)
 */
export type PaletteKey = "categorical" | "sequential" | "divergent" | "mono";

/**
 * Base props every chart primitive accepts. Per-primitive props extend
 * this with their data shape via [`DonutDatum`], [`BarDatum`], etc.
 *
 * ADR 0019 §"Shared component API" pins this contract.
 */
export interface ChartProps<T> {
  /** Data — the primitive picks the relevant fields. */
  data: T[];
  /** Width in CSS pixels. If omitted, fills container. */
  width?: number;
  /** Height in CSS pixels. If omitted, defaults per primitive's aspect ratio. */
  height?: number;
  /** Semantic color palette (see [`PaletteKey`]). */
  palette?: PaletteKey;
  /** First-paint animation in milliseconds. `0` disables. Defaults to 300. */
  animationMs?: number;
  /** Aria label for screen readers — REQUIRED (no default; every chart MUST be labeled). */
  ariaLabel: string;
  /** Optional tap callback (mobile primary interaction). */
  onTap?: (item: T) => void;
  /** Optional CSS class for outer wrapper. */
  className?: string;
}

/** Datum for [`Donut`]. */
export interface DonutDatum {
  /** Display label. */
  label: string;
  /** Slice value — must be non-negative. */
  value: number;
}

/** Datum for [`Bar`]. */
export interface BarDatum {
  /** X-axis category label. */
  label: string;
  /** Y-axis value — can be negative for divergent palettes. */
  value: number;
}

/** Datum for [`Sparkline`]. */
export interface SparklineDatum {
  /** X-axis position — typically a timestamp or sequence index. */
  x: number;
  /** Y-axis value. */
  y: number;
}

/** Datum for [`Heatmap`]. */
export interface HeatmapDatum {
  /** Cell label. */
  label: string;
  /** Cell area (relative). */
  size: number;
  /** Cell color intensity / tint (typically -1.0 .. +1.0). */
  intensity: number;
}
