/**
 * Tests for Track B PR-B15 Collectibles rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type { Holding, Instrument } from "@/lib/types";

import {
  isCollectibleHolding,
  rollupByCategory,
  rollupByItem,
  totalCollectiblesExposure,
} from "./rollup";

function makeInstrument(symbol: string, collectibleType?: string): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MANUAL",
    classifications: null,
  };
  if (collectibleType) {
    (base as unknown as { metadata: { collectible: { collectibleType: string } } }).metadata = {
      collectible: { collectibleType },
    };
  }
  return base;
}

function makeCollectible(opts: {
  id: string;
  symbol: string;
  collectibleType?: string;
  baseValue?: number;
  assetKind?: Holding["assetKind"];
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
    assetKind: opts.assetKind ?? "COLLECTIBLE",
    instrument: makeInstrument(opts.symbol, opts.collectibleType),
  };
}

describe("isCollectibleHolding", () => {
  it("returns true for COLLECTIBLE assetKind", () => {
    const h = makeCollectible({ id: "h1", symbol: "Rolex", baseValue: 25_000 });
    expect(isCollectibleHolding(h)).toBe(true);
  });

  it("returns false for PROPERTY / INVESTMENT / VEHICLE / OTHER", () => {
    for (const kind of ["PROPERTY", "INVESTMENT", "VEHICLE", "OTHER", "PRECIOUS_METAL"] as const) {
      const h = makeCollectible({ id: `h-${kind}`, symbol: "X", baseValue: 100, assetKind: kind });
      expect(isCollectibleHolding(h)).toBe(false);
    }
  });

  it("returns false when assetKind is missing", () => {
    const h = makeCollectible({ id: "h", symbol: "X", baseValue: 100 });
    delete (h as { assetKind?: unknown }).assetKind;
    expect(isCollectibleHolding(h)).toBe(false);
  });
});

describe("rollupByCategory", () => {
  it("classifies Watches/Art/Jewelry/Wine/Memorabilia/Other", () => {
    const holdings = [
      makeCollectible({ id: "h1", symbol: "Rolex Daytona", collectibleType: "watch", baseValue: 35_000 }),
      makeCollectible({ id: "h2", symbol: "Patek Nautilus", collectibleType: "watch", baseValue: 80_000 }),
      makeCollectible({ id: "h3", symbol: "Banksy", collectibleType: "art", baseValue: 120_000 }),
      makeCollectible({ id: "h4", symbol: "Cartier ring", collectibleType: "jewelry", baseValue: 15_000 }),
      makeCollectible({ id: "h5", symbol: "1982 Lafite", collectibleType: "wine", baseValue: 5_000 }),
      makeCollectible({ id: "h6", symbol: "Jordan rookie", collectibleType: "memorabilia", baseValue: 8_000 }),
      makeCollectible({ id: "h7", symbol: "vintage map", baseValue: 2_000 }),
    ];
    const rows = rollupByCategory(holdings);
    expect(rows).toEqual([
      { category: "Watches", value: 115_000 },
      { category: "Art", value: 120_000 },
      { category: "Jewelry", value: 15_000 },
      { category: "Memorabilia", value: 8_000 },
      { category: "Wine", value: 5_000 },
      { category: "Other", value: 2_000 },
    ].sort((a, b) => b.value - a.value));
  });

  it("missing collectibleType → Other", () => {
    const h = makeCollectible({ id: "h", symbol: "X", baseValue: 1000 });
    expect(rollupByCategory([h])).toEqual([{ category: "Other", value: 1000 }]);
  });

  it("case-insensitive collectibleType", () => {
    const holdings = [
      makeCollectible({ id: "h1", symbol: "A", collectibleType: "Watch", baseValue: 1 }),
      makeCollectible({ id: "h2", symbol: "B", collectibleType: "WATCH", baseValue: 2 }),
      makeCollectible({ id: "h3", symbol: "C", collectibleType: "watch", baseValue: 3 }),
    ];
    expect(rollupByCategory(holdings)).toEqual([{ category: "Watches", value: 6 }]);
  });

  it("returns empty for non-collectible holdings", () => {
    const h = makeCollectible({ id: "h", symbol: "AAPL", baseValue: 1000, assetKind: "INVESTMENT" });
    expect(rollupByCategory([h])).toEqual([]);
  });

  it("skips zero/negative", () => {
    const holdings = [
      makeCollectible({ id: "h1", symbol: "X", collectibleType: "watch", baseValue: 0 }),
      makeCollectible({ id: "h2", symbol: "Y", collectibleType: "watch", baseValue: -1 }),
      makeCollectible({ id: "h3", symbol: "Z", collectibleType: "watch", baseValue: 100 }),
    ];
    expect(rollupByCategory(holdings)).toEqual([{ category: "Watches", value: 100 }]);
  });
});

describe("rollupByItem", () => {
  it("emits one bar per item, sorted desc by value", () => {
    const holdings = [
      makeCollectible({ id: "h1", symbol: "Small", collectibleType: "watch", baseValue: 5_000 }),
      makeCollectible({ id: "h2", symbol: "Big", collectibleType: "art", baseValue: 120_000 }),
      makeCollectible({ id: "h3", symbol: "Medium", collectibleType: "watch", baseValue: 35_000 }),
    ];
    const rows = rollupByItem(holdings);
    expect(rows.map((r) => r.label)).toEqual(["Big", "Medium", "Small"]);
    expect(rows[0]?.category).toBe("Art");
    expect(rows[1]?.category).toBe("Watches");
    expect(rows[2]?.category).toBe("Watches");
  });

  it("falls back to symbol when name missing", () => {
    const h = makeCollectible({ id: "h", symbol: "ROLEX-001", baseValue: 1000 });
    if (h.instrument) h.instrument.name = null;
    const rows = rollupByItem([h]);
    expect(rows[0]?.label).toBe("ROLEX-001");
  });

  it("does NOT aggregate same-symbol — each item is its own bar", () => {
    // Two "Rolex Daytona" = two distinct watches.
    const holdings = [
      makeCollectible({ id: "h1", symbol: "Rolex Daytona", collectibleType: "watch", baseValue: 30_000 }),
      makeCollectible({ id: "h2", symbol: "Rolex Daytona", collectibleType: "watch", baseValue: 35_000 }),
    ];
    expect(rollupByItem(holdings)).toHaveLength(2);
  });

  it("preserves holdingId for tap-row navigation", () => {
    const h = makeCollectible({ id: "holding_unique", symbol: "X", collectibleType: "watch", baseValue: 1000 });
    expect(rollupByItem([h])[0]?.holdingId).toBe("holding_unique");
  });

  it("non-collectible holdings skipped", () => {
    const h = makeCollectible({ id: "h", symbol: "AAPL", baseValue: 100, assetKind: "INVESTMENT" });
    expect(rollupByItem([h])).toEqual([]);
  });
});

describe("totalCollectiblesExposure", () => {
  it("sums collectibles only", () => {
    const holdings = [
      makeCollectible({ id: "h1", symbol: "Rolex", collectibleType: "watch", baseValue: 30_000 }),
      makeCollectible({ id: "h2", symbol: "AAPL", baseValue: 500_000, assetKind: "INVESTMENT" }),
      makeCollectible({ id: "h3", symbol: "Banksy", collectibleType: "art", baseValue: 120_000 }),
    ];
    expect(totalCollectiblesExposure(holdings)).toBe(150_000);
  });

  it("empty input returns 0", () => {
    expect(totalCollectiblesExposure([])).toBe(0);
  });
});
