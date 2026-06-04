/**
 * Commodities panel rollup — Track B PR-B10 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Commodities": donut by metal (Gold / Silver /
 * Platinum / Palladium / Other), backed by MetalpriceAPI feed in
 * the heavier B14 cloud-side work. Today's panel reads whatever
 * `assetKind === 'PRECIOUS_METAL'` holdings the user has + maps
 * their symbol to a metal bucket.
 *
 * Physical vs paper distinction lands as a Mizan Badge modifier
 * in a follow-up (ADR 0023 + the `holdings_metadata` table).
 */
import type { Holding } from "@/lib/types";

/** Predicate matching commodities holdings.
 *
 * Two signals — either is sufficient:
 *   1. `assetKind === 'PRECIOUS_METAL'` (the canonical Mizan flag)
 *   2. `instrument.classifications.assetType.key` in
 *      {METAL, PRECIOUS_METAL, COMMODITY} (matches the desktop
 *      bond/equity predicate convention).
 */
export function isCommodityHolding(h: Holding): boolean {
  if (h.assetKind === "PRECIOUS_METAL") return true;
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  return key === "METAL" || key === "PRECIOUS_METAL" || key === "COMMODITY";
}

/** Metal bucket the donut groups holdings into. */
export type MetalBucket = "Gold" | "Silver" | "Platinum" | "Palladium" | "Other";

/**
 * Map an instrument symbol to its metal bucket.
 *
 * Standard COMEX/LBMA symbols + common ETF / OTC variants. Anything
 * unmatched falls to `'Other'`. Case-insensitive.
 */
export function metalForSymbol(symbol: string): MetalBucket {
  const s = symbol.trim().toUpperCase();
  // Gold — spot, futures, common ETFs
  if (s === "XAU" || s === "GOLD" || s === "GC" || s === "GLD" || s === "IAU" || s === "PHYS" || s === "SGOL") {
    return "Gold";
  }
  // Silver
  if (s === "XAG" || s === "SILVER" || s === "SI" || s === "SLV" || s === "SIVR" || s === "PSLV") {
    return "Silver";
  }
  // Platinum
  if (s === "XPT" || s === "PLATINUM" || s === "PL" || s === "PPLT") {
    return "Platinum";
  }
  // Palladium
  if (s === "XPD" || s === "PALLADIUM" || s === "PA" || s === "PALL") {
    return "Palladium";
  }
  return "Other";
}

/** Per-metal donut row. */
export interface MetalRow {
  metal: MetalBucket;
  exposureBase: number;
}

/**
 * Roll commodities holdings into per-metal exposure rows, descending
 * by exposure.
 *
 * Symbol-based classification covers the common case (ETF tickers,
 * COMEX symbols). Holdings without a recognised symbol fall to
 * `'Other'`.
 */
export function rollupByMetal(holdings: readonly Holding[]): MetalRow[] {
  const map = new Map<MetalBucket, number>();
  for (const h of holdings) {
    if (!isCommodityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const symbol = h.instrument?.symbol ?? "";
    const metal = metalForSymbol(symbol);
    map.set(metal, (map.get(metal) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([metal, exposureBase]) => ({
    metal,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/** Sum of all commodities exposure in base currency. */
export function totalCommoditiesExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isCommodityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
