/**
 * Sukuks panel rollup — Track B PR-B1 / Goal v3 §V Phase 5.
 *
 * Pure-math helpers that take the user's full holdings list and emit
 * the two §23 bar-chart views:
 *   - **by issuer** (concentration view): one bar per issuer symbol,
 *     bar value = sum of `market_value.base` across that issuer's
 *     holdings, sorted descending by exposure.
 *   - **by maturity year** (laddering view): one bar per calendar
 *     year extracted from `bond.maturityDate` in the holding's
 *     `instrument.metadata`, bar value = sum of principal returning
 *     that year, sorted ascending by year.
 *
 * Both helpers consume the same `instrument.classifications.assetType.key`
 * predicate as the bond-maturity hydrate (PR-C5.d.3 #113) — the
 * scheduler-side filter and the UI-side filter must agree.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching the §23 sukuks/bonds set. Mirrors `is_bond_holding`
 * in `mizan-4/apps/tauri/src/scheduler.rs` (PR-C5.d.3 #113). */
export function isBondHolding(h: Holding): boolean {
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  return (
    key.startsWith("BOND_") ||
    key === "DEBT_SECURITY" ||
    key === "MONEY_MARKET_DEBT" ||
    key === "SUKUK"
  );
}

/** Holding-derived row for the by-issuer bar chart. */
export interface IssuerExposureRow {
  issuer: string;
  exposureBase: number;
}

/** Holding-derived row for the by-maturity bar chart. */
export interface MaturityYearRow {
  /** ISO year as a 4-digit number (e.g. 2027). */
  year: number;
  /** Sum of principal returning in this year, base currency. */
  exposureBase: number;
}

/**
 * Roll bond/sukuk holdings into per-issuer exposure rows, descending
 * by `exposureBase`. Display label uses `instrument.name` if set, else
 * `instrument.symbol` (matches the bond-maturity hydrate pattern).
 */
export function rollupByIssuer(holdings: readonly Holding[]): IssuerExposureRow[] {
  const map = new Map<string, { label: string; total: number }>();
  for (const h of holdings) {
    if (!isBondHolding(h)) continue;
    const inst = h.instrument;
    if (!inst) continue;
    const base = h.marketValue?.base ?? 0;
    if (!Number.isFinite(base) || base <= 0) continue;
    const label = inst.name && inst.name.length > 0 ? inst.name : inst.symbol;
    const key = inst.symbol;
    const cur = map.get(key);
    if (cur) {
      cur.total += base;
    } else {
      map.set(key, { label, total: base });
    }
  }
  const rows = Array.from(map.values()).map((v) => ({
    issuer: v.label,
    exposureBase: v.total,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/**
 * Extract the maturity year (Gregorian calendar year) from a holding's
 * `instrument.metadata.bond.maturityDate`. Returns `null` if missing,
 * not parseable, or not a positive 4-digit year.
 *
 * The metadata shape mirrors the desktop `Asset.bond_spec()` JSON
 * extractor in `mizan-core::assets::Asset::bond_spec` — the same
 * data the bond-maturity scheduler hydrate reads.
 */
export function extractMaturityYear(h: Holding): number | null {
  // The frontend `Instrument` type doesn't expose `metadata` directly
  // (that lives on the desktop `Asset` model). For PR-B1 we read via
  // an `unknown` cast — once the frontend types thread bond metadata
  // through (PR-B1.a), this helper switches to a typed accessor.
  const inst = h.instrument as unknown as
    | { metadata?: { bond?: { maturityDate?: string } } }
    | null
    | undefined;
  const dateStr = inst?.metadata?.bond?.maturityDate;
  if (!dateStr) return null;
  // Accept ISO 8601 "YYYY-MM-DD" or full RFC 3339 timestamps. Take
  // first 4 chars and validate.
  const yearStr = dateStr.slice(0, 4);
  const year = Number.parseInt(yearStr, 10);
  if (!Number.isFinite(year) || year < 1900 || year > 9999) return null;
  return year;
}

/**
 * Roll bond/sukuk holdings into per-maturity-year exposure rows,
 * ascending by year. Holdings without a parseable maturity year are
 * silently skipped — the by-issuer view still surfaces them.
 */
export function rollupByMaturityYear(
  holdings: readonly Holding[],
): MaturityYearRow[] {
  const map = new Map<number, number>();
  for (const h of holdings) {
    if (!isBondHolding(h)) continue;
    const year = extractMaturityYear(h);
    if (year === null) continue;
    const base = h.marketValue?.base ?? 0;
    if (!Number.isFinite(base) || base <= 0) continue;
    map.set(year, (map.get(year) ?? 0) + base);
  }
  const rows = Array.from(map.entries()).map(([year, exposureBase]) => ({
    year,
    exposureBase,
  }));
  rows.sort((a, b) => a.year - b.year);
  return rows;
}

/** Sum of all bond/sukuk exposure in base currency. */
export function totalBondExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isBondHolding(h)) continue;
    const base = h.marketValue?.base ?? 0;
    if (Number.isFinite(base) && base > 0) sum += base;
  }
  return sum;
}
