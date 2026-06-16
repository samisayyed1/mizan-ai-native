/**
 * Tests for Track A PR-A5 Net Worth Strip.
 *
 * Covers:
 *  - window-delta math: every preset (24h / 7d / 30d / YTD / All)
 *  - sparse-history correctness: no false positives when window
 *    pre-dates earliest data
 *  - divide-by-zero guard on pct
 *  - rendering: headline, delta chip color, window toggle, tap target,
 *    privacy mode, loading skeleton
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import type { AccountValuation } from "@/lib/types";

import { NetWorthStrip, computeWindowDelta } from "./net-worth-strip";

function makeRow(date: string, totalValue: number): AccountValuation {
  return {
    id: `v-${date}`,
    accountId: "TOTAL",
    valuationDate: date,
    accountCurrency: "USD",
    baseCurrency: "USD",
    fxRateToBase: 1,
    cashBalance: 0,
    investmentMarketValue: totalValue,
    totalValue,
    costBasis: totalValue,
    netContribution: 0,
    calculatedAt: `${date}T00:00:00Z`,
  };
}

const NOW = Date.UTC(2026, 5, 4, 12, 0, 0); // 2026-06-04 12:00 UTC

describe("computeWindowDelta", () => {
  it("returns noBaseline when history is empty", () => {
    expect(computeWindowDelta([], "30d", NOW)).toEqual({
      amount: 0,
      pct: null,
      noBaseline: true,
    });
  });

  it("returns noBaseline when only one entry exists in any window", () => {
    const history = [makeRow("2026-06-03", 1_000_000)];
    expect(computeWindowDelta(history, "24h", NOW)).toMatchObject({ noBaseline: true });
    expect(computeWindowDelta(history, "All", NOW)).toMatchObject({ noBaseline: true });
  });

  it("computes 24h delta against newest on-or-before-cutoff entry", () => {
    const history = [
      makeRow("2026-06-01", 900_000),
      makeRow("2026-06-02", 950_000), // 24h cutoff is 2026-06-03 12:00 — this is the baseline
      makeRow("2026-06-03", 980_000),
      makeRow("2026-06-04", 1_000_000),
    ];
    const delta = computeWindowDelta(history, "24h", NOW);
    // Baseline is the newest entry ≤ NOW − 24h. NOW − 24h = 2026-06-03 12:00 UTC.
    // history[1] is 2026-06-02 (= 2026-06-02T00:00Z, which is ≤ cutoff).
    // history[2] is 2026-06-03 (= 2026-06-03T00:00Z, which is also ≤ cutoff).
    // Newest ≤ cutoff = 2026-06-03 → 980_000.
    expect(delta.amount).toBe(20_000);
    expect(delta.pct).toBeCloseTo((20_000 / 980_000) * 100);
    expect(delta.noBaseline).toBe(false);
  });

  it("computes 7d delta", () => {
    const history = [
      makeRow("2026-05-20", 800_000),
      makeRow("2026-05-28", 900_000), // > 7d ago
      makeRow("2026-06-04", 1_000_000),
    ];
    const delta = computeWindowDelta(history, "7d", NOW);
    expect(delta.amount).toBe(100_000); // 1_000_000 - 900_000
  });

  it("computes 30d delta", () => {
    const history = [
      makeRow("2026-04-01", 700_000),
      makeRow("2026-05-04", 850_000), // exactly 31 days ago, qualifies as ≤ cutoff
      makeRow("2026-05-15", 950_000), // 20 days ago, doesn't qualify
      makeRow("2026-06-04", 1_000_000),
    ];
    const delta = computeWindowDelta(history, "30d", NOW);
    // Cutoff = NOW − 30d = 2026-05-05 12:00 UTC; baseline is newest ≤ cutoff = 2026-05-04
    expect(delta.amount).toBe(150_000);
  });

  it("computes YTD delta against newest entry on or before Jan 1", () => {
    const history = [
      makeRow("2025-12-31", 800_000), // last entry of prior year
      makeRow("2026-01-15", 850_000),
      makeRow("2026-06-04", 1_000_000),
    ];
    const delta = computeWindowDelta(history, "YTD", NOW);
    // Cutoff = 2026-01-01 00:00 UTC; baseline is 2025-12-31 (newest ≤ cutoff)
    expect(delta.amount).toBe(200_000);
  });

  it("computes All against the very first entry", () => {
    const history = [
      makeRow("2024-01-01", 500_000),
      makeRow("2025-01-01", 700_000),
      makeRow("2026-06-04", 1_000_000),
    ];
    expect(computeWindowDelta(history, "All", NOW).amount).toBe(500_000);
  });

  it("returns noBaseline when window pre-dates earliest data", () => {
    const history = [makeRow("2026-06-03", 980_000), makeRow("2026-06-04", 1_000_000)];
    // 30d window starts 2026-05-05; first row is 2026-06-03 → no qualifying baseline
    const delta = computeWindowDelta(history, "30d", NOW);
    expect(delta).toMatchObject({ noBaseline: true });
  });

  it("guards against pct divide-by-zero when baseline value is zero", () => {
    // 30d cutoff (NOW - 30d) = 2026-05-05 12:00 UTC; 2026-05-01 is before that
    const history = [makeRow("2026-05-01", 0), makeRow("2026-06-04", 1_000)];
    const delta = computeWindowDelta(history, "30d", NOW);
    expect(delta.amount).toBe(1_000);
    expect(delta.pct).toBeNull();
  });

  it("reports negative pct correctly", () => {
    const history = [
      makeRow("2026-05-01", 1_200_000),
      makeRow("2026-06-04", 1_000_000),
    ];
    const delta = computeWindowDelta(history, "30d", NOW);
    expect(delta.amount).toBe(-200_000);
    expect(delta.pct).toBeLessThan(0);
  });
});

describe("<NetWorthStrip />", () => {
  const history = [
    makeRow("2026-05-01", 900_000),
    makeRow("2026-06-04", 1_000_000),
  ];

  it("renders the headline and the configured default window's delta", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={history} baseCurrency="USD" defaultWindow="30d" />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Net Worth/i)).toBeInTheDocument();
    // Headline formats as currency
    expect(screen.getByText(/\$1,000,000/)).toBeInTheDocument();
    // Delta chip shows the past-30d delta
    expect(screen.getByTestId("net-worth-delta").textContent).toMatch(/past 30d/);
  });

  it("changes selected window when a toggle button is clicked", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={history} baseCurrency="USD" defaultWindow="30d" />
      </MemoryRouter>,
    );
    const allBtn = screen.getByRole("button", { name: "All" });
    fireEvent.click(allBtn);
    expect(allBtn).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("net-worth-delta").textContent).toMatch(/past All/);
  });

  it("links to /net-worth (the §23 tap target)", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={history} baseCurrency="USD" />
      </MemoryRouter>,
    );
    expect(screen.getByTestId("net-worth-strip-link")).toHaveAttribute("href", "/net-worth");
  });

  it("honours custom toHref override", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={history} baseCurrency="USD" toHref="/somewhere-else" />
      </MemoryRouter>,
    );
    expect(screen.getByTestId("net-worth-strip-link")).toHaveAttribute(
      "href",
      "/somewhere-else",
    );
  });

  it("hides absolute value in privacy mode but keeps the chip + toggle", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={history} baseCurrency="USD" isPrivacyMode />
      </MemoryRouter>,
    );
    expect(screen.getAllByText("•••••").length).toBeGreaterThan(0);
    expect(screen.getByTestId("net-worth-window-toggle")).toBeInTheDocument();
  });

  it("renders a skeleton bar when isLoading is true", () => {
    render(
      <MemoryRouter>
        <NetWorthStrip history={undefined} baseCurrency="USD" isLoading />
      </MemoryRouter>,
    );
    // Skeleton uses the same accessible label
    expect(screen.getByRole("link", { name: /Net Worth/i })).toBeInTheDocument();
  });

  it("shows friendly first-day-of-data copy when the window has no comparable baseline", () => {
    const oneRow = [makeRow("2026-06-04", 1_000_000)];
    render(
      <MemoryRouter>
        <NetWorthStrip history={oneRow} baseCurrency="USD" defaultWindow="30d" />
      </MemoryRouter>,
    );
    expect(screen.getByTestId("net-worth-delta").textContent).toMatch(/First day of data/);
  });
});
