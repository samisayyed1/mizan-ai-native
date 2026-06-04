/**
 * Tests for Track NW PR-NW2 Sankey asset-allocation helper.
 */
import { describe, expect, it } from "vitest";

import type { Holding } from "@/lib/types";

import {
  buildAssetAllocationFlow,
  computePanelTotals,
  totalNetWorthBase,
} from "./asset-allocation-flow";

function makeHolding(opts: {
  id: string;
  baseValue: number;
  assetKind?: Holding["assetKind"];
  holdingType?: string;
  assetTypeKey?: string;
}): Holding {
  return {
    id: opts.id,
    holdingType: (opts.holdingType ?? "SECURITY") as Holding["holdingType"],
    accountId: "acc-1",
    quantity: 1,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: { local: opts.baseValue, base: opts.baseValue },
    weight: 1,
    asOfDate: "2026-06-04",
    assetKind: opts.assetKind ?? "INVESTMENT",
    instrument: {
      id: `i-${opts.id}`,
      symbol: opts.id,
      name: opts.id,
      currency: "USD",
      quoteMode: "MANUAL",
      classifications: (opts.assetTypeKey
        ? {
            assetType: { key: opts.assetTypeKey, name: opts.assetTypeKey },
            assetClasses: [],
            sectors: [],
            regions: [],
            customGroups: [],
          }
        : null) as unknown as null,
    },
  };
}

describe("computePanelTotals", () => {
  it("aggregates non-empty panels desc by value, excludes vehicles", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 500_000, assetKind: "PROPERTY" }),
      makeHolding({ id: "h2", baseValue: 250_000, assetKind: "INVESTMENT", assetTypeKey: "EQUITY" }),
      makeHolding({ id: "h3", baseValue: 100_000, holdingType: "CASH" }),
      makeHolding({ id: "h4", baseValue: 50_000, assetKind: "VEHICLE" }), // excluded
    ];
    const rows = computePanelTotals(holdings);
    expect(rows.map((r) => r.panelId)).toEqual([
      "real-estate",
      "equities",
      "bank-cash",
    ]);
    expect(rows[0]?.totalBase).toBe(500_000);
    expect(rows[2]?.totalBase).toBe(100_000);
  });

  it("excludes liabilities (they go on Net Worth liabilities section)", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 500_000, assetKind: "PROPERTY" }),
      makeHolding({ id: "h2", baseValue: 100_000, assetKind: "LIABILITY" }),
    ];
    const rows = computePanelTotals(holdings);
    expect(rows.map((r) => r.panelId)).toEqual(["real-estate"]);
  });

  it("aggregates multiple positions in the same panel", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 100_000, assetKind: "INVESTMENT", assetTypeKey: "EQUITY" }),
      makeHolding({ id: "h2", baseValue: 200_000, assetKind: "INVESTMENT", assetTypeKey: "ETF" }),
    ];
    const rows = computePanelTotals(holdings);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.panelId).toBe("equities");
    expect(rows[0]?.totalBase).toBe(300_000);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 0, assetKind: "PROPERTY" }),
      makeHolding({ id: "h2", baseValue: -100, assetKind: "PROPERTY" }),
      makeHolding({ id: "h3", baseValue: 100_000, assetKind: "PROPERTY" }),
    ];
    const rows = computePanelTotals(holdings);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.totalBase).toBe(100_000);
  });

  it("empty input returns empty array", () => {
    expect(computePanelTotals([])).toEqual([]);
  });
});

describe("buildAssetAllocationFlow", () => {
  it("§23-flavoured fixture: Net Worth → 5 asset classes", () => {
    const holdings = [
      // Real Estate $1.65M (1 residence + 3 rentals + 1 held-for-sale)
      makeHolding({ id: "h1", baseValue: 800_000, assetKind: "PROPERTY" }),
      makeHolding({ id: "h2", baseValue: 850_000, assetKind: "PROPERTY" }),
      // Bonds $688K (Emaar $300K + DAR $200K + Sobha $188K)
      makeHolding({ id: "h3", baseValue: 688_000, assetTypeKey: "SUKUK" }),
      // Equities $500K
      makeHolding({ id: "h4", baseValue: 500_000, assetTypeKey: "EQUITY" }),
      // Bank & Cash $300K
      makeHolding({ id: "h5", baseValue: 300_000, holdingType: "CASH" }),
      // PE $400K
      makeHolding({ id: "h6", baseValue: 400_000, assetKind: "PRIVATE_EQUITY" }),
    ];
    const flow = buildAssetAllocationFlow(holdings);
    expect(flow).not.toBeNull();
    expect(flow!.nodes[0]?.name).toBe("Net Worth");
    expect(flow!.nodes.length).toBe(6); // 1 root + 5 panels
    expect(flow!.links).toHaveLength(5);
    // All flow from root (source=0)
    expect(flow!.links.every((l) => l.source === 0)).toBe(true);
    // Sum of link values = total Net Worth
    const sum = flow!.links.reduce((acc, l) => acc + l.value, 0);
    expect(sum).toBe(1_650_000 + 688_000 + 500_000 + 300_000 + 400_000);
    // Real Estate (largest) is the first target
    expect(flow!.nodes[1]?.name).toBe("Real Estate");
  });

  it("returns null on empty portfolio", () => {
    expect(buildAssetAllocationFlow([])).toBeNull();
  });

  it("returns null when only vehicles + liabilities present", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 50_000, assetKind: "VEHICLE" }),
      makeHolding({ id: "h2", baseValue: 100_000, assetKind: "LIABILITY" }),
    ];
    expect(buildAssetAllocationFlow(holdings)).toBeNull();
  });
});

describe("totalNetWorthBase", () => {
  it("sums non-vehicle non-liability holdings", () => {
    const holdings = [
      makeHolding({ id: "h1", baseValue: 100_000, assetKind: "PROPERTY" }),
      makeHolding({ id: "h2", baseValue: 200_000, assetTypeKey: "EQUITY" }),
      makeHolding({ id: "h3", baseValue: 50_000, assetKind: "VEHICLE" }), // excluded
      makeHolding({ id: "h4", baseValue: 10_000, assetKind: "LIABILITY" }), // excluded
    ];
    expect(totalNetWorthBase(holdings)).toBe(300_000);
  });

  it("empty input returns 0", () => {
    expect(totalNetWorthBase([])).toBe(0);
  });
});
