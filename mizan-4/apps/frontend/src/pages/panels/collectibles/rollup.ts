/**
 * Collectibles panel rollup — Track B PR-B15 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Collectibles": bar chart per item + category
 * breakdown. AI estimation pipeline (Chrono24 / WatchCharts /
 * Hagerty / StockX) ships separately with PR-B15.b. Today's panel
 * reads what the user has manually entered with metadata.
 *
 * Predicate: `assetKind === 'COLLECTIBLE'` (the Mizan AssetKind
 * enum value). Categories come from `CollectibleMetadata.collectibleType`
 * (art / wine / watch / jewelry / memorabilia) per the existing
 * frontend type definition.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching collectible holdings. */
export function isCollectibleHolding(h: Holding): boolean {
  return h.assetKind === "COLLECTIBLE";
}

/** Collectible category buckets the donut groups holdings into. */
export type CollectibleCategory =
  | "Watches"
  | "Art"
  | "Jewelry"
  | "Wine"
  | "Memorabilia"
  | "Other";

function extractCollectibleCategory(h: Holding): CollectibleCategory {
  // Same `unknown` cast pattern as Real Estate / Sukuks bond
  // metadata. The frontend Instrument type doesn't expose
  // `metadata` directly; PR-B1.a will thread asset metadata
  // through cleanly.
  const inst = h.instrument as unknown as
    | { metadata?: { collectible?: { collectibleType?: string } } }
    | null
    | undefined;
  const raw = inst?.metadata?.collectible?.collectibleType;
  if (!raw) return "Other";
  switch (raw.toLowerCase()) {
    case "watch":
      return "Watches";
    case "art":
      return "Art";
    case "jewelry":
      return "Jewelry";
    case "wine":
      return "Wine";
    case "memorabilia":
      return "Memorabilia";
    default:
      return "Other";
  }
}

/** Per-item bar row (one per holding). */
export interface ItemBarRow {
  /** Display label — instrument.name → instrument.symbol → holding.id. */
  label: string;
  /** Market value in base currency. */
  value: number;
  /** Category drives the per-row chip + helps the user spot mis-classifications. */
  category: CollectibleCategory;
  /** Stable holding id for tap-row navigation. */
  holdingId: string;
}

/**
 * Roll collectible holdings into per-item bar rows, descending
 * by `value`. Each holding is its own bar (collectibles don't
 * aggregate — same brand isn't the same item, mirroring the Real
 * Estate per-property convention).
 */
export function rollupByItem(holdings: readonly Holding[]): ItemBarRow[] {
  const rows: ItemBarRow[] = [];
  for (const h of holdings) {
    if (!isCollectibleHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const label = h.instrument?.name ?? h.instrument?.symbol ?? h.id;
    rows.push({
      label,
      value,
      category: extractCollectibleCategory(h),
      holdingId: h.id,
    });
  }
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Per-category donut row. */
export interface CategoryRow {
  category: CollectibleCategory;
  value: number;
}

/**
 * Roll collectible holdings into per-category donut rows,
 * descending by value.
 */
export function rollupByCategory(holdings: readonly Holding[]): CategoryRow[] {
  const map = new Map<CollectibleCategory, number>();
  for (const h of holdings) {
    if (!isCollectibleHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const category = extractCollectibleCategory(h);
    map.set(category, (map.get(category) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([category, value]) => ({ category, value }));
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Sum of all collectible exposure in base currency. */
export function totalCollectiblesExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isCollectibleHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
