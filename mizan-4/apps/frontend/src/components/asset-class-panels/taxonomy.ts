/**
 * Twelve-panel asset class taxonomy — Track A PR-A4.
 *
 * Fixed order per ADR 0021 + Evolution Spec §3(e). The dashboard renders
 * these in this exact sequence; reorder requires ADR amendment.
 *
 * Each panel id is the route slug for the dedicated panel surface that
 * Track B PRs B1..B7 will fill in. Until those land, taps from the
 * skeleton route to the existing holdings page filtered by the holding
 * predicate documented here.
 *
 * The classifier maps a [`Holding`] to its panel id using the existing
 * `assetKind` + `instrument.instrumentType` fields. Holdings that don't
 * match any panel land in the catch-all `'other'` bucket (NOT rendered
 * in the §3(e) twelve; surfaced via the "Other" tile only if non-empty).
 */
import type { Holding } from "@/lib/types";

/**
 * Panel ids — one per dashboard tile, ordered per ADR 0021 §"The 12 panels".
 *
 * `'other'` is the catch-all and intentionally NOT part of the fixed twelve;
 * it's surfaced only when there are unclassified holdings.
 */
export type AssetClassPanelId =
  | "equities"
  | "brokerage-accounts"
  | "bank-cash"
  | "bonds-sukuks"
  | "provident-funds"
  | "insurance"
  | "private-equity"
  | "real-estate"
  | "crypto"
  | "commodities"
  | "collectibles"
  | "forex"
  | "other";

/** Panel descriptor — display name + Lucide icon name + holdings-page query. */
export interface AssetClassPanelDescriptor {
  id: AssetClassPanelId;
  label: string;
  /**
   * Icon key from `@mizan/ui` `Icons` registry. Kept as a string so this
   * file has zero React imports — pure data, easy to test.
   */
  iconKey: string;
  /**
   * Query string to navigate to the existing holdings page filtered for
   * this panel. Replaces the placeholder until the dedicated B-track
   * panel surface ships.
   */
  holdingsHref: string;
}

/**
 * The fixed twelve, in dashboard order. ADR 0021 / Spec §3(e) pins this
 * sequence; reorder requires ADR amendment + visual-regression update.
 */
export const ASSET_CLASS_PANELS: readonly AssetClassPanelDescriptor[] = [
  {
    id: "equities",
    label: "Equities",
    iconKey: "TrendingUp",
    // Track B PR-B2 — dedicated panel page (donut + region bar).
    holdingsHref: "/panels/equities",
  },
  {
    id: "brokerage-accounts",
    label: "Brokerage",
    iconKey: "Briefcase",
    // Track B PR-B8 — dedicated panel page (donut by broker + accounts list).
    // Label shortened from "Brokerage Accounts" → "Brokerage" so it
    // never truncates inside the 72px tile (ADR 0018b).
    holdingsHref: "/panels/brokerage-accounts",
  },
  {
    id: "bank-cash",
    label: "Bank & Cash",
    iconKey: "Wallet",
    // Track B PR-B3 — dedicated panel page (country/currency donut).
    holdingsHref: "/panels/bank-cash",
  },
  {
    id: "bonds-sukuks",
    label: "Bonds & Sukuks",
    iconKey: "Coins",
    // Track B PR-B1 — dedicated panel page (§23 anchor surface).
    // Other panels stay on the holdings-page filter route until
    // PR-B2..B7 ship their dedicated surfaces.
    holdingsHref: "/panels/sukuks",
  },
  {
    id: "provident-funds",
    label: "Provident Funds",
    iconKey: "PiggyBank",
    // Track B PR-B9 — dedicated panel page (scheme donut + CPF sub-donut).
    holdingsHref: "/panels/provident-funds",
  },
  {
    id: "insurance",
    label: "Insurance",
    iconKey: "ShieldCheck",
    // Track B PR-B10 — dedicated panel page (category donut + stale badges).
    holdingsHref: "/panels/insurance",
  },
  {
    id: "private-equity",
    label: "Private Equity",
    iconKey: "Building",
    // Track B PR-B7 — dedicated panel page (vintage bar + per-position bar).
    holdingsHref: "/panels/private-equity",
  },
  {
    id: "real-estate",
    label: "Real Estate",
    iconKey: "Home",
    // Track B PR-B11 — dedicated panel page (intent donut + per-property bar).
    holdingsHref: "/panels/real-estate",
  },
  {
    id: "crypto",
    label: "Crypto",
    iconKey: "Bitcoin",
    // Track B PR-B9 — dedicated panel page (chain donut).
    holdingsHref: "/panels/crypto",
  },
  {
    id: "commodities",
    label: "Commodities",
    iconKey: "Gem",
    // Track B PR-B10 — dedicated panel page (metal donut).
    holdingsHref: "/panels/commodities",
  },
  {
    id: "collectibles",
    label: "Collectibles",
    iconKey: "Sparkles",
    // Track B PR-B15 — dedicated panel page (category donut + per-item bar).
    holdingsHref: "/panels/collectibles",
  },
  {
    id: "forex",
    label: "Forex",
    iconKey: "RefreshCw",
    // Track B PR-B16 — dedicated panel page (per-pair bar + long-leg donut).
    holdingsHref: "/panels/forex",
  },
] as const;

/**
 * Classify a holding into its panel id.
 *
 * Signals (in priority order):
 *  1. `holdingType` — `CASH` short-circuits to `bank-cash`
 *  2. `assetKind` — covers the alternative-asset tracks
 *     (PROPERTY/COLLECTIBLE/PRECIOUS_METAL/PRIVATE_EQUITY/FX/VEHICLE/LIABILITY)
 *  3. `instrument.classifications.assetType.key` — the existing taxonomy
 *     used by `holdings-table.tsx:390` for bond detection
 *     (BOND_*, DEBT_SECURITY, MONEY_MARKET_DEBT)
 *
 * Conservative by design: ambiguous holdings land in `'other'` rather
 * than being guessed into a panel. Track B's per-panel PRs refine this
 * classifier as the panel surfaces ship.
 */
export function classifyHolding(holding: Holding): AssetClassPanelId {
  // Cash accounts are the unambiguous bank-cash signal
  if (holding.holdingType?.toString().toUpperCase() === "CASH") return "bank-cash";

  const kind = holding.assetKind;
  if (kind === "PROPERTY") return "real-estate";
  // Vehicles excluded from net worth per Feroz #14 — bucketed for legibility
  if (kind === "VEHICLE") return "other";
  if (kind === "COLLECTIBLE") return "collectibles";
  if (kind === "PRECIOUS_METAL") return "commodities";
  if (kind === "PRIVATE_EQUITY") return "private-equity";
  if (kind === "FX") return "forex";
  // Liabilities surface on the Net Worth page, not on a panel tile
  if (kind === "LIABILITY") return "other";

  // Bond detection via taxonomy assetType key (matches existing pattern
  // in mizan-4/apps/frontend/src/pages/holdings/components/holdings-table.tsx:390)
  const assetTypeKey =
    holding.instrument?.classifications?.assetType?.key?.toString().toUpperCase() ?? "";
  if (
    assetTypeKey.startsWith("BOND_") ||
    assetTypeKey === "DEBT_SECURITY" ||
    assetTypeKey === "MONEY_MARKET_DEBT" ||
    assetTypeKey === "SUKUK"
  ) {
    return "bonds-sukuks";
  }
  if (assetTypeKey === "CRYPTOCURRENCY" || assetTypeKey === "CRYPTO") return "crypto";
  if (assetTypeKey === "PRECIOUS_METAL" || assetTypeKey === "METAL" || assetTypeKey === "COMMODITY")
    return "commodities";
  if (
    assetTypeKey === "EQUITY" ||
    assetTypeKey === "STOCK" ||
    assetTypeKey === "ETF" ||
    assetTypeKey === "MUTUAL_FUND" ||
    assetTypeKey === "FUND_MUTUAL" ||
    assetTypeKey === "EQUITY_SECURITY" ||
    assetTypeKey === "OPTION"
  ) {
    return "equities";
  }
  // Insurance products (ULIPs, term plans, endowment policies) — the
  // INSURANCE_PRODUCT category was added in migration
  // 2026-06-14-000001 alongside this branch. Without it ULIPs fell
  // through to `brokerage-accounts` and never reached the Insurance
  // tile (which has a descriptor but had no classifier rule until
  // now). The migration is forward-only; old DBs that never had
  // INSURANCE_PRODUCT just never return this key from the join, so
  // this branch is dead code for them — safe.
  if (assetTypeKey === "INSURANCE_PRODUCT") return "insurance";
  // Provident-fund balances (CPF, EPF, NPS, 401(k) viewed as a single
  // employer-managed pool) — same migration. Distinct from a
  // retirement BROKERAGE account (which would route to brokerage with
  // sub-classifications), and distinct from a private fund (which
  // would be `FUND_PRIVATE`).
  if (assetTypeKey === "PROVIDENT_FUND") return "provident-funds";

  // INVESTMENT-kind without a recognized assetType lands in brokerage-accounts
  // (the broadest bucket — matches what SnapTrade returns for un-classified
  // brokerage positions). Everything else falls to `'other'`.
  if (kind === "INVESTMENT") return "brokerage-accounts";

  return "other";
}

/**
 * Per-panel rollup: total market value (base currency) + 24h delta +
 * holdings count. Sparkline values + 30d delta come from price-history
 * in a follow-up — keeping the skeleton lean per Goal §III step 5.
 */
export interface AssetClassPanelRollup {
  panelId: AssetClassPanelId;
  totalValue: number;
  baseCurrency: string;
  dayChange: number;
  dayChangePct: number | null;
  holdingsCount: number;
}

/**
 * Compute per-panel rollups from a holdings list. Returns one entry per
 * panel in the fixed §3(e) order; absent panels show zero so the
 * dashboard layout stays stable across portfolios.
 *
 * `'other'` is appended only when non-empty.
 */
export function rollupHoldingsByPanel(
  holdings: readonly Holding[],
  baseCurrency: string,
): AssetClassPanelRollup[] {
  const buckets = new Map<AssetClassPanelId, { total: number; day: number; count: number }>();

  for (const holding of holdings) {
    const panelId = classifyHolding(holding);
    const total = holding.marketValue?.base ?? 0;
    const day = holding.dayChange?.base ?? 0;
    const bucket = buckets.get(panelId) ?? { total: 0, day: 0, count: 0 };
    bucket.total += total;
    bucket.day += day;
    bucket.count += 1;
    buckets.set(panelId, bucket);
  }

  const rollups: AssetClassPanelRollup[] = ASSET_CLASS_PANELS.map((panel) => {
    const bucket = buckets.get(panel.id) ?? { total: 0, day: 0, count: 0 };
    const dayChangePct =
      bucket.total - bucket.day !== 0 ? (bucket.day / (bucket.total - bucket.day)) * 100 : null;
    return {
      panelId: panel.id,
      totalValue: bucket.total,
      baseCurrency,
      dayChange: bucket.day,
      dayChangePct,
      holdingsCount: bucket.count,
    };
  });

  const otherBucket = buckets.get("other");
  if (otherBucket && otherBucket.count > 0) {
    const dayChangePct =
      otherBucket.total - otherBucket.day !== 0
        ? (otherBucket.day / (otherBucket.total - otherBucket.day)) * 100
        : null;
    rollups.push({
      panelId: "other",
      totalValue: otherBucket.total,
      baseCurrency,
      dayChange: otherBucket.day,
      dayChangePct,
      holdingsCount: otherBucket.count,
    });
  }

  return rollups;
}

/** Lookup helper — panel descriptor by id. Includes the synthetic `'other'`. */
export function getPanelDescriptor(id: AssetClassPanelId): AssetClassPanelDescriptor {
  if (id === "other") {
    return {
      id: "other",
      label: "Other",
      iconKey: "Box",
      holdingsHref: "/holdings?panel=other",
    };
  }
  const panel = ASSET_CLASS_PANELS.find((p) => p.id === id);
  if (!panel) {
    throw new Error(`Unknown asset-class panel id: ${String(id)}`);
  }
  return panel;
}
