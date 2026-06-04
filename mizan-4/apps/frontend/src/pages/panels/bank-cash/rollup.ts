/**
 * Bank & Cash panel rollup — Track B PR-B3 / Goal v3 §V Phase 5.
 *
 * Per Goal v3 §V Phase 5 + ADR 0021 §"Bank / Cash":
 *   - Donut by country (toggleable to by-currency)
 *   - Per-account cards with last-synced timestamp
 *
 * §23 reference user has 12 cash accounts across SG / IN / KSA /
 * UAE — physical cash plus DBS SG, HSBC SG, Stan Chart SG, OCBC,
 * Wise, DBS IN, Axis, ICICI, SAB KSA, Stan Chart UAE, HSBC UAE.
 *
 * Predicate: `holdingType === 'cash'` (lowercase per
 * `HoldingType.CASH` in `lib/constants.ts`).
 */
import type { Holding } from "@/lib/types";

/** Predicate matching cash-classified holdings. */
export function isCashHolding(h: Holding): boolean {
  const ht = (h.holdingType ?? "").toString().toLowerCase();
  return ht === "cash";
}

/** Per-currency exposure row for the donut. */
export interface CurrencyRow {
  currency: string;
  exposureBase: number;
}

/**
 * Roll cash holdings into per-currency exposure rows, descending by
 * exposure. Uses `instrument.currency` first, then falls back to
 * `localCurrency`, then to "UNKNOWN" so the user can see the gap.
 */
export function rollupByCurrency(holdings: readonly Holding[]): CurrencyRow[] {
  const map = new Map<string, number>();
  for (const h of holdings) {
    if (!isCashHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const currencyRaw =
      h.instrument?.currency ?? h.localCurrency ?? h.baseCurrency ?? "";
    const currency = currencyRaw.trim().length > 0 ? currencyRaw.toUpperCase() : "UNKNOWN";
    map.set(currency, (map.get(currency) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([currency, exposureBase]) => ({
    currency,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/**
 * Per-country exposure row for the donut. Country is inferred from
 * the first declared top-level region on the instrument's
 * `classifications.regions`; falls back to `'Unspecified'`.
 *
 * For weighted multi-country cash positions (rare for cash, but
 * possible for global cash funds), we split the holding across its
 * declared regions in proportion to weights, same approach as the
 * Equities panel `rollupByRegion`.
 */
export interface CountryRow {
  country: string;
  exposureBase: number;
}

export function rollupByCountry(holdings: readonly Holding[]): CountryRow[] {
  const map = new Map<string, number>();
  for (const h of holdings) {
    if (!isCashHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const regions = h.instrument?.classifications?.regions ?? [];
    if (regions.length === 0) {
      map.set("Unspecified", (map.get("Unspecified") ?? 0) + value);
      continue;
    }
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
  const rows = Array.from(map.entries()).map(([country, exposureBase]) => ({
    country,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/** Sum of all cash exposure in base currency. */
export function totalCashExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isCashHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
