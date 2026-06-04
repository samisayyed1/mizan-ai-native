/**
 * Real Estate panel rollup — Track B PR-B11 / Goal v3 §V Phase 5.
 *
 * Per Goal v3 §V Phase 5 step B6 + ADR 0021 §"Real Estate":
 *   - Bar chart per property (stacked equity over mortgage — full
 *     stacked view lands in PR-B11.a once the mortgage-link wire
 *     ships; this PR ships the equity bar)
 *   - AI-estimated badge (renders when `ai_estimated` is set on
 *     holdings_metadata — surfaces via the existing badge system,
 *     not in this PR)
 *
 * §23 reference user's real estate:
 *   - Singapore Bukit Batok primary residence (Not Zakatable)
 *   - 3 Hyderabad rental units (Zakatable on net rental income)
 *   - 1 Hyderabad held-for-sale (Zakatable on market value with
 *     `'ai-estimated'` badge linking to DLD comparables)
 *
 * The intent distinction matters for Zakat:
 *   - residence/primary → Not Zakatable
 *   - rental → Zakatable on net rental income through Hawl
 *   - held-for-sale (mapped as "land" + an explicit "for_sale"
 *     subtype OR "commercial" depending on the user's metadata) →
 *     Zakatable on market value
 *
 * Predicate: `assetKind === 'PROPERTY'`.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching real-estate holdings. */
export function isRealEstateHolding(h: Holding): boolean {
  return h.assetKind === "PROPERTY";
}

/** Property-intent buckets the user can filter by. */
export type PropertyIntent = "Residence" | "Rental" | "Land" | "Commercial" | "Other";

function extractPropertyIntent(h: Holding): PropertyIntent {
  // Property intent lives on Asset.metadata (frontend types don't
  // expose `metadata` on Holding's Instrument directly — same
  // `unknown` cast pattern as `extractMaturityYear` in the Sukuks
  // panel). PR-B11.a will thread asset metadata through cleanly.
  const inst = h.instrument as unknown as
    | { metadata?: { property?: { propertyType?: string } } }
    | null
    | undefined;
  const ptRaw = inst?.metadata?.property?.propertyType;
  if (!ptRaw) return "Other";
  switch (ptRaw.toLowerCase()) {
    case "residence":
      return "Residence";
    case "rental":
      return "Rental";
    case "land":
      return "Land";
    case "commercial":
      return "Commercial";
    default:
      return "Other";
  }
}

/** Per-property bar row (one per holding). */
export interface PropertyBarRow {
  /** Display label — instrument.name → instrument.symbol → holding.id. */
  label: string;
  /** Market value in base currency. */
  value: number;
  /** Property intent — drives the §23 Zakat narrative + future badge. */
  intent: PropertyIntent;
  /** Stable holding id for tap-row navigation. */
  holdingId: string;
}

/**
 * Roll real-estate holdings into per-property bar rows, descending
 * by `value`. Each holding is its own bar (vs the Equities panel
 * which aggregates by symbol) — real estate is per-property by
 * nature.
 */
export function rollupByProperty(holdings: readonly Holding[]): PropertyBarRow[] {
  const rows: PropertyBarRow[] = [];
  for (const h of holdings) {
    if (!isRealEstateHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const label =
      h.instrument?.name ?? h.instrument?.symbol ?? h.id;
    rows.push({
      label,
      value,
      intent: extractPropertyIntent(h),
      holdingId: h.id,
    });
  }
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Per-intent donut row. */
export interface IntentRow {
  intent: PropertyIntent;
  value: number;
}

/**
 * Roll real-estate holdings into per-intent donut rows, descending
 * by `value`. Surfaces the §23-relevant Residence / Rental / Land /
 * Commercial split that drives the Zakat distinction.
 */
export function rollupByIntent(holdings: readonly Holding[]): IntentRow[] {
  const map = new Map<PropertyIntent, number>();
  for (const h of holdings) {
    if (!isRealEstateHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const intent = extractPropertyIntent(h);
    map.set(intent, (map.get(intent) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([intent, value]) => ({ intent, value }));
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Sum of all real-estate exposure in base currency. */
export function totalRealEstateExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isRealEstateHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
