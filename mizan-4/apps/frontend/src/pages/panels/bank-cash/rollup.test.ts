/**
 * Tests for Track B PR-B3 Bank & Cash rollup helpers.
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
  isCashHolding,
  rollupByCountry,
  rollupByCurrency,
  totalCashExposure,
} from "./rollup";

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
  const topLevelCategory: CategoryRef = { id: `region-${name}-top`, name };
  return { category, topLevelCategory, weight };
}

function makeClassifications(regions: CategoryWithWeight[] = []): AssetClassifications {
  return {
    assetType: null,
    riskCategory: null,
    assetClasses: [],
    sectors: [],
    regions,
    customGroups: [],
  };
}

function makeInstrument(
  symbol: string,
  currency: string,
  regions: CategoryWithWeight[] = [],
): Instrument {
  return {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency,
    quoteMode: "MARKET",
    classifications: makeClassifications(regions),
  };
}

function makeHolding(opts: {
  id: string;
  symbol: string;
  holdingType: "cash" | "security";
  currency?: string;
  baseValue?: number;
  regions?: CategoryWithWeight[];
}): Holding {
  return {
    id: opts.id,
    holdingType: opts.holdingType as Holding["holdingType"],
    accountId: "acc-1",
    quantity: 1,
    localCurrency: opts.currency ?? "USD",
    baseCurrency: "USD",
    marketValue: {
      local: opts.baseValue ?? 0,
      base: opts.baseValue ?? 0,
    },
    weight: 1,
    asOfDate: "2026-06-04",
    instrument: opts.symbol
      ? makeInstrument(opts.symbol, opts.currency ?? "USD", opts.regions ?? [])
      : null,
  };
}

describe("isCashHolding", () => {
  it("returns true for cash holdingType", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "DBS-SGD",
      holdingType: "cash",
      currency: "SGD",
      baseValue: 100_000,
    });
    expect(isCashHolding(h)).toBe(true);
  });

  it("returns false for security holdingType", () => {
    const h = makeHolding({
      id: "h2",
      symbol: "AAPL",
      holdingType: "security",
      currency: "USD",
      baseValue: 100_000,
    });
    expect(isCashHolding(h)).toBe(false);
  });

  it("is case-insensitive on holdingType string", () => {
    // Defensive: the constants enum uses lowercase, but defensive
    // matching guards against API drift returning mixed case.
    const h = {
      ...makeHolding({
        id: "h3",
        symbol: "DBS-SGD",
        holdingType: "cash",
        currency: "SGD",
        baseValue: 100,
      }),
      holdingType: "CASH" as unknown as Holding["holdingType"],
    };
    expect(isCashHolding(h)).toBe(true);
  });
});

describe("rollupByCurrency", () => {
  it("groups cash by instrument.currency, sorted desc", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "DBS-SGD", holdingType: "cash", currency: "SGD", baseValue: 100_000 }),
      makeHolding({ id: "h2", symbol: "DBS-USD", holdingType: "cash", currency: "USD", baseValue: 250_000 }),
      makeHolding({ id: "h3", symbol: "HSBC-SGD", holdingType: "cash", currency: "SGD", baseValue: 50_000 }),
    ];
    const rows = rollupByCurrency(holdings);
    expect(rows).toEqual([
      { currency: "USD", exposureBase: 250_000 },
      { currency: "SGD", exposureBase: 150_000 },
    ]);
  });

  it("skips security holdings entirely", () => {
    const h = makeHolding({ id: "h1", symbol: "AAPL", holdingType: "security", currency: "USD", baseValue: 100_000 });
    expect(rollupByCurrency([h])).toEqual([]);
  });

  it("falls back to UNKNOWN when currency is empty", () => {
    const h = makeHolding({ id: "h1", symbol: "X", holdingType: "cash", currency: "", baseValue: 1000 });
    const rows = rollupByCurrency([h]);
    expect(rows[0]?.currency).toBe("UNKNOWN");
  });

  it("normalises lowercase currency to uppercase (sgd === SGD)", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "X", holdingType: "cash", currency: "sgd", baseValue: 100 }),
      makeHolding({ id: "h2", symbol: "Y", holdingType: "cash", currency: "SGD", baseValue: 200 }),
    ];
    const rows = rollupByCurrency(holdings);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.currency).toBe("SGD");
    expect(rows[0]?.exposureBase).toBe(300);
  });

  it("skips zero / negative values", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "X", holdingType: "cash", currency: "USD", baseValue: 0 }),
      makeHolding({ id: "h2", symbol: "Y", holdingType: "cash", currency: "USD", baseValue: -100 }),
      makeHolding({ id: "h3", symbol: "Z", holdingType: "cash", currency: "USD", baseValue: 100 }),
    ];
    expect(rollupByCurrency(holdings)).toEqual([{ currency: "USD", exposureBase: 100 }]);
  });
});

describe("rollupByCountry", () => {
  it("groups cash by region, weighted-split with re-normalisation", () => {
    const holdings = [
      makeHolding({
        id: "h1",
        symbol: "DBS-SG",
        holdingType: "cash",
        currency: "SGD",
        baseValue: 100_000,
        regions: [makeRegion("Singapore", 100)],
      }),
      makeHolding({
        id: "h2",
        symbol: "HSBC-UAE",
        holdingType: "cash",
        currency: "AED",
        baseValue: 50_000,
        regions: [makeRegion("United Arab Emirates", 100)],
      }),
      makeHolding({
        id: "h3",
        symbol: "OCBC-SG",
        holdingType: "cash",
        currency: "SGD",
        baseValue: 80_000,
        regions: [makeRegion("Singapore", 100)],
      }),
    ];
    const rows = rollupByCountry(holdings);
    expect(rows).toEqual([
      { country: "Singapore", exposureBase: 180_000 },
      { country: "United Arab Emirates", exposureBase: 50_000 },
    ]);
  });

  it("falls back to Unspecified when no region classification", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "X",
      holdingType: "cash",
      currency: "USD",
      baseValue: 50_000,
      regions: [],
    });
    expect(rollupByCountry([h])).toEqual([{ country: "Unspecified", exposureBase: 50_000 }]);
  });

  it("re-normalises when region weights sum >100", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "GLOBAL-CASH",
      holdingType: "cash",
      currency: "USD",
      baseValue: 100_000,
      regions: [makeRegion("Singapore", 60), makeRegion("UAE", 60)],
    });
    const rows = rollupByCountry([h]);
    const total = rows.reduce((acc, r) => acc + r.exposureBase, 0);
    expect(total).toBeCloseTo(100_000, 5);
    expect(rows[0]?.exposureBase).toBeCloseTo(50_000, 5);
    expect(rows[1]?.exposureBase).toBeCloseTo(50_000, 5);
  });

  it("skips security holdings", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "AAPL",
      holdingType: "security",
      currency: "USD",
      baseValue: 100_000,
      regions: [makeRegion("United States", 100)],
    });
    expect(rollupByCountry([h])).toEqual([]);
  });
});

describe("totalCashExposure", () => {
  it("sums cash-only holdings in base currency", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "X", holdingType: "cash", currency: "USD", baseValue: 100_000 }),
      makeHolding({ id: "h2", symbol: "AAPL", holdingType: "security", currency: "USD", baseValue: 500_000 }),
      makeHolding({ id: "h3", symbol: "Y", holdingType: "cash", currency: "SGD", baseValue: 50_000 }),
    ];
    expect(totalCashExposure(holdings)).toBe(150_000);
  });

  it("returns 0 for empty input", () => {
    expect(totalCashExposure([])).toBe(0);
  });
});
