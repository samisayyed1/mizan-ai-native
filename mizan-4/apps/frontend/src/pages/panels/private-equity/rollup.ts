/**
 * Private Equity panel rollup — Track B PR-B7 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Private Equity": bar chart per vintage (year of
 * acquisition) + per-position list. The J-curve projection overlay
 * lands separately as PR-B7.a once the synthesis crate exposes
 * the projection API to the frontend.
 *
 * Predicate: `assetKind === 'PRIVATE_EQUITY'` (from the Mizan
 * AssetKind enum). Vintage extracted from
 * `metadata.privateEquity.purchaseDate` — first 4 chars as year.
 *
 * §23 reference user has Hasan VC + HAPL + Wholesum + Stake PE
 * positions; each has its own vintage year that the bar chart
 * surfaces.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching private-equity holdings. */
export function isPrivateEquityHolding(h: Holding): boolean {
  return h.assetKind === "PRIVATE_EQUITY";
}

/** Extract the vintage year (year of acquisition) from a holding's
 * metadata. Returns `null` if missing, not parseable, or not a
 * positive 4-digit year.
 *
 * Tries `metadata.privateEquity.purchaseDate` first, then falls
 * back to `metadata.purchaseDate` (the generic alternative-asset
 * field). Either path lets the user surface a vintage without
 * forcing them to use a PE-specific metadata shape that doesn't
 * yet exist on the desktop side.
 */
export function extractVintageYear(h: Holding): number | null {
  const inst = h.instrument as unknown as
    | {
        metadata?: {
          privateEquity?: { purchaseDate?: string };
          purchaseDate?: string;
        };
      }
    | null
    | undefined;
  const dateStr =
    inst?.metadata?.privateEquity?.purchaseDate ?? inst?.metadata?.purchaseDate;
  if (!dateStr) return null;
  const yearStr = dateStr.slice(0, 4);
  const year = Number.parseInt(yearStr, 10);
  if (!Number.isFinite(year) || year < 1900 || year > 9999) return null;
  return year;
}

/** Per-vintage bar row for the laddering view. */
export interface VintageRow {
  /** ISO year (e.g. 2022). */
  year: number;
  /** Sum of market value across positions acquired that year, base ccy. */
  exposureBase: number;
}

/**
 * Roll PE holdings into per-vintage rows, ascending by year.
 * Mirrors the Sukuks `rollupByMaturityYear` pattern — both views
 * surface time-banded exposure.
 *
 * Holdings without a parseable vintage are silently skipped; the
 * per-item bar still shows them.
 */
export function rollupByVintage(holdings: readonly Holding[]): VintageRow[] {
  const map = new Map<number, number>();
  for (const h of holdings) {
    if (!isPrivateEquityHolding(h)) continue;
    const year = extractVintageYear(h);
    if (year === null) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    map.set(year, (map.get(year) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([year, exposureBase]) => ({
    year,
    exposureBase,
  }));
  rows.sort((a, b) => a.year - b.year);
  return rows;
}

/** Per-position bar row (one per holding). */
export interface PositionRow {
  /** Display label — instrument.name → instrument.symbol → holding.id. */
  label: string;
  /** Market value in base currency. */
  value: number;
  /** Vintage year if known, else null (chip says "—"). */
  vintage: number | null;
  /** Stable holding id for tap-row navigation. */
  holdingId: string;
}

/**
 * Roll PE holdings into per-position bar rows, descending by value.
 * Each holding is its own bar (mirrors Real Estate + Collectibles
 * per-item convention — same fund vintage isn't the same position).
 */
export function rollupByPosition(holdings: readonly Holding[]): PositionRow[] {
  const rows: PositionRow[] = [];
  for (const h of holdings) {
    if (!isPrivateEquityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const label = h.instrument?.name ?? h.instrument?.symbol ?? h.id;
    rows.push({
      label,
      value,
      vintage: extractVintageYear(h),
      holdingId: h.id,
    });
  }
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Sum of all PE exposure in base currency. */
export function totalPrivateEquityExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isPrivateEquityHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
