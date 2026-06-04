/**
 * Tests for Track B PR-B10 Commodities rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type {
  AssetClassifications,
  Holding,
  Instrument,
  TaxonomyCategory,
} from "@/lib/types";

import {
  isCommodityHolding,
  metalForSymbol,
  rollupByMetal,
  totalCommoditiesExposure,
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

function makeClassifications(assetTypeKey: string | null): AssetClassifications {
  return {
    assetType: assetTypeKey ? makeAssetType(assetTypeKey) : null,
    riskCategory: null,
    assetClasses: [],
    sectors: [],
    regions: [],
    customGroups: [],
  };
}

function makeInstrument(symbol: string, assetTypeKey: string | null): Instrument {
  return {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MARKET",
    classifications: makeClassifications(assetTypeKey),
  };
}

function makeHolding(opts: {
  id: string;
  symbol: string;
  assetKind?: Holding["assetKind"];
  assetTypeKey?: string | null;
  baseValue?: number;
}): Holding {
  return {
    id: opts.id,
    holdingType: "SECURITY" as Holding["holdingType"],
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
    assetKind: opts.assetKind,
    instrument: makeInstrument(opts.symbol, opts.assetTypeKey ?? null),
  };
}

describe("isCommodityHolding", () => {
  it("matches assetKind PRECIOUS_METAL", () => {
    const h = makeHolding({ id: "h", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 1000 });
    expect(isCommodityHolding(h)).toBe(true);
  });

  it.each([
    ["METAL", true],
    ["PRECIOUS_METAL", true],
    ["COMMODITY", true],
    ["EQUITY", false],
    ["SUKUK", false],
    [null, false],
  ])("matches taxonomy %s → %s", (key, expected) => {
    const h = makeHolding({ id: "h", symbol: "X", assetTypeKey: key, baseValue: 1000 });
    expect(isCommodityHolding(h)).toBe(expected);
  });

  it("either signal is sufficient (assetKind OR classification)", () => {
    // assetKind set but classification missing → still recognised
    const h1 = makeHolding({ id: "h1", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 1 });
    expect(isCommodityHolding(h1)).toBe(true);
    // classification set but assetKind missing → still recognised
    const h2 = makeHolding({ id: "h2", symbol: "GLD", assetTypeKey: "METAL", baseValue: 1 });
    expect(isCommodityHolding(h2)).toBe(true);
  });

  it("lowercase classification key still recognised", () => {
    const h = makeHolding({ id: "h", symbol: "X", assetTypeKey: "precious_metal", baseValue: 1 });
    expect(isCommodityHolding(h)).toBe(true);
  });
});

describe("metalForSymbol", () => {
  it.each([
    ["XAU", "Gold"],
    ["GOLD", "Gold"],
    ["GLD", "Gold"],
    ["IAU", "Gold"],
    ["PHYS", "Gold"],
    ["SGOL", "Gold"],
    ["XAG", "Silver"],
    ["SLV", "Silver"],
    ["SIVR", "Silver"],
    ["PSLV", "Silver"],
    ["XPT", "Platinum"],
    ["PPLT", "Platinum"],
    ["XPD", "Palladium"],
    ["PALL", "Palladium"],
    ["UNKNOWN", "Other"],
    ["BTC", "Other"],
    ["", "Other"],
  ] as const)("%s → %s", (symbol, expected) => {
    expect(metalForSymbol(symbol)).toBe(expected);
  });

  it("case-insensitive (gld ≡ GLD)", () => {
    expect(metalForSymbol("gld")).toBe("Gold");
    expect(metalForSymbol("GlD")).toBe("Gold");
  });

  it("trims whitespace", () => {
    expect(metalForSymbol("  XAU  ")).toBe("Gold");
  });
});

describe("rollupByMetal", () => {
  it("groups + sorts desc by exposure (multi-metal fixture)", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 100_000 }),
      makeHolding({ id: "h2", symbol: "SLV", assetKind: "PRECIOUS_METAL", baseValue: 30_000 }),
      makeHolding({ id: "h3", symbol: "XAU", assetKind: "PRECIOUS_METAL", baseValue: 50_000 }),
      makeHolding({ id: "h4", symbol: "PPLT", assetKind: "PRECIOUS_METAL", baseValue: 10_000 }),
    ];
    const rows = rollupByMetal(holdings);
    expect(rows).toEqual([
      { metal: "Gold", exposureBase: 150_000 },
      { metal: "Silver", exposureBase: 30_000 },
      { metal: "Platinum", exposureBase: 10_000 },
    ]);
  });

  it("aggregates COMEX + ETF symbols into the canonical metal bucket", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "XAU", assetKind: "PRECIOUS_METAL", baseValue: 50_000 }),
      makeHolding({ id: "h2", symbol: "IAU", assetKind: "PRECIOUS_METAL", baseValue: 25_000 }),
      makeHolding({ id: "h3", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 25_000 }),
    ];
    expect(rollupByMetal(holdings)).toEqual([{ metal: "Gold", exposureBase: 100_000 }]);
  });

  it("unknown commodity falls to Other", () => {
    const h = makeHolding({ id: "h", symbol: "CORN-FUT", assetTypeKey: "COMMODITY", baseValue: 5000 });
    expect(rollupByMetal([h])).toEqual([{ metal: "Other", exposureBase: 5000 }]);
  });

  it("skips non-commodity holdings", () => {
    const h = makeHolding({ id: "h", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 });
    expect(rollupByMetal([h])).toEqual([]);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 0 }),
      makeHolding({ id: "h2", symbol: "SLV", assetKind: "PRECIOUS_METAL", baseValue: -100 }),
      makeHolding({ id: "h3", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 100 }),
    ];
    expect(rollupByMetal(holdings)).toEqual([{ metal: "Gold", exposureBase: 100 }]);
  });

  it("empty input returns empty", () => {
    expect(rollupByMetal([])).toEqual([]);
  });
});

describe("totalCommoditiesExposure", () => {
  it("sums commodities-only holdings", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "GLD", assetKind: "PRECIOUS_METAL", baseValue: 50_000 }),
      makeHolding({ id: "h2", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 }),
      makeHolding({ id: "h3", symbol: "SLV", assetKind: "PRECIOUS_METAL", baseValue: 30_000 }),
    ];
    expect(totalCommoditiesExposure(holdings)).toBe(80_000);
  });

  it("empty input returns 0", () => {
    expect(totalCommoditiesExposure([])).toBe(0);
  });
});
