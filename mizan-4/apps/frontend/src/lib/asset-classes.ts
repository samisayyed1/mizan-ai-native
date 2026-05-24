/**
 * Asset class taxonomy — Feroz May-17 product direction.
 *
 * A "Portfolio" (formerly "Account") is a holding company. It does NOT
 * contain holdings directly. It contains ASSET CLASSES. Each asset
 * class then contains the holdings.
 *
 * This file is the single source of truth for:
 *  - the user-facing asset class enum (Feroz's named categories)
 *  - the display label + icon for each class
 *  - the stable display order on the Portfolio detail page
 *  - the classifier that maps a Holding from the existing data model
 *    into one of these classes
 *  - a grouping helper that bundles holdings + per-class totals
 *
 * No UI imports — pure data layer. Consumed by `asset-classes-view.tsx`
 * and unit-tested in `asset-classes.test.ts`.
 *
 * Reference: .claude/product-notes/feroz-meeting-2026-05-17.md
 * (decisions 5, 9, 10, 17 — portfolio-as-container, asset-class list,
 *  bank-accounts-as-asset-class).
 */

import { AssetKind, HoldingType } from "@/lib/constants";
import type { Holding } from "@/lib/types";

// ---------------------------------------------------------------------------
// Asset class enum + display metadata
// ---------------------------------------------------------------------------

/**
 * The user-facing asset class buckets Feroz enumerated on the May-17 call.
 * Stable string values — these may end up persisted in URL params and
 * settings, so do NOT reorder or rename keys casually.
 */
export const AssetClass = {
  STOCKS: "STOCKS",
  SUKUKS: "SUKUKS",
  ETFS: "ETFS",
  BONDS: "BONDS",
  BANK_ACCOUNTS: "BANK_ACCOUNTS",
  PROPERTY: "PROPERTY",
  COLLECTIBLES: "COLLECTIBLES",
  PRECIOUS_METALS: "PRECIOUS_METALS",
  OTHER: "OTHER",
} as const;

export type AssetClass = (typeof AssetClass)[keyof typeof AssetClass];

/**
 * Singular + plural display labels per class. Singular forms are used
 * in empty-state copy ("Add Sukuk") and CTA buttons; plurals are used
 * for card titles and counts ("3 Stocks").
 */
export const ASSET_CLASS_LABELS: Record<AssetClass, { singular: string; plural: string }> = {
  STOCKS: { singular: "Stock", plural: "Stocks" },
  SUKUKS: { singular: "Sukuk", plural: "Sukuks" },
  ETFS: { singular: "ETF", plural: "ETFs" },
  BONDS: { singular: "Bond", plural: "Bonds" },
  BANK_ACCOUNTS: { singular: "Bank Account", plural: "Bank Accounts" },
  PROPERTY: { singular: "Property", plural: "Property" },
  COLLECTIBLES: { singular: "Collectible", plural: "Collectibles" },
  PRECIOUS_METALS: { singular: "Precious Metal", plural: "Precious Metals" },
  OTHER: { singular: "Other Asset", plural: "Other Assets" },
};

/**
 * Icon name per class — keyed to the `Icons` registry in @mizan/ui. The
 * UI looks these up dynamically so a new class only needs an icon
 * addition here, not a UI change.
 */
export const ASSET_CLASS_ICON_NAMES: Record<AssetClass, string> = {
  STOCKS: "TrendingUp",
  SUKUKS: "BadgeDollarSign",
  ETFS: "PieChart",
  BONDS: "FileText",
  BANK_ACCOUNTS: "Wallet",
  PROPERTY: "Home",
  COLLECTIBLES: "Gem",
  PRECIOUS_METALS: "Coins",
  OTHER: "Ellipsis",
};

/**
 * A stable accent color per asset class (Feroz: "every asset class a
 * different colored graph"). Each class maps to a distinct Flexoki hue
 * defined as a theme token in `globals.css` (`--asset-*`), so the colors
 * are genuinely different yet stay inside the app's paper palette and
 * adapt to light/dark. Consumed by the per-class history chart and the
 * asset-class card weight bar so the two always agree.
 */
const ASSET_CLASS_TOKEN: Record<AssetClass, string> = {
  STOCKS: "var(--asset-stocks)",
  SUKUKS: "var(--asset-sukuks)",
  ETFS: "var(--asset-etfs)",
  BONDS: "var(--asset-bonds)",
  BANK_ACCOUNTS: "var(--asset-bank)",
  PROPERTY: "var(--asset-property)",
  COLLECTIBLES: "var(--asset-collectibles)",
  PRECIOUS_METALS: "var(--asset-metals)",
  OTHER: "var(--asset-other)",
};

export function assetClassColor(cls: AssetClass): string {
  return ASSET_CLASS_TOKEN[cls] ?? "var(--asset-other)";
}

/**
 * Stable display order on the Portfolio detail page. Matches the order
 * Feroz enumerated on the call. New classes append at the end (before
 * OTHER) — never insert in the middle.
 */
export const ASSET_CLASS_ORDER: readonly AssetClass[] = [
  AssetClass.STOCKS,
  AssetClass.SUKUKS,
  AssetClass.ETFS,
  AssetClass.BONDS,
  AssetClass.BANK_ACCOUNTS,
  AssetClass.PROPERTY,
  AssetClass.COLLECTIBLES,
  AssetClass.PRECIOUS_METALS,
  AssetClass.OTHER,
];

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

/**
 * Set of taxonomy keys/names that route an INVESTMENT-kind holding into
 * a specific Feroz class. We match on lower-cased substrings so the
 * classifier survives small variations across taxonomy seeds
 * ("stock" vs "stocks" vs "common stock", etc.).
 *
 * Order matters: ETFs / Sukuks are checked BEFORE plain "bond" / "stock"
 * because an ETF could be a "bond etf" and would otherwise mis-route.
 */
const INVESTMENT_KEYWORD_RULES: readonly {
  keywords: readonly string[];
  cls: AssetClass;
}[] = [
  { keywords: ["sukuk"], cls: AssetClass.SUKUKS },
  { keywords: ["etf", "exchange-traded fund", "exchange traded fund"], cls: AssetClass.ETFS },
  {
    keywords: ["bond", "fixed income", "treasury", "gilt"],
    cls: AssetClass.BONDS,
  },
  {
    keywords: ["stock", "equity", "equities", "share", "common"],
    cls: AssetClass.STOCKS,
  },
];

/**
 * Map a Holding into one of Feroz's asset classes.
 *
 * The classifier uses three signals, in priority order:
 *   1. `holdingType` — cash always routes to Bank Accounts.
 *   2. `assetKind` — alternative kinds (property, collectible, …) map
 *      one-to-one to their named class.
 *   3. `instrument.classifications` taxonomy — for INVESTMENT-kind
 *      holdings we split stocks / ETFs / bonds / sukuks by keyword.
 *
 * The fallback for a securities-kind holding with no useful taxonomy is
 * STOCKS, because the overwhelming majority of broker-imported holdings
 * are common stock. OTHER is reserved for genuinely unclassifiable
 * cases (FX, LIABILITY, unknown).
 *
 * Vehicles and Liabilities are deliberately NOT first-class buckets:
 *  - Vehicles will be excluded from net worth entirely (Feroz #14).
 *  - Liabilities get their own dedicated section, not a portfolio
 *    asset class (Feroz #20). Both currently fall into OTHER here
 *    so the UI degrades gracefully rather than crashing on legacy data.
 */
export function classifyHolding(holding: Holding): AssetClass {
  if (holding.holdingType === HoldingType.CASH) {
    return AssetClass.BANK_ACCOUNTS;
  }

  switch (holding.assetKind) {
    case AssetKind.PROPERTY:
      return AssetClass.PROPERTY;
    case AssetKind.COLLECTIBLE:
      return AssetClass.COLLECTIBLES;
    case AssetKind.PRECIOUS_METAL:
      return AssetClass.PRECIOUS_METALS;
    case AssetKind.VEHICLE:
    case AssetKind.LIABILITY:
    case AssetKind.FX:
    case AssetKind.OTHER:
      return AssetClass.OTHER;
  }

  const classifications = holding.instrument?.classifications;
  const candidates: string[] = [];
  if (classifications?.assetType?.name) candidates.push(classifications.assetType.name);
  if (classifications?.assetType?.key) candidates.push(classifications.assetType.key);
  for (const cw of classifications?.assetClasses ?? []) {
    if (cw.category?.name) candidates.push(cw.category.name);
    if (cw.category?.key) candidates.push(cw.category.key);
    if (cw.topLevelCategory?.name) candidates.push(cw.topLevelCategory.name);
  }
  // Normalize hyphens / underscores to spaces so taxonomy seeds like
  // "fixed-income" or "exchange_traded_fund" still hit the keyword rules
  // below (which use space-separated phrases).
  const haystack = candidates.join(" ").toLowerCase().replace(/[-_]+/g, " ");

  if (haystack) {
    for (const rule of INVESTMENT_KEYWORD_RULES) {
      if (rule.keywords.some((kw) => haystack.includes(kw))) {
        return rule.cls;
      }
    }
  }

  // INVESTMENT / PRIVATE_EQUITY with no taxonomy signal: assume stocks.
  if (
    holding.assetKind === AssetKind.INVESTMENT ||
    holding.assetKind === AssetKind.PRIVATE_EQUITY
  ) {
    return AssetClass.STOCKS;
  }

  return AssetClass.OTHER;
}

// ---------------------------------------------------------------------------
// Grouping helper
// ---------------------------------------------------------------------------

/**
 * Per-class summary for the Portfolio detail page cards.
 *
 * `totalValue` is the sum of holding market values in the portfolio's
 * base currency.
 *
 * `totalGain` is the sum of `holding.totalGain.base` across the bucket.
 * `null` if every holding's totalGain was null/undefined — we keep the
 * distinction "no data" vs "exactly zero" so the UI can hide the gain
 * indicator entirely instead of showing a misleading $0.00.
 *
 * `weightPercent` is this bucket's share of the *portfolio total*, in
 * the range 0..100. `0` when the portfolio total is zero (so no
 * NaN/Infinity ever reaches the UI).
 */
export interface AssetClassBucket {
  cls: AssetClass;
  holdings: readonly Holding[];
  totalValue: number;
  count: number;
  totalGain: number | null;
  weightPercent: number;
}

/**
 * Group a flat holdings list into one bucket per asset class.
 *
 * Returns ALL classes in `ASSET_CLASS_ORDER`, even those with zero
 * holdings — Feroz wants empty buckets visible on the portfolio page
 * so the user can click an empty class and get the "Add Sukuk" CTA.
 * Callers that only want non-empty buckets can use `partitionBuckets`.
 *
 * Holdings inside each bucket retain their input order. We don't sort
 * by value here — the caller may want a different ordering depending
 * on the surface (largest-first inside the drill-down, alphabetical
 * inside a picker, etc.).
 *
 * Each bucket carries derived totals (`totalValue`, `totalGain`,
 * `weightPercent`) so the UI never has to recompute them per render.
 * `totalGain` follows a deliberate null-vs-zero contract — see the
 * `AssetClassBucket` doc above.
 */
export function groupHoldingsByAssetClass(holdings: readonly Holding[]): AssetClassBucket[] {
  const byClass = new Map<AssetClass, Holding[]>();
  for (const cls of ASSET_CLASS_ORDER) byClass.set(cls, []);

  for (const h of holdings) {
    const cls = classifyHolding(h);
    const bucket = byClass.get(cls);
    if (bucket) bucket.push(h);
  }

  // First pass — compute totalValue per class and the portfolio total.
  // We need the portfolio total before we can derive each bucket's
  // weight, so this is unavoidably two-pass.
  const interim = ASSET_CLASS_ORDER.map((cls) => {
    const bucketHoldings = byClass.get(cls) ?? [];
    const totalValue = bucketHoldings.reduce((acc, h) => acc + (h.marketValue?.base ?? 0), 0);
    const { totalGain } = sumBucketGain(bucketHoldings);
    return { cls, bucketHoldings, totalValue, totalGain };
  });

  const portfolioTotal = interim.reduce((acc, b) => acc + b.totalValue, 0);

  return interim.map(({ cls, bucketHoldings, totalValue, totalGain }) => ({
    cls,
    holdings: bucketHoldings,
    totalValue,
    count: bucketHoldings.length,
    totalGain,
    weightPercent: portfolioTotal > 0 ? (totalValue / portfolioTotal) * 100 : 0,
  }));
}

/**
 * Sum `holding.totalGain.base` across a bucket, preserving the
 * null-vs-zero distinction described on AssetClassBucket.
 *
 * Returns `null` when every holding in the bucket has a null or
 * undefined `totalGain` — the bucket has no gain data, the UI should
 * hide the gain indicator. Returns a number (including 0) when at
 * least one holding contributes data.
 *
 * Exported for tests; callers usually consume the field on
 * AssetClassBucket directly.
 */
export function sumBucketGain(holdings: readonly Holding[]): { totalGain: number | null } {
  let sum = 0;
  let anyData = false;
  for (const h of holdings) {
    const v = h.totalGain?.base;
    if (typeof v === "number" && Number.isFinite(v)) {
      sum += v;
      anyData = true;
    }
  }
  return { totalGain: anyData ? sum : null };
}

/**
 * Partition a list of buckets into the ones that have holdings
 * ("active") and the ones that don't ("empty"). Used by the Portfolio
 * detail page to render an "Active" grid prominently and an "Add
 * another asset class" rail underneath for the empty classes — both
 * always visible per Feroz, but visually weighted by relevance.
 *
 * Order of buckets is preserved within each partition.
 */
export function partitionBuckets(buckets: readonly AssetClassBucket[]): {
  active: AssetClassBucket[];
  empty: AssetClassBucket[];
} {
  const active: AssetClassBucket[] = [];
  const empty: AssetClassBucket[] = [];
  for (const b of buckets) {
    if (b.count > 0) active.push(b);
    else empty.push(b);
  }
  return { active, empty };
}

/**
 * Convenience: case-insensitive lookup of an AssetClass by URL param
 * value. Returns null if the param doesn't match a known class, so the
 * UI can treat that as "no class selected" rather than crashing.
 */
export function parseAssetClassParam(raw: string | null | undefined): AssetClass | null {
  if (!raw) return null;
  const upper = raw.toUpperCase();
  if ((Object.values(AssetClass) as string[]).includes(upper)) {
    return upper as AssetClass;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Asset-class history scaling (Step 8 — per-class graphs)
// ---------------------------------------------------------------------------

/**
 * Minimal row shape we need from a portfolio-level valuation history
 * to derive a class-level estimate. Matches `AccountValuation` from
 * `lib/types.ts` but kept structural so the helper stays pure and the
 * tests don't need to construct full DTOs.
 */
export interface ValuationHistoryRow {
  valuationDate: string;
  totalValue: number;
  netContribution: number;
  baseCurrency?: string | null;
}

/**
 * One point of the per-class history time-series fed to `<HistoryChart>`.
 * Same shape the dashboard's chart already consumes — drop-in.
 */
export interface AssetClassHistoryPoint {
  date: string;
  totalValue: number;
  netContribution: number;
  currency: string;
}

/**
 * Project a portfolio-level valuation history onto a single asset
 * class by **scaling every point by the class's current weight**.
 *
 * This is the constant-weight approximation: it assumes the class
 * held its current share of the portfolio throughout the period.
 * Reality drifts — rebalances, new contributions to one class, sells
 * out of another — but the SHAPE of the resulting curve still tells
 * the user "how did this class do" relative to portfolio context.
 *
 * The caveat is communicated to the user with a visible "Estimated"
 * disclaimer on the chart. A future iteration will replace this with
 * a real per-class history once the backend exposes one.
 *
 * Math invariants enforced here:
 *  - `weightPercent` is clamped to 0..100 (defensive against caller
 *    bugs or NaN that slipped through).
 *  - Rows with non-finite `totalValue` or `netContribution` are
 *    skipped entirely rather than poisoning the chart.
 *  - Dates pass through unchanged so the x-axis aligns with the
 *    portfolio-level chart.
 *  - Currency falls back to `defaultCurrency` when the row didn't
 *    carry one — `<HistoryChart>` needs a non-empty string.
 *
 * Pure function — exported and unit-tested in `asset-classes.test.ts`.
 */
export function scaleHistoryByWeight(
  history: readonly ValuationHistoryRow[],
  weightPercent: number,
  defaultCurrency: string,
): AssetClassHistoryPoint[] {
  if (!Number.isFinite(weightPercent)) return [];
  const weight = Math.max(0, Math.min(100, weightPercent)) / 100;
  if (weight === 0) {
    // Caller's UI should normally hide the chart entirely in this case,
    // but returning an empty list (rather than rows of zeros) is the
    // unambiguous signal for "we have no signal to project".
    return [];
  }

  const out: AssetClassHistoryPoint[] = [];
  for (const row of history) {
    if (!Number.isFinite(row.totalValue) || !Number.isFinite(row.netContribution)) continue;
    out.push({
      date: row.valuationDate,
      totalValue: row.totalValue * weight,
      netContribution: row.netContribution * weight,
      currency: row.baseCurrency || defaultCurrency,
    });
  }
  return out;
}
