import { describe, expect, it } from "vitest";
import {
  AssetClass,
  ASSET_CLASS_ORDER,
  assetClassColor,
  classifyHolding,
  groupHoldingsByAssetClass,
  parseAssetClassParam,
  partitionBuckets,
  scaleHistoryByWeight,
  sumBucketGain,
  type ValuationHistoryRow,
} from "./asset-classes";
import { AssetKind, HoldingType, QuoteMode } from "./constants";
import type { AssetClassifications, Holding, Instrument, TaxonomyCategory } from "./types";

// ---------------------------------------------------------------------------
// Test factories — keep the per-test boilerplate small
// ---------------------------------------------------------------------------

function makeTaxonomyCategory(over: Partial<TaxonomyCategory> = {}): TaxonomyCategory {
  return {
    id: over.id ?? "cat-1",
    taxonomyId: over.taxonomyId ?? "tax-asset-type",
    parentId: over.parentId ?? null,
    name: over.name ?? "Stocks",
    key: over.key ?? "stocks",
    color: over.color ?? "#000",
    description: over.description ?? null,
    sortOrder: over.sortOrder ?? 0,
    createdAt: over.createdAt ?? "2026-01-01T00:00:00Z",
    updatedAt: over.updatedAt ?? "2026-01-01T00:00:00Z",
  };
}

function makeClassifications(over: Partial<AssetClassifications> = {}): AssetClassifications {
  return {
    assetType: over.assetType ?? null,
    riskCategory: over.riskCategory ?? null,
    assetClasses: over.assetClasses ?? [],
    sectors: over.sectors ?? [],
    regions: over.regions ?? [],
    customGroups: over.customGroups ?? [],
  };
}

function makeInstrument(over: Partial<Instrument> = {}): Instrument {
  return {
    id: over.id ?? "inst-1",
    symbol: over.symbol ?? "TEST",
    name: over.name ?? "Test Instrument",
    currency: over.currency ?? "USD",
    notes: over.notes ?? null,
    quoteMode: over.quoteMode ?? QuoteMode.MARKET,
    preferredProvider: over.preferredProvider ?? null,
    classifications: over.classifications ?? null,
  };
}

function makeHolding(over: Partial<Holding> = {}): Holding {
  return {
    id: over.id ?? "h-1",
    holdingType: over.holdingType ?? HoldingType.SECURITY,
    accountId: over.accountId ?? "acc-1",
    // `instrument` and `assetKind` need explicit `in` checks because
    // `??` would replace a deliberate `null` override with the default
    // — and the "null assetKind falls through to OTHER" branch relies
    // on actually passing through a null.
    instrument: "instrument" in over ? (over.instrument ?? null) : null,
    assetKind: "assetKind" in over ? (over.assetKind ?? null) : AssetKind.INVESTMENT,
    quantity: over.quantity ?? 1,
    openDate: over.openDate ?? null,
    lots: over.lots ?? null,
    localCurrency: over.localCurrency ?? "USD",
    baseCurrency: over.baseCurrency ?? "USD",
    fxRate: over.fxRate ?? 1,
    marketValue: over.marketValue ?? { local: 100, base: 100 },
    costBasis: over.costBasis ?? null,
    price: over.price ?? null,
    unrealizedGain: over.unrealizedGain ?? null,
    unrealizedGainPct: over.unrealizedGainPct ?? null,
    realizedGain: over.realizedGain ?? null,
    realizedGainPct: over.realizedGainPct ?? null,
    totalGain: over.totalGain ?? null,
    totalGainPct: over.totalGainPct ?? null,
    dayChange: over.dayChange ?? null,
    dayChangePct: over.dayChangePct ?? null,
    prevCloseValue: over.prevCloseValue ?? null,
    weight: over.weight ?? 0,
    asOfDate: over.asOfDate ?? "2026-05-17",
  };
}

// ---------------------------------------------------------------------------
// classifyHolding
// ---------------------------------------------------------------------------

describe("classifyHolding", () => {
  describe("cash → Bank Accounts (Feroz #17)", () => {
    it("routes a cash holding to BANK_ACCOUNTS regardless of assetKind", () => {
      const h = makeHolding({ holdingType: HoldingType.CASH, assetKind: AssetKind.INVESTMENT });
      expect(classifyHolding(h)).toBe(AssetClass.BANK_ACCOUNTS);
    });

    it("routes cash holdings with null assetKind to BANK_ACCOUNTS", () => {
      const h = makeHolding({ holdingType: HoldingType.CASH, assetKind: null });
      expect(classifyHolding(h)).toBe(AssetClass.BANK_ACCOUNTS);
    });
  });

  describe("alternative assetKinds map directly", () => {
    it("PROPERTY → Property", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.PROPERTY }))).toBe(
        AssetClass.PROPERTY,
      );
    });

    it("COLLECTIBLE → Collectibles", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.COLLECTIBLE }))).toBe(
        AssetClass.COLLECTIBLES,
      );
    });

    it("PRECIOUS_METAL → Precious Metals", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.PRECIOUS_METAL }))).toBe(
        AssetClass.PRECIOUS_METALS,
      );
    });

    it("VEHICLE → OTHER (will be removed from net worth per Feroz #14)", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.VEHICLE }))).toBe(AssetClass.OTHER);
    });

    it("LIABILITY → OTHER (liabilities will get their own section per Feroz #20)", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.LIABILITY }))).toBe(
        AssetClass.OTHER,
      );
    });

    it("FX → OTHER", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.FX }))).toBe(AssetClass.OTHER);
    });

    it("OTHER assetKind → OTHER class", () => {
      expect(classifyHolding(makeHolding({ assetKind: AssetKind.OTHER }))).toBe(AssetClass.OTHER);
    });
  });

  describe("INVESTMENT — taxonomy-driven classification", () => {
    function withAssetTypeKey(key: string): Holding {
      return makeHolding({
        assetKind: AssetKind.INVESTMENT,
        instrument: makeInstrument({
          classifications: makeClassifications({
            assetType: makeTaxonomyCategory({ key, name: key }),
          }),
        }),
      });
    }

    it("'stocks' key → STOCKS", () => {
      expect(classifyHolding(withAssetTypeKey("stocks"))).toBe(AssetClass.STOCKS);
    });

    it("'equity' key → STOCKS", () => {
      expect(classifyHolding(withAssetTypeKey("equity"))).toBe(AssetClass.STOCKS);
    });

    it("'etf' key → ETFS", () => {
      expect(classifyHolding(withAssetTypeKey("etf"))).toBe(AssetClass.ETFS);
    });

    it("'exchange-traded fund' → ETFS", () => {
      expect(classifyHolding(withAssetTypeKey("Exchange-Traded Fund"))).toBe(AssetClass.ETFS);
    });

    it("'sukuk' key → SUKUKS", () => {
      expect(classifyHolding(withAssetTypeKey("sukuk"))).toBe(AssetClass.SUKUKS);
    });

    it("'bond' key → BONDS", () => {
      expect(classifyHolding(withAssetTypeKey("bond"))).toBe(AssetClass.BONDS);
    });

    it("'fixed income' → BONDS", () => {
      expect(classifyHolding(withAssetTypeKey("fixed-income"))).toBe(AssetClass.BONDS);
    });

    it("'treasury' → BONDS", () => {
      expect(classifyHolding(withAssetTypeKey("US Treasury"))).toBe(AssetClass.BONDS);
    });

    it("'gilt' → BONDS", () => {
      expect(classifyHolding(withAssetTypeKey("UK Gilt"))).toBe(AssetClass.BONDS);
    });

    it("SUKUK keyword wins over BOND keyword when both could match", () => {
      // A 'sukuk' is sometimes described as an Islamic bond — make sure
      // the SUKUK rule fires first so it doesn't get mis-bucketed.
      expect(classifyHolding(withAssetTypeKey("Sukuk (Islamic Bond)"))).toBe(AssetClass.SUKUKS);
    });

    it("ETF keyword wins over BOND keyword when both could match", () => {
      // "Bond ETF" must classify as ETF, not Bond.
      expect(classifyHolding(withAssetTypeKey("Aggregate Bond ETF"))).toBe(AssetClass.ETFS);
    });

    it("ETF keyword wins over STOCK keyword when both could match", () => {
      expect(classifyHolding(withAssetTypeKey("Equity ETF"))).toBe(AssetClass.ETFS);
    });
  });

  describe("INVESTMENT — fallback behavior", () => {
    it("INVESTMENT with no instrument → STOCKS", () => {
      const h = makeHolding({ assetKind: AssetKind.INVESTMENT, instrument: null });
      expect(classifyHolding(h)).toBe(AssetClass.STOCKS);
    });

    it("INVESTMENT with instrument but no classifications → STOCKS", () => {
      const h = makeHolding({
        assetKind: AssetKind.INVESTMENT,
        instrument: makeInstrument({ symbol: "AAPL", name: "Apple Inc.", classifications: null }),
      });
      expect(classifyHolding(h)).toBe(AssetClass.STOCKS);
    });

    it("PRIVATE_EQUITY with no classifications → STOCKS", () => {
      const h = makeHolding({ assetKind: AssetKind.PRIVATE_EQUITY, instrument: null });
      expect(classifyHolding(h)).toBe(AssetClass.STOCKS);
    });

    it("INVESTMENT with empty classifications → STOCKS", () => {
      const h = makeHolding({
        assetKind: AssetKind.INVESTMENT,
        instrument: makeInstrument({
          symbol: "AAPL",
          name: "Apple Inc.",
          classifications: makeClassifications(),
        }),
      });
      expect(classifyHolding(h)).toBe(AssetClass.STOCKS);
    });

    it("falls through to assetClasses[] when assetType has no signal", () => {
      const h = makeHolding({
        assetKind: AssetKind.INVESTMENT,
        instrument: makeInstrument({
          symbol: "AGG",
          name: "Some Index",
          classifications: makeClassifications({
            assetType: makeTaxonomyCategory({ key: "index", name: "Index Fund" }),
            assetClasses: [
              {
                category: makeTaxonomyCategory({ key: "bond", name: "Bond" }),
                topLevelCategory: { id: "tlc", name: "Bond" },
                weight: 100,
              },
            ],
          }),
        }),
      });
      expect(classifyHolding(h)).toBe(AssetClass.BONDS);
    });
  });

  describe("null assetKind", () => {
    it("security with null assetKind → OTHER (no INVESTMENT default to anchor on)", () => {
      const h = makeHolding({ assetKind: null });
      expect(classifyHolding(h)).toBe(AssetClass.OTHER);
    });
  });
});

// ---------------------------------------------------------------------------
// groupHoldingsByAssetClass
// ---------------------------------------------------------------------------

describe("groupHoldingsByAssetClass", () => {
  it("returns ALL classes in ASSET_CLASS_ORDER, even empty ones (Feroz: empty class shows Add CTA)", () => {
    const buckets = groupHoldingsByAssetClass([]);
    expect(buckets.map((b) => b.cls)).toEqual([...ASSET_CLASS_ORDER]);
    expect(buckets.every((b) => b.count === 0 && b.totalValue === 0)).toBe(true);
  });

  it("sums market value in base currency per bucket", () => {
    const holdings = [
      makeHolding({
        id: "a",
        assetKind: AssetKind.PROPERTY,
        marketValue: { local: 500_000, base: 500_000 },
      }),
      makeHolding({
        id: "b",
        assetKind: AssetKind.PROPERTY,
        marketValue: { local: 300_000, base: 300_000 },
      }),
      makeHolding({
        id: "c",
        holdingType: HoldingType.CASH,
        marketValue: { local: 10_000, base: 10_000 },
      }),
    ];
    const buckets = groupHoldingsByAssetClass(holdings);
    const property = buckets.find((b) => b.cls === AssetClass.PROPERTY)!;
    const bank = buckets.find((b) => b.cls === AssetClass.BANK_ACCOUNTS)!;

    expect(property.count).toBe(2);
    expect(property.totalValue).toBe(800_000);
    expect(bank.count).toBe(1);
    expect(bank.totalValue).toBe(10_000);
  });

  it("preserves input order of holdings within a bucket", () => {
    const holdings = [
      makeHolding({ id: "first", assetKind: AssetKind.COLLECTIBLE }),
      makeHolding({ id: "second", assetKind: AssetKind.COLLECTIBLE }),
      makeHolding({ id: "third", assetKind: AssetKind.COLLECTIBLE }),
    ];
    const collectibles = groupHoldingsByAssetClass(holdings).find(
      (b) => b.cls === AssetClass.COLLECTIBLES,
    )!;
    expect(collectibles.holdings.map((h) => h.id)).toEqual(["first", "second", "third"]);
  });

  it("treats a null base market value as zero (defensive)", () => {
    const h = makeHolding({
      assetKind: AssetKind.PROPERTY,
      marketValue: { local: 0, base: 0 },
    });
    const buckets = groupHoldingsByAssetClass([h]);
    const property = buckets.find((b) => b.cls === AssetClass.PROPERTY)!;
    expect(property.totalValue).toBe(0);
    expect(property.count).toBe(1);
  });

  it("computes weightPercent as bucket totalValue ÷ portfolio total × 100", () => {
    const holdings = [
      makeHolding({
        id: "a",
        assetKind: AssetKind.PROPERTY,
        marketValue: { local: 750_000, base: 750_000 },
      }),
      makeHolding({
        id: "b",
        holdingType: HoldingType.CASH,
        marketValue: { local: 250_000, base: 250_000 },
      }),
    ];
    const buckets = groupHoldingsByAssetClass(holdings);
    const property = buckets.find((b) => b.cls === AssetClass.PROPERTY)!;
    const bank = buckets.find((b) => b.cls === AssetClass.BANK_ACCOUNTS)!;
    const stocks = buckets.find((b) => b.cls === AssetClass.STOCKS)!;
    expect(property.weightPercent).toBe(75);
    expect(bank.weightPercent).toBe(25);
    expect(stocks.weightPercent).toBe(0);
  });

  it("returns weightPercent=0 for every bucket when portfolio total is zero", () => {
    const buckets = groupHoldingsByAssetClass([]);
    expect(buckets.every((b) => b.weightPercent === 0)).toBe(true);
    expect(buckets.some((b) => !Number.isFinite(b.weightPercent))).toBe(false);
  });

  it("rolls up totalGain across holdings in base currency", () => {
    const holdings = [
      makeHolding({
        id: "a",
        assetKind: AssetKind.PROPERTY,
        totalGain: { local: 12_000, base: 12_000 },
      }),
      makeHolding({
        id: "b",
        assetKind: AssetKind.PROPERTY,
        totalGain: { local: -3_000, base: -3_000 },
      }),
    ];
    const property = groupHoldingsByAssetClass(holdings).find(
      (b) => b.cls === AssetClass.PROPERTY,
    )!;
    expect(property.totalGain).toBe(9_000);
  });

  it("returns totalGain=null when every holding has no gain data", () => {
    const holdings = [
      makeHolding({ id: "a", assetKind: AssetKind.COLLECTIBLE, totalGain: null }),
      makeHolding({ id: "b", assetKind: AssetKind.COLLECTIBLE, totalGain: null }),
    ];
    const collectibles = groupHoldingsByAssetClass(holdings).find(
      (b) => b.cls === AssetClass.COLLECTIBLES,
    )!;
    expect(collectibles.totalGain).toBeNull();
  });

  it("preserves the null-vs-zero distinction (one holding with 0 gain ⇒ total 0, not null)", () => {
    const holdings = [
      makeHolding({
        id: "a",
        assetKind: AssetKind.COLLECTIBLE,
        totalGain: { local: 0, base: 0 },
      }),
    ];
    const collectibles = groupHoldingsByAssetClass(holdings).find(
      (b) => b.cls === AssetClass.COLLECTIBLES,
    )!;
    expect(collectibles.totalGain).toBe(0);
  });

  it("ignores non-finite gain values (NaN, Infinity) defensively", () => {
    const holdings = [
      makeHolding({
        id: "a",
        assetKind: AssetKind.COLLECTIBLE,
        totalGain: { local: 100, base: 100 },
      }),
      makeHolding({
        id: "b",
        assetKind: AssetKind.COLLECTIBLE,
        // simulate a malformed row that slipped through the wire layer
        totalGain: { local: NaN, base: NaN },
      }),
    ];
    const collectibles = groupHoldingsByAssetClass(holdings).find(
      (b) => b.cls === AssetClass.COLLECTIBLES,
    )!;
    expect(collectibles.totalGain).toBe(100);
  });
});

// ---------------------------------------------------------------------------
// sumBucketGain — direct unit tests of the helper
// ---------------------------------------------------------------------------

describe("sumBucketGain", () => {
  it("returns { totalGain: null } for an empty list", () => {
    expect(sumBucketGain([])).toEqual({ totalGain: null });
  });

  it("returns null when every holding has a null totalGain", () => {
    const holdings = [makeHolding({ totalGain: null }), makeHolding({ id: "x", totalGain: null })];
    expect(sumBucketGain(holdings)).toEqual({ totalGain: null });
  });

  it("sums when at least one holding contributes", () => {
    const holdings = [
      makeHolding({ totalGain: { local: 50, base: 50 } }),
      makeHolding({ id: "x", totalGain: null }),
      makeHolding({ id: "y", totalGain: { local: 25, base: 25 } }),
    ];
    expect(sumBucketGain(holdings)).toEqual({ totalGain: 75 });
  });
});

// ---------------------------------------------------------------------------
// partitionBuckets
// ---------------------------------------------------------------------------

describe("partitionBuckets", () => {
  it("splits by count > 0, preserving input order within each partition", () => {
    const holdings = [
      makeHolding({ id: "p1", assetKind: AssetKind.PROPERTY }),
      makeHolding({ id: "c1", holdingType: HoldingType.CASH }),
    ];
    const buckets = groupHoldingsByAssetClass(holdings);
    const { active, empty } = partitionBuckets(buckets);

    expect(active.map((b) => b.cls)).toEqual([AssetClass.BANK_ACCOUNTS, AssetClass.PROPERTY]);
    expect(empty.map((b) => b.cls)).not.toContain(AssetClass.BANK_ACCOUNTS);
    expect(empty.map((b) => b.cls)).not.toContain(AssetClass.PROPERTY);
    // Empty partition is non-empty itself (every other class shows up there)
    expect(empty.length).toBe(ASSET_CLASS_ORDER.length - 2);
  });

  it("returns empty active + full empty when no holdings", () => {
    const buckets = groupHoldingsByAssetClass([]);
    const { active, empty } = partitionBuckets(buckets);
    expect(active).toHaveLength(0);
    expect(empty).toHaveLength(ASSET_CLASS_ORDER.length);
  });
});

// ---------------------------------------------------------------------------
// parseAssetClassParam
// ---------------------------------------------------------------------------

describe("parseAssetClassParam", () => {
  it("accepts exact upper-case values", () => {
    expect(parseAssetClassParam("STOCKS")).toBe(AssetClass.STOCKS);
    expect(parseAssetClassParam("BANK_ACCOUNTS")).toBe(AssetClass.BANK_ACCOUNTS);
  });

  it("is case-insensitive", () => {
    expect(parseAssetClassParam("stocks")).toBe(AssetClass.STOCKS);
    expect(parseAssetClassParam("Bank_Accounts")).toBe(AssetClass.BANK_ACCOUNTS);
  });

  it("returns null for unknown values", () => {
    expect(parseAssetClassParam("crypto")).toBeNull();
    expect(parseAssetClassParam("")).toBeNull();
    expect(parseAssetClassParam(null)).toBeNull();
    expect(parseAssetClassParam(undefined)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// scaleHistoryByWeight — constant-weight class history projection
// ---------------------------------------------------------------------------

describe("scaleHistoryByWeight", () => {
  function row(over: Partial<ValuationHistoryRow> = {}): ValuationHistoryRow {
    return {
      valuationDate: over.valuationDate ?? "2026-05-17",
      totalValue: over.totalValue ?? 100,
      netContribution: over.netContribution ?? 50,
      baseCurrency: "baseCurrency" in over ? (over.baseCurrency ?? null) : "USD",
    };
  }

  it("scales totalValue and netContribution by weightPercent / 100", () => {
    const history = [row({ valuationDate: "2026-05-17", totalValue: 1000, netContribution: 400 })];
    const out = scaleHistoryByWeight(history, 50, "USD");
    expect(out).toEqual([
      { date: "2026-05-17", totalValue: 500, netContribution: 200, currency: "USD" },
    ]);
  });

  it("preserves dates exactly across the input", () => {
    const history = [
      row({ valuationDate: "2026-01-01" }),
      row({ valuationDate: "2026-02-01" }),
      row({ valuationDate: "2026-03-01" }),
    ];
    const out = scaleHistoryByWeight(history, 25, "USD");
    expect(out.map((p) => p.date)).toEqual(["2026-01-01", "2026-02-01", "2026-03-01"]);
  });

  it("uses the row's baseCurrency when present", () => {
    const history = [row({ baseCurrency: "SGD" })];
    const out = scaleHistoryByWeight(history, 50, "USD");
    expect(out[0]?.currency).toBe("SGD");
  });

  it("falls back to defaultCurrency when the row has no currency", () => {
    const history = [row({ baseCurrency: null })];
    const out = scaleHistoryByWeight(history, 50, "USD");
    expect(out[0]?.currency).toBe("USD");
  });

  it("falls back to defaultCurrency when the row currency is an empty string", () => {
    const history = [row({ baseCurrency: "" })];
    const out = scaleHistoryByWeight(history, 50, "USD");
    expect(out[0]?.currency).toBe("USD");
  });

  it("returns [] when weightPercent is 0 — caller should hide the chart", () => {
    const history = [row()];
    expect(scaleHistoryByWeight(history, 0, "USD")).toEqual([]);
  });

  it("returns [] when weightPercent is not finite", () => {
    const history = [row()];
    expect(scaleHistoryByWeight(history, NaN, "USD")).toEqual([]);
    expect(scaleHistoryByWeight(history, Infinity, "USD")).toEqual([]);
    expect(scaleHistoryByWeight(history, -Infinity, "USD")).toEqual([]);
  });

  it("clamps negative weights to 0 (returns [])", () => {
    const history = [row()];
    expect(scaleHistoryByWeight(history, -10, "USD")).toEqual([]);
  });

  it("clamps weights >100 to 100% (returns input unchanged in value)", () => {
    const history = [row({ totalValue: 100, netContribution: 40 })];
    const out = scaleHistoryByWeight(history, 1000, "USD");
    expect(out[0]?.totalValue).toBe(100);
    expect(out[0]?.netContribution).toBe(40);
  });

  it("skips rows with non-finite totalValue or netContribution", () => {
    const history = [
      row({ valuationDate: "2026-01-01", totalValue: NaN }),
      row({ valuationDate: "2026-02-01", netContribution: Infinity }),
      row({ valuationDate: "2026-03-01", totalValue: 200, netContribution: 80 }),
    ];
    const out = scaleHistoryByWeight(history, 50, "USD");
    expect(out).toEqual([
      { date: "2026-03-01", totalValue: 100, netContribution: 40, currency: "USD" },
    ]);
  });

  it("returns [] for an empty input", () => {
    expect(scaleHistoryByWeight([], 50, "USD")).toEqual([]);
  });

  it("handles fractional weights without precision drama", () => {
    const history = [row({ totalValue: 1000, netContribution: 400 })];
    const out = scaleHistoryByWeight(history, 33.33, "USD");
    expect(out[0]?.totalValue).toBeCloseTo(333.3, 5);
    expect(out[0]?.netContribution).toBeCloseTo(133.32, 5);
  });
});

describe("assetClassColor", () => {
  it("assigns a distinct color to every asset class (Feroz: a different colored graph per class)", () => {
    const colors = ASSET_CLASS_ORDER.map(assetClassColor);
    expect(new Set(colors).size).toBe(ASSET_CLASS_ORDER.length);
  });

  it("returns a stable per-class theme token (not the gain/loss colors)", () => {
    expect(assetClassColor(AssetClass.STOCKS)).toBe("var(--asset-stocks)");
    expect(assetClassColor(AssetClass.PRECIOUS_METALS)).toBe("var(--asset-metals)");
    // never the reserved gain/loss tokens
    for (const cls of ASSET_CLASS_ORDER) {
      expect(assetClassColor(cls)).not.toBe("var(--success)");
      expect(assetClassColor(cls)).not.toBe("var(--destructive)");
    }
  });
});
