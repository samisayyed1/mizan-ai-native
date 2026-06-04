/**
 * Tests for Track B PR-B2 Equities-panel rollup helpers.
 *
 * Pure-math tests — no React, no Tauri bridge.
 */
import { describe, expect, it } from "vitest";

import type {
  AssetClassifications,
  CategoryRef,
  CategoryWithWeight,
  Holding,
  Instrument,
  TaxonomyCategory,
} from "@/lib/types";

import {
  isEquityHolding,
  rollupByRegion,
  rollupBySubclass,
  totalEquityExposure,
} from "./rollup";

function makeAssetType(key: string): TaxonomyCategory {
  return {
    id: `cat-${key}`,
    taxonomyId: "tx-asset-type",
    name: key,
    key,
    color: "#000",
    sortOrder: 0,
    createdAt: "2026-06-04T00:00:00Z",
    updatedAt: "2026-06-04T00:00:00Z",
  };
}

function makeRegion(name: string, weight: number): CategoryWithWeight {
  const category: TaxonomyCategory = {
    id: `region-${name}`,
    taxonomyId: "tx-regions",
    name,
    key: name.toUpperCase().replace(/ /g, "_"),
    color: "#000",
    sortOrder: 0,
    createdAt: "2026-06-04T00:00:00Z",
    updatedAt: "2026-06-04T00:00:00Z",
  };
  const topLevelCategory: CategoryRef = {
    id: `region-${name}-top`,
    name,
  };
  return { category, topLevelCategory, weight };
}

function makeClassifications(
  assetTypeKey: string | null,
  regions: CategoryWithWeight[] = [],
): AssetClassifications {
  return {
    assetType: assetTypeKey ? makeAssetType(assetTypeKey) : null,
    riskCategory: null,
    assetClasses: [],
    sectors: [],
    regions,
    customGroups: [],
  };
}

function makeInstrument(
  symbol: string,
  name: string | null,
  assetTypeKey: string | null,
  regions: CategoryWithWeight[] = [],
): Instrument {
  return {
    id: `i-${symbol}`,
    symbol,
    name,
    currency: "USD",
    quoteMode: "MARKET",
    classifications: makeClassifications(assetTypeKey, regions),
  };
}

function makeHolding(opts: {
  id: string;
  symbol: string;
  name?: string | null;
  assetTypeKey?: string | null;
  regions?: CategoryWithWeight[];
  baseValue?: number;
}): Holding {
  return {
    id: opts.id,
    holdingType: "INVESTMENT" as Holding["holdingType"],
    accountId: "acc-1",
    quantity: 1,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: {
      local: opts.baseValue ?? 0,
      base: opts.baseValue ?? 0,
    },
    weight: 1,
    asOfDate: "2026-06-04",
    instrument: makeInstrument(
      opts.symbol,
      opts.name ?? null,
      opts.assetTypeKey ?? null,
      opts.regions ?? [],
    ),
  };
}

describe("isEquityHolding", () => {
  it.each([
    ["EQUITY", true],
    ["STOCK", true],
    ["ETF", true],
    ["MUTUAL_FUND", true],
    ["OPTION", true],
    ["BOND_GOVERNMENT", false],
    ["SUKUK", false],
    ["CRYPTOCURRENCY", false],
    [null, false],
  ])("for %s returns %s", (key, expected) => {
    const h = makeHolding({ id: "h", symbol: "X", assetTypeKey: key, baseValue: 1000 });
    expect(isEquityHolding(h)).toBe(expected);
  });

  it("lowercase asset key still recognised", () => {
    const h = makeHolding({ id: "h", symbol: "X", assetTypeKey: "etf", baseValue: 1000 });
    expect(isEquityHolding(h)).toBe(true);
  });
});

describe("rollupBySubclass", () => {
  it("partitions across Stocks / ETFs / Mutual Funds / Options", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 500_000 }),
      makeHolding({ id: "h2", symbol: "SPUS", assetTypeKey: "ETF", baseValue: 250_000 }),
      makeHolding({ id: "h3", symbol: "VFINX", assetTypeKey: "MUTUAL_FUND", baseValue: 150_000 }),
      makeHolding({ id: "h4", symbol: "AAPL_C", assetTypeKey: "OPTION", baseValue: 25_000 }),
    ];
    const rows = rollupBySubclass(holdings);
    // Sorted descending by value: Stocks 500K, ETFs 250K, MF 150K, Options 25K
    expect(rows).toEqual([
      { label: "Stocks", value: 500_000 },
      { label: "ETFs", value: 250_000 },
      { label: "Mutual Funds", value: 150_000 },
      { label: "Options", value: 25_000 },
    ]);
  });

  it("skips non-equity holdings", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "EMAAR",
      assetTypeKey: "SUKUK",
      baseValue: 100_000,
    });
    expect(rollupBySubclass([h])).toEqual([]);
  });

  it("aggregates duplicates within a subclass", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 }),
      makeHolding({ id: "h2", symbol: "MSFT", assetTypeKey: "STOCK", baseValue: 50_000 }),
    ];
    const rows = rollupBySubclass(holdings);
    expect(rows).toEqual([{ label: "Stocks", value: 150_000 }]);
  });

  it("skips zero / negative values", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "X", assetTypeKey: "EQUITY", baseValue: 0 }),
      makeHolding({ id: "h2", symbol: "Y", assetTypeKey: "EQUITY", baseValue: -100 }),
      makeHolding({ id: "h3", symbol: "Z", assetTypeKey: "EQUITY", baseValue: 100 }),
    ];
    expect(rollupBySubclass(holdings)).toEqual([{ label: "Stocks", value: 100 }]);
  });

  it("returns empty for no holdings", () => {
    expect(rollupBySubclass([])).toEqual([]);
  });
});

describe("rollupByRegion", () => {
  it("splits a 50/50 holding across two regions in equal share", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "GLOBAL",
        assetTypeKey: "ETF",
        baseValue: 200_000,
        regions: [makeRegion("United States", 50), makeRegion("Europe", 50)],
      }),
    ];
    const rows = rollupByRegion(holdings);
    expect(rows.length).toBe(2);
    // Sorted descending by exposure (tied: pick whichever comes first
    // from the Map; both 100K so order is insertion).
    const total = rows.reduce((acc, r) => acc + r.exposureBase, 0);
    expect(total).toBe(200_000);
    expect(rows.find((r) => r.region === "United States")?.exposureBase).toBe(100_000);
    expect(rows.find((r) => r.region === "Europe")?.exposureBase).toBe(100_000);
  });

  it("bucketises unclassified holdings to Unspecified", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "X",
        assetTypeKey: "EQUITY",
        baseValue: 50_000,
        regions: [],
      }),
    ];
    expect(rollupByRegion(holdings)).toEqual([
      { region: "Unspecified", exposureBase: 50_000 },
    ]);
  });

  it("normalises when region weights sum to more than 100", () => {
    // 60+60=120; 200K should still split into 100K/100K (re-normalised).
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "GLOBAL",
        assetTypeKey: "ETF",
        baseValue: 200_000,
        regions: [makeRegion("United States", 60), makeRegion("Europe", 60)],
      }),
    ];
    const rows = rollupByRegion(holdings);
    const total = rows.reduce((acc, r) => acc + r.exposureBase, 0);
    expect(total).toBeCloseTo(200_000, 5);
    // Equal weights → equal shares
    expect(rows[0]?.exposureBase).toBeCloseTo(100_000, 5);
    expect(rows[1]?.exposureBase).toBeCloseTo(100_000, 5);
  });

  it("falls to Unspecified when all weights are zero", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "X",
        assetTypeKey: "EQUITY",
        baseValue: 50_000,
        regions: [makeRegion("United States", 0)],
      }),
    ];
    expect(rollupByRegion(holdings)).toEqual([
      { region: "Unspecified", exposureBase: 50_000 },
    ]);
  });

  it("skips non-equity holdings", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "EMAAR",
        assetTypeKey: "SUKUK",
        baseValue: 100_000,
        regions: [makeRegion("UAE", 100)],
      }),
    ];
    expect(rollupByRegion(holdings)).toEqual([]);
  });

  it("aggregates across holdings and sorts descending by exposure", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "AAPL",
        assetTypeKey: "EQUITY",
        baseValue: 500_000,
        regions: [makeRegion("United States", 100)],
      }),
      makeHolding({
        id: "h2",
        symbol: "ASML",
        assetTypeKey: "EQUITY",
        baseValue: 200_000,
        regions: [makeRegion("Europe", 100)],
      }),
      makeHolding({
        id: "h3",
        symbol: "MSFT",
        assetTypeKey: "STOCK",
        baseValue: 100_000,
        regions: [makeRegion("United States", 100)],
      }),
    ];
    const rows = rollupByRegion(holdings);
    expect(rows).toEqual([
      { region: "United States", exposureBase: 600_000 },
      { region: "Europe", exposureBase: 200_000 },
    ]);
  });
});

describe("totalEquityExposure", () => {
  it("sums across equity holdings only", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 300_000 }),
      makeHolding({ id: "h2", symbol: "EMAAR", assetTypeKey: "SUKUK", baseValue: 200_000 }),
      makeHolding({ id: "h3", symbol: "SPUS", assetTypeKey: "ETF", baseValue: 100_000 }),
    ];
    expect(totalEquityExposure(holdings)).toBe(400_000);
  });

  it("returns 0 for empty input", () => {
    expect(totalEquityExposure([])).toBe(0);
  });
});
