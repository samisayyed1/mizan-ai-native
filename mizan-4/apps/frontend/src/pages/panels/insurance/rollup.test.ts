/**
 * Tests for Track B PR-B10 Insurance rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type { Holding, Instrument } from "@/lib/types";

import {
  STALE_THRESHOLD_MS,
  categoryLabel,
  countStale,
  extractCategory,
  extractSurrenderValueBase,
  isInsuranceHolding,
  isPureProtection,
  isStaleAt,
  normaliseCategory,
  rollupByCategory,
  rollupByPolicy,
  totalInsuranceExposure,
} from "./rollup";

const NOW_MS = Date.UTC(2026, 5, 4, 0, 0, 0); // 2026-06-04T00:00:00Z

function makeInstrument(
  symbol: string,
  insurance?: Record<string, unknown>,
): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MANUAL",
    classifications: null,
  };
  if (insurance) {
    (base as unknown as { metadata: Record<string, unknown> }).metadata = {
      insurance,
    };
  }
  return base;
}

function makePolicy(opts: {
  id: string;
  symbol: string;
  insurance?: Record<string, unknown>;
  baseValue?: number;
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
    assetKind: "INVESTMENT",
    instrument: makeInstrument(opts.symbol, opts.insurance),
  };
}

describe("normaliseCategory + categoryLabel", () => {
  it("maps canonical codes", () => {
    expect(normaliseCategory("INVESTMENT_LINKED")).toBe("INVESTMENT_LINKED");
    expect(normaliseCategory("TERM_LIFE")).toBe("TERM_LIFE");
    expect(normaliseCategory("WHOLE_LIFE")).toBe("WHOLE_LIFE");
    expect(normaliseCategory("HEALTH")).toBe("HEALTH");
    expect(normaliseCategory("PROPERTY")).toBe("PROPERTY");
  });

  it("accepts ULIP / ILP aliases", () => {
    expect(normaliseCategory("ULIP")).toBe("INVESTMENT_LINKED");
    expect(normaliseCategory("ilp")).toBe("INVESTMENT_LINKED");
    expect(normaliseCategory("investment linked")).toBe("INVESTMENT_LINKED");
    expect(normaliseCategory("investment-linked")).toBe("INVESTMENT_LINKED");
  });

  it("accepts short forms", () => {
    expect(normaliseCategory("term")).toBe("TERM_LIFE");
    expect(normaliseCategory("whole")).toBe("WHOLE_LIFE");
    expect(normaliseCategory("medical")).toBe("HEALTH");
    expect(normaliseCategory("home")).toBe("PROPERTY");
  });

  it("falls back to OTHER", () => {
    expect(normaliseCategory("MYSTERY")).toBe("OTHER");
    expect(normaliseCategory(undefined)).toBe("OTHER");
    expect(normaliseCategory(null)).toBe("OTHER");
    expect(normaliseCategory("")).toBe("OTHER");
  });

  it("renders display labels", () => {
    expect(categoryLabel("INVESTMENT_LINKED")).toBe("Investment-Linked");
    expect(categoryLabel("TERM_LIFE")).toBe("Term Life");
    expect(categoryLabel("WHOLE_LIFE")).toBe("Whole Life");
    expect(categoryLabel("HEALTH")).toBe("Health");
    expect(categoryLabel("PROPERTY")).toBe("Property");
    expect(categoryLabel("OTHER")).toBe("Other");
  });
});

describe("isPureProtection", () => {
  it("identifies pure-protection categories", () => {
    expect(isPureProtection("TERM_LIFE")).toBe(true);
    expect(isPureProtection("HEALTH")).toBe(true);
    expect(isPureProtection("INVESTMENT_LINKED")).toBe(false);
    expect(isPureProtection("WHOLE_LIFE")).toBe(false);
    expect(isPureProtection("PROPERTY")).toBe(false);
    expect(isPureProtection("OTHER")).toBe(false);
  });
});

describe("isInsuranceHolding + extractCategory", () => {
  it("returns true when insurance metadata present", () => {
    const h = makePolicy({
      id: "h",
      symbol: "X",
      insurance: { category: "TERM_LIFE" },
    });
    expect(isInsuranceHolding(h)).toBe(true);
    expect(extractCategory(h)).toBe("TERM_LIFE");
  });

  it("returns false otherwise", () => {
    expect(
      isInsuranceHolding(makePolicy({ id: "h", symbol: "AAPL", baseValue: 1000 })),
    ).toBe(false);
  });
});

describe("extractSurrenderValueBase", () => {
  it("uses surrenderValue when present", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "INVESTMENT_LINKED", surrenderValue: 25_000 },
          baseValue: 100_000,
        }),
      ),
    ).toBe(25_000);
  });

  it("falls back to cashValue when surrenderValue absent", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "INVESTMENT_LINKED", cashValue: 18_000 },
          baseValue: 100_000,
        }),
      ),
    ).toBe(18_000);
  });

  it("falls back to marketValue.base for non-pure-protection", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "WHOLE_LIFE" },
          baseValue: 50_000,
        }),
      ),
    ).toBe(50_000);
  });

  it("returns 0 for pure protection with no declared surrender", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "Term-X",
          insurance: { category: "TERM_LIFE" },
          baseValue: 9_999_999,
        }),
      ),
    ).toBe(0);
  });

  it("honours declared surrenderValue even on pure protection (cash-out riders)", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "Term-Rider",
          insurance: { category: "TERM_LIFE", surrenderValue: 1500 },
        }),
      ),
    ).toBe(1500);
  });

  it("accepts numeric strings for surrenderValue + cashValue", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "WHOLE_LIFE", surrenderValue: "12345.67" },
        }),
      ),
    ).toBeCloseTo(12345.67);
  });

  it("ignores negative surrender", () => {
    expect(
      extractSurrenderValueBase(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "WHOLE_LIFE", surrenderValue: -100 },
          baseValue: 50_000,
        }),
      ),
    ).toBe(50_000);
  });
});

describe("isStaleAt", () => {
  it("returns true when lastValuedAt absent", () => {
    expect(
      isStaleAt(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "INVESTMENT_LINKED" },
        }),
        NOW_MS,
      ),
    ).toBe(true);
  });

  it("returns false when lastValuedAt within 7 days", () => {
    const sixDaysAgo = new Date(NOW_MS - 6 * 24 * 60 * 60 * 1000).toISOString();
    expect(
      isStaleAt(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "INVESTMENT_LINKED", lastValuedAt: sixDaysAgo },
        }),
        NOW_MS,
      ),
    ).toBe(false);
  });

  it("returns true when lastValuedAt older than 7 days", () => {
    const eightDaysAgo = new Date(NOW_MS - 8 * 24 * 60 * 60 * 1000).toISOString();
    expect(
      isStaleAt(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "INVESTMENT_LINKED", lastValuedAt: eightDaysAgo },
        }),
        NOW_MS,
      ),
    ).toBe(true);
  });

  it("returns false at exactly 7 days (boundary inclusive of fresh)", () => {
    const sevenDaysAgo = new Date(NOW_MS - STALE_THRESHOLD_MS).toISOString();
    expect(
      isStaleAt(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "WHOLE_LIFE", lastValuedAt: sevenDaysAgo },
        }),
        NOW_MS,
      ),
    ).toBe(false);
  });

  it("returns true for invalid lastValuedAt string", () => {
    expect(
      isStaleAt(
        makePolicy({
          id: "h",
          symbol: "X",
          insurance: { category: "WHOLE_LIFE", lastValuedAt: "not-a-date" },
        }),
        NOW_MS,
      ),
    ).toBe(true);
  });
});

describe("rollupByCategory", () => {
  it("§23 fixture: ULIP + Whole Life + Property + Term + Health portfolio", () => {
    const sixDaysAgo = new Date(NOW_MS - 6 * 24 * 60 * 60 * 1000).toISOString();
    const holdings = [
      makePolicy({
        id: "p1",
        symbol: "Pru ULIP",
        insurance: {
          category: "INVESTMENT_LINKED",
          surrenderValue: 80_000,
          lastValuedAt: sixDaysAgo,
        },
      }),
      makePolicy({
        id: "p2",
        symbol: "AIA Whole Life",
        insurance: {
          category: "WHOLE_LIFE",
          cashValue: 50_000,
          lastValuedAt: sixDaysAgo,
        },
      }),
      makePolicy({
        id: "p3",
        symbol: "Hyderabad fire cover",
        insurance: { category: "PROPERTY", surrenderValue: 0 },
      }),
      makePolicy({
        id: "p4",
        symbol: "NTUC Term",
        insurance: { category: "TERM_LIFE" },
      }),
      makePolicy({
        id: "p5",
        symbol: "Health rider",
        insurance: { category: "HEALTH" },
      }),
    ];
    const rows = rollupByCategory(holdings);
    expect(rows[0]).toEqual({
      category: "INVESTMENT_LINKED",
      label: "Investment-Linked",
      totalValueBase: 80_000,
      policyCount: 1,
    });
    expect(rows[1]).toEqual({
      category: "WHOLE_LIFE",
      label: "Whole Life",
      totalValueBase: 50_000,
      policyCount: 1,
    });
    const termRow = rows.find((r) => r.category === "TERM_LIFE");
    const healthRow = rows.find((r) => r.category === "HEALTH");
    const propertyRow = rows.find((r) => r.category === "PROPERTY");
    expect(termRow?.totalValueBase).toBe(0);
    expect(termRow?.policyCount).toBe(1);
    expect(healthRow?.totalValueBase).toBe(0);
    expect(healthRow?.policyCount).toBe(1);
    expect(propertyRow?.totalValueBase).toBe(0);
    expect(propertyRow?.policyCount).toBe(1);
  });

  it("skips non-insurance holdings", () => {
    expect(
      rollupByCategory([makePolicy({ id: "h", symbol: "AAPL", baseValue: 1000 })]),
    ).toEqual([]);
  });
});

describe("rollupByPolicy", () => {
  it("emits one row per policy with stale + pureProtection flags", () => {
    const twoDaysAgo = new Date(NOW_MS - 2 * 24 * 60 * 60 * 1000).toISOString();
    const tenDaysAgo = new Date(NOW_MS - 10 * 24 * 60 * 60 * 1000).toISOString();
    const holdings = [
      makePolicy({
        id: "fresh",
        symbol: "Pru ULIP",
        insurance: {
          category: "ULIP",
          surrenderValue: 80_000,
          lastValuedAt: twoDaysAgo,
          policyNumber: "PRU-001",
        },
      }),
      makePolicy({
        id: "stale",
        symbol: "AIA Whole",
        insurance: {
          category: "WHOLE_LIFE",
          surrenderValue: 50_000,
          lastValuedAt: tenDaysAgo,
        },
      }),
      makePolicy({
        id: "term",
        symbol: "NTUC Term",
        insurance: { category: "TERM_LIFE" },
      }),
    ];
    const rows = rollupByPolicy(holdings, NOW_MS);
    expect(rows.map((r) => r.holdingId)).toEqual(["fresh", "stale", "term"]);
    expect(rows[0]?.isStale).toBe(false);
    expect(rows[0]?.policyNumber).toBe("PRU-001");
    expect(rows[0]?.category).toBe("INVESTMENT_LINKED");
    expect(rows[1]?.isStale).toBe(true);
    expect(rows[2]?.isPureProtection).toBe(true);
    expect(rows[2]?.valueBase).toBe(0);
    expect(rows[2]?.isStale).toBe(true); // no lastValuedAt → stale
  });

  it("name→symbol fallback", () => {
    const h = makePolicy({
      id: "h",
      symbol: "POL-RAW",
      insurance: { category: "WHOLE_LIFE" },
    });
    if (h.instrument) h.instrument.name = null;
    expect(rollupByPolicy([h], NOW_MS)[0]?.label).toBe("POL-RAW");
  });
});

describe("totalInsuranceExposure + countStale", () => {
  it("sums surrender exposure across insurance only", () => {
    const holdings = [
      makePolicy({
        id: "p1",
        symbol: "ULIP",
        insurance: { category: "ULIP", surrenderValue: 80_000 },
      }),
      makePolicy({
        id: "p2",
        symbol: "Whole",
        insurance: { category: "WHOLE_LIFE", cashValue: 50_000 },
      }),
      makePolicy({
        id: "p3",
        symbol: "Term",
        insurance: { category: "TERM_LIFE" },
      }),
      makePolicy({ id: "p4", symbol: "AAPL", baseValue: 9_999_999 }),
    ];
    expect(totalInsuranceExposure(holdings)).toBe(130_000);
  });

  it("counts stale across all insurance holdings", () => {
    const fresh = new Date(NOW_MS - 1 * 24 * 60 * 60 * 1000).toISOString();
    const stale = new Date(NOW_MS - 15 * 24 * 60 * 60 * 1000).toISOString();
    const holdings = [
      makePolicy({
        id: "p1",
        symbol: "Fresh",
        insurance: { category: "WHOLE_LIFE", lastValuedAt: fresh },
      }),
      makePolicy({
        id: "p2",
        symbol: "Stale",
        insurance: { category: "WHOLE_LIFE", lastValuedAt: stale },
      }),
      makePolicy({
        id: "p3",
        symbol: "Missing",
        insurance: { category: "WHOLE_LIFE" },
      }),
    ];
    expect(countStale(holdings, NOW_MS)).toBe(2);
  });

  it("empty input → 0 totals", () => {
    expect(totalInsuranceExposure([])).toBe(0);
    expect(countStale([], NOW_MS)).toBe(0);
  });
});
