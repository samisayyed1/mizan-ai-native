/**
 * Tests for Track B PR-B16 Forex rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type {
  AssetClassifications,
  Holding,
  Instrument,
  TaxonomyCategory,
} from "@/lib/types";

import {
  extractBaseCurrency,
  isFxHolding,
  rollupByLongLeg,
  rollupByPair,
  totalFxExposure,
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

function makeInstrument(
  symbol: string,
  name: string | null,
  assetTypeKey: string | null,
): Instrument {
  return {
    id: `i-${symbol}`,
    symbol,
    name,
    currency: "USD",
    quoteMode: "MARKET",
    classifications: makeClassifications(assetTypeKey),
  };
}

function makeFxHolding(opts: {
  id: string;
  symbol: string;
  name?: string | null;
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
    instrument: makeInstrument(opts.symbol, opts.name ?? null, opts.assetTypeKey ?? null),
  };
}

describe("isFxHolding", () => {
  it("matches assetKind FX", () => {
    const h = makeFxHolding({ id: "h", symbol: "USD/INR", assetKind: "FX", baseValue: 1000 });
    expect(isFxHolding(h)).toBe(true);
  });

  it.each([
    ["FX", true],
    ["CURRENCY", true],
    ["FOREX", true],
    ["EQUITY", false],
    ["SUKUK", false],
    [null, false],
  ])("matches taxonomy %s → %s", (key, expected) => {
    const h = makeFxHolding({ id: "h", symbol: "X", assetTypeKey: key, baseValue: 1000 });
    expect(isFxHolding(h)).toBe(expected);
  });

  it("lowercase taxonomy key still recognised", () => {
    const h = makeFxHolding({ id: "h", symbol: "USD/INR", assetTypeKey: "fx", baseValue: 1 });
    expect(isFxHolding(h)).toBe(true);
  });

  it("either signal is sufficient", () => {
    const h1 = makeFxHolding({ id: "h1", symbol: "USD/INR", assetKind: "FX", baseValue: 1 });
    expect(isFxHolding(h1)).toBe(true);
    const h2 = makeFxHolding({ id: "h2", symbol: "USD/INR", assetTypeKey: "FX", baseValue: 1 });
    expect(isFxHolding(h2)).toBe(true);
  });
});

describe("extractBaseCurrency", () => {
  it.each([
    ["USD/INR", "USD"],
    ["EUR/USD", "EUR"],
    ["USD-INR", "USD"],
    ["USDINR", "USD"],
    ["USDINR=X", "USD"],
    ["usd/inr", "USD"], // case-insensitive
    ["  USD/INR  ", "USD"], // whitespace
    ["", "Unknown"],
    ["INVALID", "Unknown"], // not 6 chars
    ["TOOLONGFORX", "Unknown"], // not 6 chars (and no separator)
  ] as const)("%s → %s", (symbol, expected) => {
    expect(extractBaseCurrency(symbol)).toBe(expected);
  });
});

describe("rollupByPair", () => {
  it("emits one bar per FX position, sorted desc", () => {
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USD/INR", name: "USD/INR", assetKind: "FX", baseValue: 50_000 }),
      makeFxHolding({ id: "h2", symbol: "EUR/USD", name: "EUR/USD", assetKind: "FX", baseValue: 100_000 }),
      makeFxHolding({ id: "h3", symbol: "USD/SGD", name: "USD/SGD", assetKind: "FX", baseValue: 25_000 }),
    ];
    const rows = rollupByPair(holdings);
    expect(rows.map((r) => r.label)).toEqual(["EUR/USD", "USD/INR", "USD/SGD"]);
    expect(rows[0]?.exposureBase).toBe(100_000);
  });

  it("falls back to symbol when name missing", () => {
    const h = makeFxHolding({ id: "h", symbol: "USDINR=X", assetKind: "FX", baseValue: 1000 });
    if (h.instrument) h.instrument.name = null;
    const rows = rollupByPair([h]);
    expect(rows[0]?.label).toBe("USDINR=X");
  });

  it("skips non-FX holdings", () => {
    const h = makeFxHolding({ id: "h", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 });
    expect(rollupByPair([h])).toEqual([]);
  });

  it("does NOT aggregate by pair — distinct positions stay distinct", () => {
    // Two USD/INR positions in different accounts → two distinct
    // rows (mirrors Real Estate per-property convention).
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USD/INR", assetKind: "FX", baseValue: 50_000 }),
      makeFxHolding({ id: "h2", symbol: "USD/INR", assetKind: "FX", baseValue: 30_000 }),
    ];
    const rows = rollupByPair(holdings);
    expect(rows).toHaveLength(2);
    expect(rows[0]?.exposureBase).toBe(50_000);
    expect(rows[1]?.exposureBase).toBe(30_000);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USD/INR", assetKind: "FX", baseValue: 0 }),
      makeFxHolding({ id: "h2", symbol: "EUR/USD", assetKind: "FX", baseValue: -1 }),
      makeFxHolding({ id: "h3", symbol: "USD/SGD", assetKind: "FX", baseValue: 100 }),
    ];
    expect(rollupByPair(holdings)).toHaveLength(1);
  });

  it("preserves holdingId for nav", () => {
    const h = makeFxHolding({ id: "holding_unique", symbol: "USD/INR", assetKind: "FX", baseValue: 1000 });
    expect(rollupByPair([h])[0]?.holdingId).toBe("holding_unique");
  });
});

describe("rollupByLongLeg", () => {
  it("aggregates USD long across pairs (USD/INR + USD/SGD)", () => {
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USD/INR", assetKind: "FX", baseValue: 50_000 }),
      makeFxHolding({ id: "h2", symbol: "USD/SGD", assetKind: "FX", baseValue: 30_000 }),
      makeFxHolding({ id: "h3", symbol: "EUR/USD", assetKind: "FX", baseValue: 20_000 }),
    ];
    const rows = rollupByLongLeg(holdings);
    expect(rows).toEqual([
      { currency: "USD", exposureBase: 80_000 },
      { currency: "EUR", exposureBase: 20_000 },
    ]);
  });

  it("falls to Unknown when symbol doesn't match a recognised format", () => {
    const h = makeFxHolding({ id: "h", symbol: "MYSTERY-FX", assetKind: "FX", baseValue: 1000 });
    expect(rollupByLongLeg([h])).toEqual([{ currency: "Unknown", exposureBase: 1000 }]);
  });

  it("skips non-FX holdings", () => {
    const h = makeFxHolding({ id: "h", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 });
    expect(rollupByLongLeg([h])).toEqual([]);
  });

  it("handles Yahoo Finance format (USDINR=X)", () => {
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USDINR=X", assetKind: "FX", baseValue: 50_000 }),
    ];
    expect(rollupByLongLeg(holdings)).toEqual([{ currency: "USD", exposureBase: 50_000 }]);
  });
});

describe("totalFxExposure", () => {
  it("sums FX-only holdings", () => {
    const holdings = [
      makeFxHolding({ id: "h1", symbol: "USD/INR", assetKind: "FX", baseValue: 50_000 }),
      makeFxHolding({ id: "h2", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 1_000_000 }),
      makeFxHolding({ id: "h3", symbol: "EUR/USD", assetKind: "FX", baseValue: 30_000 }),
    ];
    expect(totalFxExposure(holdings)).toBe(80_000);
  });

  it("empty input returns 0", () => {
    expect(totalFxExposure([])).toBe(0);
  });
});
