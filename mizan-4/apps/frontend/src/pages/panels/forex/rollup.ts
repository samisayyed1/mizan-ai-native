/**
 * Forex panel rollup — Track B PR-B16 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Forex": bar chart per pair (one bar per held FX
 * position). Histogram per-pair lands in PR-B16.a alongside the FX
 * history hydration shipped in PR-C5.d.4 (#114).
 *
 * Predicate matches two signals:
 *   1. `assetKind === 'FX'` (the canonical Mizan flag from
 *      `constants.ts` AssetKind enum)
 *   2. `instrument.classifications.assetType.key === 'FX'`
 *      (taxonomy-based fallback, matches the desktop predicate
 *      convention in scheduler.rs PR-C5.d.4)
 */
import type { Holding } from "@/lib/types";

/** Predicate matching FX-classified holdings. */
export function isFxHolding(h: Holding): boolean {
  if (h.assetKind === "FX") return true;
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  return key === "FX" || key === "CURRENCY" || key === "FOREX";
}

/** Per-pair bar row. */
export interface PairBarRow {
  /** Display label — typically `BASE/QUOTE` (e.g. "USD/INR"). */
  label: string;
  /** Symbol for tap-row navigation (instrument.symbol). */
  symbol: string;
  /** Holding id for tap-row navigation. */
  holdingId: string;
  /** Exposure in base currency. */
  exposureBase: number;
}

/**
 * Roll FX holdings into per-pair bar rows, descending by exposure.
 * Each holding is its own bar (FX positions don't aggregate by
 * pair — two USD/INR positions in different accounts stay distinct,
 * matching the Real Estate per-property convention).
 *
 * Display label prefers `instrument.name` (typically "USD/INR" or
 * similar formatted form), falling back to `instrument.symbol`
 * (e.g. "USDINR=X" from Yahoo or "USD/INR" from a Twelve Data
 * feed), then the holding id.
 */
export function rollupByPair(holdings: readonly Holding[]): PairBarRow[] {
  const rows: PairBarRow[] = [];
  for (const h of holdings) {
    if (!isFxHolding(h)) continue;
    const exposureBase = h.marketValue?.base ?? 0;
    if (!Number.isFinite(exposureBase) || exposureBase <= 0) continue;
    const symbol = h.instrument?.symbol ?? h.id;
    const label = h.instrument?.name ?? symbol;
    rows.push({
      label,
      symbol,
      holdingId: h.id,
      exposureBase,
    });
  }
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/**
 * Roll FX holdings into per-base-currency exposure (the "from"
 * leg of each pair). For a USD/INR position, the user is long USD
 * vs INR — this aggregates by the long leg. Per the §23 reasoning,
 * concentration in a single foreign currency matters for the
 * FxMovedMaterially rule (PR-C5.a #108).
 *
 * Extracts the base currency from `instrument.symbol` via common
 * pair formats: `USD/INR`, `USDINR`, `USD-INR`, `USDINR=X`. Falls
 * back to `'Unknown'` when no format matches.
 */
export interface CurrencyExposureRow {
  currency: string;
  exposureBase: number;
}

export function rollupByLongLeg(holdings: readonly Holding[]): CurrencyExposureRow[] {
  const map = new Map<string, number>();
  for (const h of holdings) {
    if (!isFxHolding(h)) continue;
    const exposureBase = h.marketValue?.base ?? 0;
    if (!Number.isFinite(exposureBase) || exposureBase <= 0) continue;
    const symbol = h.instrument?.symbol ?? "";
    const currency = extractBaseCurrency(symbol);
    map.set(currency, (map.get(currency) ?? 0) + exposureBase);
  }
  const rows = Array.from(map.entries()).map(([currency, exposureBase]) => ({
    currency,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/**
 * Extract the base (long) currency from an FX pair symbol.
 * Returns `'Unknown'` when no format matches.
 *
 * Recognised formats (case-insensitive, in priority order):
 *   - `USD/INR` (canonical, slash separator)
 *   - `USD-INR` (dash separator)
 *   - `USDINR=X` (Yahoo Finance format — strip the `=X` suffix)
 *   - `USDINR` (concatenated; assumes 3-char ISO 4217 base + quote)
 */
export function extractBaseCurrency(symbol: string): string {
  const s = symbol.trim().toUpperCase();
  if (s.length === 0) return "Unknown";

  // Slash form: BASE/QUOTE — both sides must be 3-char ISO 4217.
  if (s.includes("/")) {
    const [left, right] = s.split("/", 2);
    if (left && right && left.length === 3 && right.length === 3) {
      return left;
    }
    return "Unknown";
  }
  // Dash form: BASE-QUOTE — same 3+3 constraint.
  if (s.includes("-")) {
    const [left, right] = s.split("-", 2);
    if (left && right && left.length === 3 && right.length === 3) {
      return left;
    }
    return "Unknown";
  }
  // Yahoo =X suffix — strip then treat as concatenated
  const stripped = s.endsWith("=X") ? s.slice(0, -2) : s;
  // Concatenated form: 3-char base + 3-char quote (ISO 4217)
  if (stripped.length === 6) {
    return stripped.slice(0, 3);
  }
  return "Unknown";
}

/** Sum of all FX exposure in base currency. */
export function totalFxExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isFxHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
