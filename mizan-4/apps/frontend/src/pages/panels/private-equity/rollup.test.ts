/**
 * Tests for Track B PR-B7 Private Equity rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type { Holding, Instrument } from "@/lib/types";

import {
  extractVintageYear,
  isPrivateEquityHolding,
  rollupByPosition,
  rollupByVintage,
  totalPrivateEquityExposure,
} from "./rollup";

function makeInstrument(
  symbol: string,
  metadata?: Record<string, unknown>,
): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MANUAL",
    classifications: null,
  };
  if (metadata) {
    (base as unknown as { metadata: Record<string, unknown> }).metadata = metadata;
  }
  return base;
}

function makePosition(opts: {
  id: string;
  symbol: string;
  purchaseDate?: string;
  scope?: "pe" | "generic";
  baseValue?: number;
  assetKind?: Holding["assetKind"];
}): Holding {
  const metadata = opts.purchaseDate
    ? opts.scope === "generic"
      ? { purchaseDate: opts.purchaseDate }
      : { privateEquity: { purchaseDate: opts.purchaseDate } }
    : undefined;
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
    assetKind: opts.assetKind ?? "PRIVATE_EQUITY",
    instrument: makeInstrument(opts.symbol, metadata),
  };
}

describe("isPrivateEquityHolding", () => {
  it("returns true for PRIVATE_EQUITY", () => {
    const h = makePosition({ id: "h", symbol: "Hasan VC", baseValue: 100_000 });
    expect(isPrivateEquityHolding(h)).toBe(true);
  });

  it("returns false for other assetKinds", () => {
    for (const kind of ["INVESTMENT", "PROPERTY", "COLLECTIBLE", "VEHICLE"] as const) {
      const h = makePosition({ id: `h-${kind}`, symbol: "X", baseValue: 100, assetKind: kind });
      expect(isPrivateEquityHolding(h)).toBe(false);
    }
  });

  it("returns false when assetKind missing", () => {
    const h = makePosition({ id: "h", symbol: "X", baseValue: 100 });
    delete (h as { assetKind?: unknown }).assetKind;
    expect(isPrivateEquityHolding(h)).toBe(false);
  });
});

describe("extractVintageYear", () => {
  it("extracts year from privateEquity.purchaseDate", () => {
    const h = makePosition({
      id: "h",
      symbol: "Hasan VC",
      purchaseDate: "2022-07-15",
      scope: "pe",
      baseValue: 100,
    });
    expect(extractVintageYear(h)).toBe(2022);
  });

  it("falls back to generic metadata.purchaseDate", () => {
    const h = makePosition({
      id: "h",
      symbol: "HAPL",
      purchaseDate: "2021-03-01",
      scope: "generic",
      baseValue: 100,
    });
    expect(extractVintageYear(h)).toBe(2021);
  });

  it("prefers privateEquity.purchaseDate over generic when both present", () => {
    const h = makePosition({ id: "h", symbol: "X", baseValue: 1 });
    if (h.instrument) {
      (h.instrument as unknown as { metadata: Record<string, unknown> }).metadata = {
        privateEquity: { purchaseDate: "2024-01-01" },
        purchaseDate: "2020-01-01",
      };
    }
    expect(extractVintageYear(h)).toBe(2024);
  });

  it("returns null on missing metadata", () => {
    const h = makePosition({ id: "h", symbol: "X", baseValue: 100 });
    expect(extractVintageYear(h)).toBeNull();
  });

  it("returns null on unparseable year", () => {
    const h = makePosition({
      id: "h",
      symbol: "X",
      purchaseDate: "junk-date",
      scope: "pe",
      baseValue: 100,
    });
    expect(extractVintageYear(h)).toBeNull();
  });
});

describe("rollupByVintage", () => {
  it("§23 fixture: groups Hasan VC + HAPL + Wholesum + Stake by vintage, ascending", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "Hasan VC", purchaseDate: "2022-07-15", scope: "pe", baseValue: 250_000 }),
      makePosition({ id: "h2", symbol: "HAPL", purchaseDate: "2021-03-01", scope: "pe", baseValue: 100_000 }),
      makePosition({ id: "h3", symbol: "Wholesum", purchaseDate: "2022-11-20", scope: "pe", baseValue: 150_000 }),
      makePosition({ id: "h4", symbol: "Stake", purchaseDate: "2023-05-10", scope: "pe", baseValue: 75_000 }),
    ];
    const rows = rollupByVintage(holdings);
    expect(rows).toEqual([
      { year: 2021, exposureBase: 100_000 },
      { year: 2022, exposureBase: 400_000 },
      { year: 2023, exposureBase: 75_000 },
    ]);
  });

  it("skips holdings without parseable vintage", () => {
    const h = makePosition({ id: "h", symbol: "Hasan VC", baseValue: 100_000 });
    expect(rollupByVintage([h])).toEqual([]);
  });

  it("skips non-PE holdings", () => {
    const h = makePosition({
      id: "h",
      symbol: "AAPL",
      purchaseDate: "2020-01-01",
      scope: "pe",
      baseValue: 100,
      assetKind: "INVESTMENT",
    });
    expect(rollupByVintage([h])).toEqual([]);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "X", purchaseDate: "2022-01-01", scope: "pe", baseValue: 0 }),
      makePosition({ id: "h2", symbol: "Y", purchaseDate: "2022-01-01", scope: "pe", baseValue: -10 }),
      makePosition({ id: "h3", symbol: "Z", purchaseDate: "2022-01-01", scope: "pe", baseValue: 100 }),
    ];
    expect(rollupByVintage(holdings)).toEqual([{ year: 2022, exposureBase: 100 }]);
  });
});

describe("rollupByPosition", () => {
  it("emits one bar per position, sorted desc by value, with vintage chip", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "Small", purchaseDate: "2023-01-01", scope: "pe", baseValue: 25_000 }),
      makePosition({ id: "h2", symbol: "Big", purchaseDate: "2021-01-01", scope: "pe", baseValue: 500_000 }),
      makePosition({ id: "h3", symbol: "Medium", baseValue: 100_000 }), // no vintage
    ];
    const rows = rollupByPosition(holdings);
    expect(rows.map((r) => r.label)).toEqual(["Big", "Medium", "Small"]);
    expect(rows[0]?.vintage).toBe(2021);
    expect(rows[1]?.vintage).toBeNull();
    expect(rows[2]?.vintage).toBe(2023);
  });

  it("name→symbol fallback when name missing", () => {
    const h = makePosition({ id: "h", symbol: "HASAN-VC-001", baseValue: 1000 });
    if (h.instrument) h.instrument.name = null;
    expect(rollupByPosition([h])[0]?.label).toBe("HASAN-VC-001");
  });

  it("does NOT aggregate same-symbol — distinct funds stay distinct", () => {
    // Two HAPL drawdowns in different vintages → two distinct rows.
    const holdings = [
      makePosition({ id: "h1", symbol: "HAPL", purchaseDate: "2021-01-01", scope: "pe", baseValue: 100_000 }),
      makePosition({ id: "h2", symbol: "HAPL", purchaseDate: "2023-01-01", scope: "pe", baseValue: 50_000 }),
    ];
    expect(rollupByPosition(holdings)).toHaveLength(2);
  });

  it("non-PE holdings skipped", () => {
    const h = makePosition({
      id: "h",
      symbol: "AAPL",
      baseValue: 100,
      assetKind: "INVESTMENT",
    });
    expect(rollupByPosition([h])).toEqual([]);
  });

  it("preserves holdingId for nav", () => {
    const h = makePosition({ id: "holding_unique", symbol: "X", baseValue: 1000 });
    expect(rollupByPosition([h])[0]?.holdingId).toBe("holding_unique");
  });
});

describe("totalPrivateEquityExposure", () => {
  it("sums PE-only holdings", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "Hasan VC", purchaseDate: "2022-01-01", scope: "pe", baseValue: 250_000 }),
      makePosition({ id: "h2", symbol: "AAPL", baseValue: 500_000, assetKind: "INVESTMENT" }),
      makePosition({ id: "h3", symbol: "HAPL", baseValue: 100_000 }),
    ];
    expect(totalPrivateEquityExposure(holdings)).toBe(350_000);
  });

  it("empty input returns 0", () => {
    expect(totalPrivateEquityExposure([])).toBe(0);
  });
});
