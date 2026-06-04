/**
 * Equities panel rollup — Track B PR-B2 / Goal v3 §V Phase 5.
 *
 * Pure-math helpers that take the user's full holdings list and emit
 * the two §23 chart views for the Equities panel:
 *   - **donut by sub-class** (Stocks / ETFs / Mutual Funds / Options) —
 *     allocation within the equity bucket
 *   - **bar by region** — top-level geographic exposure from
 *     `instrument.classifications.regions[].topLevelCategory`
 *
 * `isEquityHolding` mirrors the desktop scheduler predicate semantics
 * — the UI-side and scheduler-side filters must agree on what "an
 * equity" is for the bond/equity-class boundary.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching equity-classified holdings: EQUITY / STOCK / ETF /
 * MUTUAL_FUND / OPTION on `assetType.key`. */
export function isEquityHolding(h: Holding): boolean {
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  return (
    key === "EQUITY" ||
    key === "STOCK" ||
    key === "ETF" ||
    key === "MUTUAL_FUND" ||
    key === "OPTION"
  );
}

/** Sub-classes the donut groups holdings into. */
export type EquitySubclass = "Stocks" | "ETFs" | "Mutual Funds" | "Options" | "Other";

function classifyEquitySubclass(h: Holding): EquitySubclass {
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  if (key === "ETF") return "ETFs";
  if (key === "MUTUAL_FUND") return "Mutual Funds";
  if (key === "OPTION") return "Options";
  if (key === "EQUITY" || key === "STOCK") return "Stocks";
  return "Other";
}

/** Per-subclass donut row. */
export interface SubclassRow {
  label: EquitySubclass;
  value: number;
}

/**
 * Roll equity holdings into per-subclass allocation rows, descending
 * by `value`. Donut consumer expects { label, value } shape.
 */
export function rollupBySubclass(holdings: readonly Holding[]): SubclassRow[] {
  const map = new Map<EquitySubclass, number>();
  for (const h of holdings) {
    if (!isEquityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const subclass = classifyEquitySubclass(h);
    map.set(subclass, (map.get(subclass) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([label, value]) => ({ label, value }));
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Per-region bar row — top-level geographic exposure. */
export interface RegionRow {
  region: string;
  exposureBase: number;
}

/**
 * Roll equity holdings into per-region exposure rows.
 *
 * Each holding's `instrument.classifications.regions` is a weighted
 * list (each entry has a `weight` in 0..100). We split the holding's
 * base value across its declared top-level regions in proportion to
 * those weights. Holdings without region classification fall into the
 * `'Unspecified'` bucket so the user can see the gap.
 *
 * Sorted descending by `exposureBase`.
 */
export function rollupByRegion(holdings: readonly Holding[]): RegionRow[] {
  const map = new Map<string, number>();
  for (const h of holdings) {
    if (!isEquityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;

    const regions = h.instrument?.classifications?.regions ?? [];
    if (regions.length === 0) {
      map.set("Unspecified", (map.get("Unspecified") ?? 0) + value);
      continue;
    }

    // Sum of declared weights — guards against weights that don't add
    // to 100. We re-normalise so the slice always sums to `value`
    // (rather than overstating exposure when weights total >100).
    const totalWeight = regions.reduce((acc, r) => acc + (r.weight ?? 0), 0);
    if (totalWeight <= 0) {
      map.set("Unspecified", (map.get("Unspecified") ?? 0) + value);
      continue;
    }

    for (const region of regions) {
      const weight = region.weight ?? 0;
      if (weight <= 0) continue;
      const share = (weight / totalWeight) * value;
      const label =
        region.topLevelCategory?.name ?? region.category?.name ?? "Unspecified";
      map.set(label, (map.get(label) ?? 0) + share);
    }
  }
  const rows = Array.from(map.entries()).map(([region, exposureBase]) => ({
    region,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/** Sum of all equity exposure in base currency. */
export function totalEquityExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isEquityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
