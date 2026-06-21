/**
 * Tests for Track A PR-A8 Holdings Heatmap tap → asset detail.
 *
 * Two layers:
 *   1. holdingDetailRouteFromNode — pure helper that maps a Recharts
 *      Treemap onClick node to a router route. The helper carries the
 *      assetId-required + URL-encode invariants.
 *   2. <HoldingsHeatmap /> — exercise the loading / empty / data
 *      branches via @testing-library/react. We can't simulate the
 *      actual Treemap click in jsdom (Recharts needs a real
 *      ResponsiveContainer measurement), but we can assert the
 *      surfaces and the mode toggle work and the heatmap excludes
 *      cash + alternative assets per spec.
 */
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import type { Holding } from "@/lib/types";

import { HoldingsHeatmap, holdingDetailRouteFromNode } from "./holdings-heatmap";

function makeHolding(overrides: Partial<Holding>): Holding {
  return {
    id: overrides.id ?? "h-1",
    holdingType: "INVESTMENT" as Holding["holdingType"],
    accountId: "acc-1",
    quantity: 10,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: { local: 10_000, base: 10_000 },
    weight: 1,
    asOfDate: "2026-06-04",
    ...overrides,
  };
}

describe("holdingDetailRouteFromNode", () => {
  it("returns the encoded route for a node with a valid assetId", () => {
    expect(holdingDetailRouteFromNode({ assetId: "AAPL" })).toBe("/holdings/AAPL");
  });

  it("URL-encodes special characters in the asset id", () => {
    // OCC option symbol has spaces and slashes in some encodings
    expect(holdingDetailRouteFromNode({ assetId: "AAPL 260116C00250000" })).toBe(
      "/holdings/AAPL%20260116C00250000",
    );
    // Custom assets may have user-typed labels with characters that
    // must not break the route — the helper guards by encoding.
    expect(holdingDetailRouteFromNode({ assetId: "a/b?c=d&e=f" })).toBe(
      "/holdings/a%2Fb%3Fc%3Dd%26e%3Df",
    );
  });

  it("returns null when the node has no assetId (avoids broken-route nav)", () => {
    expect(holdingDetailRouteFromNode({})).toBeNull();
    expect(holdingDetailRouteFromNode({ assetId: "" })).toBeNull();
    expect(holdingDetailRouteFromNode(null)).toBeNull();
    expect(holdingDetailRouteFromNode(undefined)).toBeNull();
    expect(holdingDetailRouteFromNode("not an object")).toBeNull();
    expect(holdingDetailRouteFromNode({ assetId: 123 })).toBeNull();
  });

  it("ignores extra fields on the node (Recharts passes the whole datum)", () => {
    expect(
      holdingDetailRouteFromNode({
        assetId: "TSLA",
        symbol: "TSLA",
        size: 50_000,
        changePct: 0.03,
        holdingId: "h-tsla",
      }),
    ).toBe("/holdings/TSLA");
  });
});

describe("<HoldingsHeatmap />", () => {
  it("renders the loading skeleton when isLoading is true", () => {
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={[]} isLoading baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(screen.getByText("Heatmap")).toBeInTheDocument();
  });

  it("renders an empty-state message when there are no tradeable holdings", () => {
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={[]} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(
      screen.getByText(/No holdings to chart/),
    ).toBeInTheDocument();
  });

  it("excludes cash holdings from the heatmap (cash isn't tradeable, no pct return)", () => {
    const holdings: Holding[] = [
      makeHolding({
        id: "h-cash",
        holdingType: "cash" as Holding["holdingType"],
        marketValue: { local: 50_000, base: 50_000 },
      }),
    ];
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={holdings} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    // Only cash → renders the empty state, not the chart
    expect(
      screen.getByText(/No holdings to chart/),
    ).toBeInTheDocument();
  });

  it("excludes alternative assets (property, vehicle, collectible) from the heatmap", () => {
    const holdings: Holding[] = [
      makeHolding({ id: "p", assetKind: "PROPERTY" }),
      makeHolding({ id: "v", assetKind: "VEHICLE" }),
      makeHolding({ id: "c", assetKind: "COLLECTIBLE" }),
    ];
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={holdings} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(
      screen.getByText(/No holdings to chart/),
    ).toBeInTheDocument();
  });

  it("renders Total/Day mode toggle when there are tradeable holdings", () => {
    const holdings: Holding[] = [
      makeHolding({
        id: "h-aapl",
        instrument: {
          id: "AAPL",
          symbol: "AAPL",
          currency: "USD",
          quoteMode: "MARKET",
          // The heatmap now filters by classifyHolding(h) === "equities"
          // (`feat(heatmap): restrict to equities only`, 2026-06-14).
          // Without an EQUITY_SECURITY classification AAPL falls through
          // to `brokerage-accounts` and is excluded, leaving the empty
          // state on screen and the Total/Day toggle absent.
          classifications: {
            assetType: { key: "EQUITY_SECURITY" },
          },
        } as Holding["instrument"],
        marketValue: { local: 10_000, base: 10_000 },
        unrealizedGain: { local: 1_000, base: 1_000 },
        unrealizedGainPct: 0.1,
      }),
    ];
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={holdings} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    const total = screen.getByRole("button", { name: /Total/i });
    const day = screen.getByRole("button", { name: /Day/i });
    expect(total).toBeInTheDocument();
    expect(day).toBeInTheDocument();
    // Toggling does not throw + does not change the surface to broken
    fireEvent.click(day);
    fireEvent.click(total);
    expect(screen.getByText("Heatmap")).toBeInTheDocument();
  });

  it("renders cold-start positions sized by cost basis when market value is missing", () => {
    // Regression for: freshly-seeded equities with no live quote yet
    // showed 'No holdings to chart' even though the Equities tile + the
    // Holdings page already displayed the positions. The fix sizes
    // tiles by cost basis when market value is unset; the heatmap
    // surfaces the toggle (and thus the tile grid) instead of the
    // empty state.
    const holdings: Holding[] = [
      makeHolding({
        id: "h-aapl-cold",
        instrument: {
          id: "AAPL",
          symbol: "AAPL",
          currency: "USD",
          quoteMode: "MARKET",
          classifications: { assetType: { key: "EQUITY_SECURITY" } },
        } as Holding["instrument"],
        marketValue: { local: 0, base: 0 },
        costBasis: { local: 5_240, base: 5_240 },
      }),
    ];
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={holdings} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    // Toggle is visible only when at least one tradeable tile exists.
    expect(screen.getByRole("button", { name: /Total/i })).toBeInTheDocument();
    expect(screen.queryByText(/No holdings to chart/)).not.toBeInTheDocument();
  });

  it("hides a position with neither market value nor cost basis", () => {
    // Negative case for the cold-start fallback: a holding with zero
    // market value AND zero cost basis is genuine noise and should
    // stay out of the heatmap.
    const holdings: Holding[] = [
      makeHolding({
        id: "h-empty",
        instrument: {
          id: "GHOST",
          symbol: "GHOST",
          currency: "USD",
          quoteMode: "MARKET",
          classifications: { assetType: { key: "EQUITY_SECURITY" } },
        } as Holding["instrument"],
        marketValue: { local: 0, base: 0 },
        costBasis: { local: 0, base: 0 },
      }),
    ];
    render(
      <MemoryRouter>
        <HoldingsHeatmap holdings={holdings} isLoading={false} baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(screen.getByText(/No holdings to chart/)).toBeInTheDocument();
  });
});
