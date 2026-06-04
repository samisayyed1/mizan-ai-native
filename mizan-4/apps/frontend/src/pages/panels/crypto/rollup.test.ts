/**
 * Tests for Track B PR-B9 Crypto rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type {
  AssetClassifications,
  Holding,
  Instrument,
  TaxonomyCategory,
} from "@/lib/types";

import {
  chainForSymbol,
  isCryptoHolding,
  rollupByChain,
  totalCryptoExposure,
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
    instrument: makeInstrument(opts.symbol, opts.assetTypeKey ?? null),
  };
}

describe("isCryptoHolding", () => {
  it.each([
    ["CRYPTOCURRENCY", true],
    ["CRYPTO", true],
    ["EQUITY", false],
    ["SUKUK", false],
    ["BOND_GOVERNMENT", false],
    [null, false],
  ])("for %s returns %s", (key, expected) => {
    const h = makeHolding({ id: "h", symbol: "BTC", assetTypeKey: key, baseValue: 1000 });
    expect(isCryptoHolding(h)).toBe(expected);
  });

  it("lowercase key still recognised", () => {
    const h = makeHolding({ id: "h", symbol: "BTC", assetTypeKey: "cryptocurrency", baseValue: 1000 });
    expect(isCryptoHolding(h)).toBe(true);
  });
});

describe("chainForSymbol", () => {
  it.each([
    ["BTC", "Bitcoin"],
    ["WBTC", "Bitcoin"],
    ["ETH", "Ethereum"],
    ["WETH", "Ethereum"],
    ["stETH", "Ethereum"],
    ["wstETH", "Ethereum"],
    ["rETH", "Ethereum"],
    ["SOL", "Solana"],
    ["jitoSOL", "Solana"],
    ["BNB", "BNB Chain"],
    ["MATIC", "Polygon"],
    ["POL", "Polygon"],
    ["USDC", "Stablecoins"],
    ["USDT", "Stablecoins"],
    ["DAI", "Stablecoins"],
    ["PYUSD", "Stablecoins"],
    ["FRAX", "Stablecoins"],
    ["DOGE", "Other"],
    ["LINK", "Other"],
    ["", "Other"],
  ] as const)("%s → %s", (symbol, expected) => {
    expect(chainForSymbol(symbol)).toBe(expected);
  });

  it("case-insensitive (btc ≡ BTC)", () => {
    expect(chainForSymbol("btc")).toBe("Bitcoin");
    expect(chainForSymbol("BtC")).toBe("Bitcoin");
  });

  it("trims whitespace", () => {
    expect(chainForSymbol("  ETH  ")).toBe("Ethereum");
  });
});

describe("rollupByChain", () => {
  it("groups + sorts desc by exposure (§23-flavored mix)", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "BTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 50_000 }),
      makeHolding({ id: "h2", symbol: "ETH", assetTypeKey: "CRYPTOCURRENCY", baseValue: 30_000 }),
      makeHolding({ id: "h3", symbol: "USDC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 100_000 }),
      makeHolding({ id: "h4", symbol: "SOL", assetTypeKey: "CRYPTOCURRENCY", baseValue: 10_000 }),
    ];
    const rows = rollupByChain(holdings);
    expect(rows).toEqual([
      { chain: "Stablecoins", exposureBase: 100_000 },
      { chain: "Bitcoin", exposureBase: 50_000 },
      { chain: "Ethereum", exposureBase: 30_000 },
      { chain: "Solana", exposureBase: 10_000 },
    ]);
  });

  it("aggregates wrapped variants into the canonical chain", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "BTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 25_000 }),
      makeHolding({ id: "h2", symbol: "WBTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 25_000 }),
      makeHolding({ id: "h3", symbol: "ETH", assetTypeKey: "CRYPTOCURRENCY", baseValue: 10_000 }),
      makeHolding({ id: "h4", symbol: "stETH", assetTypeKey: "CRYPTOCURRENCY", baseValue: 5_000 }),
    ];
    const rows = rollupByChain(holdings);
    expect(rows).toEqual([
      { chain: "Bitcoin", exposureBase: 50_000 },
      { chain: "Ethereum", exposureBase: 15_000 },
    ]);
  });

  it("unknown coin falls to Other", () => {
    const h = makeHolding({ id: "h1", symbol: "DOGE", assetTypeKey: "CRYPTOCURRENCY", baseValue: 1000 });
    expect(rollupByChain([h])).toEqual([{ chain: "Other", exposureBase: 1000 }]);
  });

  it("skips non-crypto holdings", () => {
    const h = makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 });
    expect(rollupByChain([h])).toEqual([]);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "BTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 0 }),
      makeHolding({ id: "h2", symbol: "ETH", assetTypeKey: "CRYPTOCURRENCY", baseValue: -100 }),
      makeHolding({ id: "h3", symbol: "BTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 100 }),
    ];
    expect(rollupByChain(holdings)).toEqual([{ chain: "Bitcoin", exposureBase: 100 }]);
  });

  it("empty input returns empty", () => {
    expect(rollupByChain([])).toEqual([]);
  });
});

describe("totalCryptoExposure", () => {
  it("sums crypto-only holdings", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "BTC", assetTypeKey: "CRYPTOCURRENCY", baseValue: 50_000 }),
      makeHolding({ id: "h2", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000 }),
      makeHolding({ id: "h3", symbol: "ETH", assetTypeKey: "CRYPTO", baseValue: 30_000 }),
    ];
    expect(totalCryptoExposure(holdings)).toBe(80_000);
  });

  it("empty input returns 0", () => {
    expect(totalCryptoExposure([])).toBe(0);
  });
});
