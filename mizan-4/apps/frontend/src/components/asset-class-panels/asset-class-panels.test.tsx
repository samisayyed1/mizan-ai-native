/**
 * Tests for Track A PR-A4 twelve-panel asset class skeleton.
 *
 * Covers:
 *  - taxonomy: fixed §3(e) order, classifier coverage, rollup math
 *  - card rendering: descriptor → DOM
 *  - grid: 12 panels always render; `'other'` appears only when non-empty
 *  - tap target: each card is a navigable link (the §23 step
 *    "He taps the Sukuks panel" precondition)
 */
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import type { AssetClassifications, Holding, Instrument, TaxonomyCategory } from "@/lib/types";

import {
  ASSET_CLASS_PANELS,
  AssetClassPanelGrid,
  classifyHolding,
  getPanelDescriptor,
  rollupHoldingsByPanel,
  type AssetClassPanelId,
} from "./index";

function makeAssetType(key: string, name = key): TaxonomyCategory {
  return {
    id: `cat-${key}`,
    taxonomyId: "tx-asset-type",
    name,
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

function makeInstrument(symbol: string, assetTypeKey?: string): Instrument {
  return {
    id: `i-${symbol}`,
    symbol,
    currency: "USD",
    quoteMode: "MARKET",
    classifications: assetTypeKey ? makeClassifications(assetTypeKey) : null,
  };
}

function makeHolding(overrides: Partial<Holding>): Holding {
  return {
    id: overrides.id ?? "h-1",
    holdingType: "INVESTMENT" as Holding["holdingType"],
    accountId: "acc-1",
    quantity: 1,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: { local: 100, base: 100 },
    weight: 1,
    asOfDate: "2026-06-04",
    ...overrides,
  };
}

describe("ASSET_CLASS_PANELS", () => {
  it("declares exactly twelve panels in fixed dashboard order per ADR 0021", () => {
    expect(ASSET_CLASS_PANELS).toHaveLength(12);
    const ids = ASSET_CLASS_PANELS.map((p) => p.id);
    expect(ids).toEqual([
      "equities",
      "brokerage-accounts",
      "bank-cash",
      "bonds-sukuks",
      "provident-funds",
      "insurance",
      "private-equity",
      "real-estate",
      "crypto",
      "commodities",
      "collectibles",
      "forex",
    ]);
  });

  it("every panel has a navigable holdings href", () => {
    for (const panel of ASSET_CLASS_PANELS) {
      expect(panel.holdingsHref).toMatch(/^\/holdings\?panel=/);
    }
  });
});

describe("classifyHolding", () => {
  const cases: [string, Partial<Holding>, AssetClassPanelId][] = [
    ["cash holding", { holdingType: "CASH" as Holding["holdingType"] }, "bank-cash"],
    ["property", { assetKind: "PROPERTY" }, "real-estate"],
    ["vehicle → other", { assetKind: "VEHICLE" }, "other"],
    ["collectible", { assetKind: "COLLECTIBLE" }, "collectibles"],
    ["precious metal", { assetKind: "PRECIOUS_METAL" }, "commodities"],
    ["private equity", { assetKind: "PRIVATE_EQUITY" }, "private-equity"],
    ["fx", { assetKind: "FX" }, "forex"],
    ["liability → other", { assetKind: "LIABILITY" }, "other"],
    [
      "BOND_GOVERNMENT → bonds-sukuks",
      { assetKind: "INVESTMENT", instrument: makeInstrument("UST", "BOND_GOVERNMENT") },
      "bonds-sukuks",
    ],
    [
      "DEBT_SECURITY → bonds-sukuks",
      { assetKind: "INVESTMENT", instrument: makeInstrument("EMAAR", "DEBT_SECURITY") },
      "bonds-sukuks",
    ],
    [
      "SUKUK → bonds-sukuks",
      { assetKind: "INVESTMENT", instrument: makeInstrument("DAR", "SUKUK") },
      "bonds-sukuks",
    ],
    [
      "MONEY_MARKET_DEBT → bonds-sukuks",
      { assetKind: "INVESTMENT", instrument: makeInstrument("MM", "MONEY_MARKET_DEBT") },
      "bonds-sukuks",
    ],
    [
      "CRYPTOCURRENCY → crypto",
      { assetKind: "INVESTMENT", instrument: makeInstrument("BTC", "CRYPTOCURRENCY") },
      "crypto",
    ],
    [
      "EQUITY → equities",
      { assetKind: "INVESTMENT", instrument: makeInstrument("AAPL", "EQUITY") },
      "equities",
    ],
    [
      "ETF → equities",
      { assetKind: "INVESTMENT", instrument: makeInstrument("SPUS", "ETF") },
      "equities",
    ],
    [
      "MUTUAL_FUND → equities",
      { assetKind: "INVESTMENT", instrument: makeInstrument("VFINX", "MUTUAL_FUND") },
      "equities",
    ],
    [
      "OPTION → equities",
      { assetKind: "INVESTMENT", instrument: makeInstrument("AAPL_C", "OPTION") },
      "equities",
    ],
    [
      "PRECIOUS_METAL classification → commodities",
      { assetKind: "INVESTMENT", instrument: makeInstrument("XAU", "PRECIOUS_METAL") },
      "commodities",
    ],
    [
      "INVESTMENT without classification → brokerage-accounts",
      { assetKind: "INVESTMENT" },
      "brokerage-accounts",
    ],
  ];

  for (const [name, holding, expected] of cases) {
    it(`maps ${name}`, () => {
      expect(classifyHolding(makeHolding(holding))).toBe(expected);
    });
  }
});

describe("rollupHoldingsByPanel", () => {
  it("returns twelve rollups in fixed order even with empty holdings", () => {
    const rollups = rollupHoldingsByPanel([], "USD");
    expect(rollups).toHaveLength(12);
    expect(rollups[0]?.panelId).toBe("equities");
    expect(rollups[11]?.panelId).toBe("forex");
    expect(rollups.every((r) => r.holdingsCount === 0)).toBe(true);
  });

  it("appends `other` only when there are unclassified holdings", () => {
    const withoutOther = rollupHoldingsByPanel(
      [makeHolding({ assetKind: "INVESTMENT", instrument: makeInstrument("AAPL", "EQUITY") })],
      "USD",
    );
    expect(withoutOther.find((r) => r.panelId === "other")).toBeUndefined();

    const withOther = rollupHoldingsByPanel([makeHolding({ assetKind: "VEHICLE" })], "USD");
    expect(withOther.find((r) => r.panelId === "other")).toBeDefined();
    expect(withOther.find((r) => r.panelId === "other")?.holdingsCount).toBe(1);
  });

  it("sums market values + day changes into the correct bucket", () => {
    const holdings: Holding[] = [
      makeHolding({
        id: "h1",
        assetKind: "INVESTMENT",
        instrument: makeInstrument("EMAAR", "SUKUK"),
        marketValue: { local: 100_000, base: 188_000 },
        dayChange: { local: 500, base: 940 },
      }),
      makeHolding({
        id: "h2",
        assetKind: "INVESTMENT",
        instrument: makeInstrument("DAR", "SUKUK"),
        marketValue: { local: 100_000, base: 200_000 },
        dayChange: { local: -200, base: -400 },
      }),
    ];

    const rollups = rollupHoldingsByPanel(holdings, "USD");
    const sukuks = rollups.find((r) => r.panelId === "bonds-sukuks");
    expect(sukuks?.totalValue).toBe(388_000);
    expect(sukuks?.dayChange).toBe(540);
    expect(sukuks?.holdingsCount).toBe(2);
  });

  it("dayChangePct stays null when prior-day total was zero (avoids divide-by-zero)", () => {
    const holdings: Holding[] = [
      makeHolding({
        assetKind: "INVESTMENT",
        instrument: makeInstrument("X", "EQUITY"),
        marketValue: { local: 100, base: 100 },
        dayChange: { local: 100, base: 100 },
      }),
    ];
    const rollups = rollupHoldingsByPanel(holdings, "USD");
    const eq = rollups.find((r) => r.panelId === "equities");
    expect(eq?.dayChangePct).toBeNull();
  });
});

describe("getPanelDescriptor", () => {
  it("returns synthetic Other descriptor for `'other'`", () => {
    const d = getPanelDescriptor("other");
    expect(d.id).toBe("other");
    expect(d.label).toBe("Other");
  });

  it("throws on an unknown id", () => {
    expect(() => getPanelDescriptor("nope" as AssetClassPanelId)).toThrow(/Unknown asset-class/);
  });
});

describe("<AssetClassPanelGrid />", () => {
  it("renders twelve cards in fixed dashboard order", () => {
    render(
      <MemoryRouter>
        <AssetClassPanelGrid holdings={[]} baseCurrency="USD" />
      </MemoryRouter>,
    );
    const links = screen.getAllByRole("link");
    expect(links).toHaveLength(12);
    expect(links[0]).toHaveAttribute("aria-label", "Open Equities panel");
    expect(links[3]).toHaveAttribute("aria-label", "Open Bonds & Sukuks panel");
  });

  it("each card links to the panel's holdings href (the §23 tap surface)", () => {
    render(
      <MemoryRouter>
        <AssetClassPanelGrid holdings={[]} baseCurrency="USD" />
      </MemoryRouter>,
    );
    const sukuks = screen.getByLabelText("Open Bonds & Sukuks panel");
    expect(sukuks).toHaveAttribute("href", "/holdings?panel=bonds-sukuks");
  });

  it("hides absolute values in privacy mode but keeps holdings count", () => {
    const holdings = [
      makeHolding({
        assetKind: "INVESTMENT",
        instrument: makeInstrument("X", "EQUITY"),
        marketValue: { local: 1000, base: 1000 },
      }),
    ];
    render(
      <MemoryRouter>
        <AssetClassPanelGrid holdings={holdings} baseCurrency="USD" isPrivacyMode />
      </MemoryRouter>,
    );
    expect(screen.getAllByText("•••••").length).toBeGreaterThan(0);
    expect(screen.getByText("1 holding")).toBeInTheDocument();
  });

  it("renders an Other tile only when there are unclassified holdings", () => {
    const { rerender } = render(
      <MemoryRouter>
        <AssetClassPanelGrid holdings={[]} baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(screen.queryByLabelText("Open Other panel")).toBeNull();

    rerender(
      <MemoryRouter>
        <AssetClassPanelGrid
          holdings={[makeHolding({ assetKind: "VEHICLE" })]}
          baseCurrency="USD"
        />
      </MemoryRouter>,
    );
    expect(screen.getByLabelText("Open Other panel")).toBeInTheDocument();
  });
});
