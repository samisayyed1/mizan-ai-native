import { describe, expect, it } from "vitest";

import { calculatePerformanceMetrics } from "./utils";
import type { AccountValuation } from "./types";

function valuation(over: Partial<AccountValuation> = {}): AccountValuation {
  return {
    id: "VAL",
    accountId: "ACC",
    valuationDate: "2026-01-01",
    accountCurrency: "USD",
    baseCurrency: "USD",
    fxRateToBase: 1,
    cashBalance: 0,
    investmentMarketValue: 0,
    totalValue: 0,
    costBasis: 0,
    netContribution: 0,
    calculatedAt: "2026-01-01T00:00:00Z",
    ...over,
  } as AccountValuation;
}

describe("calculatePerformanceMetrics — TWR formula", () => {
  it("returns zero on empty/missing history", () => {
    expect(calculatePerformanceMetrics(null)).toEqual({
      gainLossAmount: 0,
      simpleReturn: 0,
    });
    expect(calculatePerformanceMetrics(undefined)).toEqual({
      gainLossAmount: 0,
      simpleReturn: 0,
    });
    expect(calculatePerformanceMetrics([])).toEqual({
      gainLossAmount: 0,
      simpleReturn: 0,
    });
  });

  it("pure market gain with no flows: +10% becomes +10%", () => {
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 1000, netContribution: 1000 }),
      valuation({ valuationDate: "2026-01-02", totalValue: 1100, netContribution: 1000 }),
    ];
    const result = calculatePerformanceMetrics(history);
    expect(result.gainLossAmount).toBe(100);
    expect(result.simpleReturn).toBeCloseTo(0.1, 8);
  });

  it("QA Pass 13: matches backend TWR (start-of-day flow convention)", () => {
    // Backend (performance_service.rs): twr = curr/(prev+cf) - 1.
    // Pre-fix the frontend computed (curr-cf)/prev which gave a different
    // number whenever cash flowed. Same data → two views disagree on
    // "past year %" → CRITICAL.
    //
    // Day 1: prev=$100, curr=$120, cf=+$10 deposit.
    // Backend: 120 / (100 + 10) - 1 = 9.090909...%
    // Frontend OLD: (120 - 10) / 100 - 1 = 10.0%  (BUG)
    // Frontend NEW: matches backend.
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 100, netContribution: 0 }),
      valuation({ valuationDate: "2026-01-02", totalValue: 120, netContribution: 10 }),
    ];
    const result = calculatePerformanceMetrics(history);
    // 120 / 110 - 1 = 0.0909090909...
    expect(result.simpleReturn).toBeCloseTo(120 / 110 - 1, 8);
  });

  it("withdrawal mid-period boosts TWR (backend convention)", () => {
    // Day 1: prev=$100, curr=$95, cf=-$10 withdrawal.
    // Backend: 95 / (100 - 10) - 1 = 95/90 - 1 = +5.555...%
    // The withdrawal removed cash, so the remaining capital outperformed.
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 100, netContribution: 100 }),
      valuation({ valuationDate: "2026-01-02", totalValue: 95, netContribution: 90 }),
    ];
    const result = calculatePerformanceMetrics(history);
    expect(result.simpleReturn).toBeCloseTo(95 / 90 - 1, 8);
  });

  it("compounds across multiple days (chained factors)", () => {
    // Three consecutive +10% market days, no flows.
    // Day 1: 100 → 110, day 2: 110 → 121, day 3: 121 → 133.1
    // Cumulative TWR factor: 1.1 × 1.1 × 1.1 = 1.331 → 33.1%
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 100, netContribution: 100 }),
      valuation({ valuationDate: "2026-01-02", totalValue: 110, netContribution: 100 }),
      valuation({ valuationDate: "2026-01-03", totalValue: 121, netContribution: 100 }),
      valuation({ valuationDate: "2026-01-04", totalValue: 133.1, netContribution: 100 }),
    ];
    const result = calculatePerformanceMetrics(history);
    expect(result.simpleReturn).toBeCloseTo(0.331, 6);
  });

  it("zero-denom guard: skips degenerate day instead of dividing by zero", () => {
    // Withdrawal exactly equals starting balance → denom = 0. Must NOT
    // produce Infinity/NaN; matches backend zero-denom skip.
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 100, netContribution: 100 }),
      valuation({ valuationDate: "2026-01-02", totalValue: 0, netContribution: 0 }),
      valuation({ valuationDate: "2026-01-03", totalValue: 50, netContribution: 50 }),
    ];
    const result = calculatePerformanceMetrics(history);
    expect(Number.isFinite(result.simpleReturn)).toBe(true);
  });

  it("isAllTime: divides cumulative gain by ending net contribution", () => {
    const history = [
      valuation({ valuationDate: "2026-01-01", totalValue: 1000, netContribution: 1000 }),
      valuation({ valuationDate: "2026-12-31", totalValue: 1500, netContribution: 1200 }),
    ];
    const result = calculatePerformanceMetrics(history, true);
    // gain = 1500 - 1200 = 300; ROI = 300/1200 = 25%
    expect(result.gainLossAmount).toBe(300);
    expect(result.simpleReturn).toBeCloseTo(0.25, 8);
  });
});
