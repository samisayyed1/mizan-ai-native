/**
 * Tests for Track B PR-B1 Sukuks-panel rollup helpers.
 *
 * Pure-math tests — no React, no Tauri bridge. The §23 fixture is
 * the anchor: Emaar / Dar al Arkan / Sobha Sukuks split across 2026
 * + 2027 maturities.
 */
import { describe, expect, it } from "vitest";

import type { AssetClassifications, Holding, Instrument, TaxonomyCategory } from "@/lib/types";

import {
  extractMaturityYear,
  isBondHolding,
  rollupByIssuer,
  rollupByMaturityYear,
  totalBondExposure,
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

function makeClassifications(assetTypeKey: string): AssetClassifications {
  return {
    assetType: makeAssetType(assetTypeKey),
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
  maturityDate?: string,
): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name,
    currency: "USD",
    quoteMode: "MARKET",
    classifications: assetTypeKey ? makeClassifications(assetTypeKey) : null,
  };
  // Match the same `unknown` cast extractMaturityYear uses to read
  // bond metadata. Frontend Instrument type doesn't expose `metadata`
  // directly; PR-B1.a will thread it through cleanly.
  if (maturityDate) {
    (base as unknown as { metadata: { bond: { maturityDate: string } } }).metadata = {
      bond: { maturityDate },
    };
  }
  return base;
}

function makeHolding(opts: {
  id: string;
  symbol: string;
  name?: string | null;
  assetTypeKey?: string | null;
  maturityDate?: string;
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
      opts.maturityDate,
    ),
  };
}

describe("isBondHolding", () => {
  it.each([
    ["BOND_GOVERNMENT", true],
    ["BOND_CORPORATE", true],
    ["DEBT_SECURITY", true],
    ["MONEY_MARKET_DEBT", true],
    ["SUKUK", true],
    ["EQUITY", false],
    ["CRYPTOCURRENCY", false],
    [null, false],
  ])("for asset key %s returns %s", (key, expected) => {
    const h = makeHolding({
      id: "h",
      symbol: "X",
      assetTypeKey: key,
      baseValue: 100_000,
    });
    expect(isBondHolding(h)).toBe(expected);
  });

  it("returns false when no instrument", () => {
    const h = makeHolding({ id: "h", symbol: "X", baseValue: 100 });
    h.instrument = null;
    expect(isBondHolding(h)).toBe(false);
  });

  it("lowercase asset type key still recognized", () => {
    const h = makeHolding({ id: "h", symbol: "X", assetTypeKey: "sukuk", baseValue: 100 });
    expect(isBondHolding(h)).toBe(true);
  });
});

describe("rollupByIssuer", () => {
  it("returns empty for no holdings", () => {
    expect(rollupByIssuer([])).toEqual([]);
  });

  it("skips non-bond holdings", () => {
    const h = makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 1_000 });
    expect(rollupByIssuer([h])).toEqual([]);
  });

  it("§23 fixture: emits Emaar/DAR/Sobha sorted by exposure descending", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "EMAAR", name: "Emaar Sukuk", assetTypeKey: "SUKUK", baseValue: 300_000 }),
      makeHolding({ id: "h2", symbol: "DAR", name: "Dar al Arkan Sukuk", assetTypeKey: "SUKUK", baseValue: 200_000 }),
      makeHolding({ id: "h3", symbol: "SOBHA", name: "Sobha Sukuk", assetTypeKey: "SUKUK", baseValue: 150_000 }),
    ];
    const rows = rollupByIssuer(holdings);
    expect(rows).toEqual([
      { issuer: "Emaar Sukuk", exposureBase: 300_000 },
      { issuer: "Dar al Arkan Sukuk", exposureBase: 200_000 },
      { issuer: "Sobha Sukuk", exposureBase: 150_000 },
    ]);
  });

  it("aggregates duplicate symbols into one row", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "EMAAR", name: "Emaar", assetTypeKey: "SUKUK", baseValue: 100_000 }),
      makeHolding({ id: "h2", symbol: "EMAAR", name: "Emaar", assetTypeKey: "SUKUK", baseValue: 50_000 }),
    ];
    const rows = rollupByIssuer(holdings);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.exposureBase).toBe(150_000);
  });

  it("falls back to symbol when name is empty", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "TSLA-NOTE", name: "", assetTypeKey: "BOND_CORPORATE", baseValue: 10_000 }),
    ];
    expect(rollupByIssuer(holdings)[0]?.issuer).toBe("TSLA-NOTE");
  });

  it("skips zero/negative exposure", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "X", assetTypeKey: "SUKUK", baseValue: 0 }),
      makeHolding({ id: "h2", symbol: "Y", assetTypeKey: "SUKUK", baseValue: -100 }),
      makeHolding({ id: "h3", symbol: "Z", name: "Z", assetTypeKey: "SUKUK", baseValue: 100 }),
    ];
    expect(rollupByIssuer(holdings).map((r) => r.issuer)).toEqual(["Z"]);
  });
});

describe("extractMaturityYear", () => {
  it("extracts year from ISO date", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "X",
      assetTypeKey: "SUKUK",
      maturityDate: "2027-07-13",
    });
    expect(extractMaturityYear(h)).toBe(2027);
  });

  it("returns null on missing metadata", () => {
    const h = makeHolding({ id: "h1", symbol: "X", assetTypeKey: "SUKUK" });
    expect(extractMaturityYear(h)).toBeNull();
  });

  it("returns null on non-numeric year prefix", () => {
    const h = makeHolding({
      id: "h1",
      symbol: "X",
      assetTypeKey: "SUKUK",
      maturityDate: "junk",
    });
    expect(extractMaturityYear(h)).toBeNull();
  });
});

describe("rollupByMaturityYear", () => {
  it("§23 fixture: emits per-year bars sorted ascending", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "EMAAR", assetTypeKey: "SUKUK", baseValue: 188_000, maturityDate: "2026-07-13" }),
      makeHolding({ id: "h2", symbol: "DAR", assetTypeKey: "SUKUK", baseValue: 200_000, maturityDate: "2027-03-15" }),
      makeHolding({ id: "h3", symbol: "SOBHA", assetTypeKey: "SUKUK", baseValue: 150_000, maturityDate: "2027-11-20" }),
    ];
    const rows = rollupByMaturityYear(holdings);
    expect(rows).toEqual([
      { year: 2026, exposureBase: 188_000 },
      { year: 2027, exposureBase: 350_000 },
    ]);
  });

  it("skips holdings without maturity_date", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "EMAAR", assetTypeKey: "SUKUK", baseValue: 100_000 }),
    ];
    expect(rollupByMaturityYear(holdings)).toEqual([]);
  });

  it("skips non-bond holdings", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 100_000, maturityDate: "2030-01-01" }),
    ];
    expect(rollupByMaturityYear(holdings)).toEqual([]);
  });
});

describe("totalBondExposure", () => {
  it("sums base-currency value across bond holdings only", () => {
    const holdings = [
      makeHolding({ id: "h1", symbol: "EMAAR", assetTypeKey: "SUKUK", baseValue: 300_000 }),
      makeHolding({ id: "h2", symbol: "AAPL", assetTypeKey: "EQUITY", baseValue: 1_000_000 }),
      makeHolding({ id: "h3", symbol: "DAR", assetTypeKey: "SUKUK", baseValue: 200_000 }),
    ];
    expect(totalBondExposure(holdings)).toBe(500_000);
  });

  it("returns 0 for empty input", () => {
    expect(totalBondExposure([])).toBe(0);
  });
});
