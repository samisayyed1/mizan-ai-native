/**
 * Tests for Track B PR-B11 Real Estate rollup helpers.
 *
 * §23 fixture: Singapore Bukit Batok (Residence) + 3 Hyderabad
 * rentals + 1 Hyderabad held-for-sale (Land).
 */
import { describe, expect, it } from "vitest";

import type { Holding, Instrument } from "@/lib/types";

import {
  isRealEstateHolding,
  rollupByIntent,
  rollupByProperty,
  totalRealEstateExposure,
} from "./rollup";

function makeInstrument(symbol: string, propertyType?: string): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MANUAL",
    classifications: null,
  };
  if (propertyType) {
    (base as unknown as { metadata: { property: { propertyType: string } } }).metadata = {
      property: { propertyType },
    };
  }
  return base;
}

function makeProperty(opts: {
  id: string;
  symbol: string;
  propertyType?: string;
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
    assetKind: opts.assetKind ?? "PROPERTY",
    instrument: makeInstrument(opts.symbol, opts.propertyType),
  };
}

describe("isRealEstateHolding", () => {
  it("returns true for assetKind PROPERTY", () => {
    const h = makeProperty({ id: "h1", symbol: "BB", baseValue: 100 });
    expect(isRealEstateHolding(h)).toBe(true);
  });

  it("returns false for INVESTMENT / VEHICLE / OTHER", () => {
    for (const kind of ["INVESTMENT", "VEHICLE", "OTHER", "PRIVATE_EQUITY"] as const) {
      const h = makeProperty({ id: `h-${kind}`, symbol: "X", baseValue: 100, assetKind: kind });
      expect(isRealEstateHolding(h)).toBe(false);
    }
  });

  it("returns false when assetKind is missing", () => {
    const h = makeProperty({ id: "h", symbol: "X", baseValue: 100 });
    delete (h as { assetKind?: unknown }).assetKind;
    expect(isRealEstateHolding(h)).toBe(false);
  });
});

describe("rollupByIntent", () => {
  it("§23 fixture: classifies Residence/Rental/Land/Other correctly", () => {
    const holdings = [
      makeProperty({ id: "h1", symbol: "Bukit Batok", propertyType: "residence", baseValue: 1_400_000 }),
      makeProperty({ id: "h2", symbol: "Hyd Rental A", propertyType: "rental", baseValue: 250_000 }),
      makeProperty({ id: "h3", symbol: "Hyd Rental B", propertyType: "rental", baseValue: 300_000 }),
      makeProperty({ id: "h4", symbol: "Hyd Rental C", propertyType: "rental", baseValue: 200_000 }),
      makeProperty({ id: "h5", symbol: "Hyd For Sale", propertyType: "land", baseValue: 600_000 }),
    ];
    const rows = rollupByIntent(holdings);
    expect(rows).toEqual([
      { intent: "Residence", value: 1_400_000 },
      { intent: "Rental", value: 750_000 },
      { intent: "Land", value: 600_000 },
    ]);
  });

  it("returns empty for no real-estate holdings", () => {
    const h = makeProperty({ id: "h", symbol: "AAPL", baseValue: 100, assetKind: "INVESTMENT" });
    expect(rollupByIntent([h])).toEqual([]);
  });

  it("maps missing/unknown propertyType to 'Other'", () => {
    const h = makeProperty({ id: "h", symbol: "X", baseValue: 1000 });
    const rows = rollupByIntent([h]);
    expect(rows).toEqual([{ intent: "Other", value: 1000 }]);
  });

  it("case-insensitive propertyType (residence vs Residence)", () => {
    const holdings = [
      makeProperty({ id: "h1", symbol: "A", propertyType: "Residence", baseValue: 100 }),
      makeProperty({ id: "h2", symbol: "B", propertyType: "RESIDENCE", baseValue: 200 }),
      makeProperty({ id: "h3", symbol: "C", propertyType: "residence", baseValue: 300 }),
    ];
    expect(rollupByIntent(holdings)).toEqual([{ intent: "Residence", value: 600 }]);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makeProperty({ id: "h1", symbol: "X", propertyType: "rental", baseValue: 0 }),
      makeProperty({ id: "h2", symbol: "Y", propertyType: "rental", baseValue: -100 }),
      makeProperty({ id: "h3", symbol: "Z", propertyType: "rental", baseValue: 100 }),
    ];
    expect(rollupByIntent(holdings)).toEqual([{ intent: "Rental", value: 100 }]);
  });

  it("classifies commercial", () => {
    const h = makeProperty({ id: "h", symbol: "OFFICE", propertyType: "commercial", baseValue: 500_000 });
    expect(rollupByIntent([h])).toEqual([{ intent: "Commercial", value: 500_000 }]);
  });
});

describe("rollupByProperty", () => {
  it("emits one bar per property, sorted desc by value", () => {
    const holdings = [
      makeProperty({ id: "h1", symbol: "Small", propertyType: "rental", baseValue: 100_000 }),
      makeProperty({ id: "h2", symbol: "Big", propertyType: "residence", baseValue: 1_400_000 }),
      makeProperty({ id: "h3", symbol: "Medium", propertyType: "rental", baseValue: 300_000 }),
    ];
    const rows = rollupByProperty(holdings);
    expect(rows.map((r) => r.label)).toEqual(["Big", "Medium", "Small"]);
    expect(rows[0]?.intent).toBe("Residence");
    expect(rows[1]?.intent).toBe("Rental");
    expect(rows[2]?.intent).toBe("Rental");
  });

  it("falls back to symbol when name missing", () => {
    const h = makeProperty({ id: "h", symbol: "BB-RES-001", baseValue: 1000 });
    if (h.instrument) h.instrument.name = null;
    const rows = rollupByProperty([h]);
    expect(rows[0]?.label).toBe("BB-RES-001");
  });

  it("skips non-property holdings", () => {
    const h = makeProperty({ id: "h", symbol: "AAPL", baseValue: 1000, assetKind: "INVESTMENT" });
    expect(rollupByProperty([h])).toEqual([]);
  });

  it("does NOT aggregate same-symbol — each property is its own bar", () => {
    // Properties don't aggregate like equity symbols would; same name
    // = distinct properties (e.g. two Hyderabad rentals on the same
    // street).
    const holdings = [
      makeProperty({ id: "h1", symbol: "Hyderabad Rental", propertyType: "rental", baseValue: 250_000 }),
      makeProperty({ id: "h2", symbol: "Hyderabad Rental", propertyType: "rental", baseValue: 280_000 }),
    ];
    const rows = rollupByProperty(holdings);
    expect(rows).toHaveLength(2);
    expect(rows[0]?.value).toBe(280_000);
    expect(rows[1]?.value).toBe(250_000);
  });

  it("preserves holdingId for tap-row navigation", () => {
    const h = makeProperty({ id: "holding_unique_id", symbol: "X", propertyType: "rental", baseValue: 1000 });
    const rows = rollupByProperty([h]);
    expect(rows[0]?.holdingId).toBe("holding_unique_id");
  });
});

describe("totalRealEstateExposure", () => {
  it("sums real-estate holdings only", () => {
    const holdings = [
      makeProperty({ id: "h1", symbol: "BB", propertyType: "residence", baseValue: 1_400_000 }),
      makeProperty({ id: "h2", symbol: "AAPL", baseValue: 500_000, assetKind: "INVESTMENT" }),
      makeProperty({ id: "h3", symbol: "Hyd", propertyType: "rental", baseValue: 250_000 }),
    ];
    expect(totalRealEstateExposure(holdings)).toBe(1_650_000);
  });

  it("returns 0 for empty input", () => {
    expect(totalRealEstateExposure([])).toBe(0);
  });
});
