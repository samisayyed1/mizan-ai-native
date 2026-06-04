/**
 * Tests for Track B PR-B9 Provident Funds rollup helpers.
 */
import { describe, expect, it } from "vitest";

import type { Holding, Instrument } from "@/lib/types";

import {
  extractCpfSubAccount,
  extractScheme,
  extractSourceAccountId,
  isProvidentFundHolding,
  normaliseScheme,
  rollupByCpfSubAccount,
  rollupByPosition,
  rollupByScheme,
  schemeLabel,
  totalProvidentFundExposure,
} from "./rollup";

function makeInstrument(
  symbol: string,
  pf?: { scheme?: string; subAccount?: string; sourceAccountId?: string },
): Instrument {
  const base: Instrument = {
    id: `i-${symbol}`,
    symbol,
    name: symbol,
    currency: "USD",
    quoteMode: "MANUAL",
    classifications: null,
  };
  if (pf) {
    (base as unknown as { metadata: Record<string, unknown> }).metadata = {
      providentFund: pf,
    };
  }
  return base;
}

function makePosition(opts: {
  id: string;
  symbol: string;
  pf?: { scheme?: string; subAccount?: string; sourceAccountId?: string };
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
    assetKind: opts.assetKind ?? "INVESTMENT",
    instrument: makeInstrument(opts.symbol, opts.pf),
  };
}

describe("normaliseScheme", () => {
  it("maps canonical codes", () => {
    expect(normaliseScheme("CPF")).toBe("CPF");
    expect(normaliseScheme("EPF")).toBe("EPF");
    expect(normaliseScheme("401K")).toBe("401K");
    expect(normaliseScheme("NPS")).toBe("NPS");
    expect(normaliseScheme("SUPER")).toBe("SUPER");
  });

  it("handles case + whitespace + separators", () => {
    expect(normaliseScheme("cpf")).toBe("CPF");
    expect(normaliseScheme("  ePf  ")).toBe("EPF");
    expect(normaliseScheme("401-K")).toBe("401K");
    expect(normaliseScheme("401_K")).toBe("401K");
    expect(normaliseScheme("K401")).toBe("401K");
    expect(normaliseScheme("Superannuation")).toBe("SUPER");
  });

  it("returns OTHER for unrecognised + null + empty", () => {
    expect(normaliseScheme("GRATUITY")).toBe("OTHER");
    expect(normaliseScheme(null)).toBe("OTHER");
    expect(normaliseScheme(undefined)).toBe("OTHER");
    expect(normaliseScheme("")).toBe("OTHER");
  });
});

describe("schemeLabel", () => {
  it("renders display labels including formatted 401(k)", () => {
    expect(schemeLabel("CPF")).toBe("CPF");
    expect(schemeLabel("EPF")).toBe("EPF");
    expect(schemeLabel("401K")).toBe("401(k)");
    expect(schemeLabel("NPS")).toBe("NPS");
    expect(schemeLabel("SUPER")).toBe("Superannuation");
    expect(schemeLabel("OTHER")).toBe("Other");
  });
});

describe("isProvidentFundHolding", () => {
  it("returns true when providentFund metadata exists", () => {
    expect(
      isProvidentFundHolding(
        makePosition({ id: "h", symbol: "X", pf: { scheme: "CPF" }, baseValue: 100 }),
      ),
    ).toBe(true);
  });

  it("returns false when metadata absent", () => {
    expect(
      isProvidentFundHolding(makePosition({ id: "h", symbol: "X", baseValue: 100 })),
    ).toBe(false);
  });
});

describe("extractScheme + extractCpfSubAccount + extractSourceAccountId", () => {
  it("extracts CPF + OA from a §23-shaped holding", () => {
    const h = makePosition({
      id: "h",
      symbol: "CPF-OA",
      pf: { scheme: "CPF", subAccount: "OA", sourceAccountId: "acc-cpf" },
      baseValue: 100,
    });
    expect(extractScheme(h)).toBe("CPF");
    expect(extractCpfSubAccount(h)).toBe("OA");
    expect(extractSourceAccountId(h)).toBe("acc-cpf");
  });

  it("returns null sub-account for non-CPF schemes even if subAccount declared", () => {
    const h = makePosition({
      id: "h",
      symbol: "EPF-A",
      pf: { scheme: "EPF", subAccount: "OA" },
      baseValue: 100,
    });
    expect(extractCpfSubAccount(h)).toBeNull();
  });

  it("returns null sub-account when subAccount missing or unknown", () => {
    expect(
      extractCpfSubAccount(
        makePosition({ id: "h", symbol: "X", pf: { scheme: "CPF" }, baseValue: 100 }),
      ),
    ).toBeNull();
    expect(
      extractCpfSubAccount(
        makePosition({
          id: "h",
          symbol: "X",
          pf: { scheme: "CPF", subAccount: "ZZ" },
          baseValue: 100,
        }),
      ),
    ).toBeNull();
  });

  it("returns null sourceAccountId when missing or empty", () => {
    expect(
      extractSourceAccountId(
        makePosition({ id: "h", symbol: "X", pf: { scheme: "CPF" }, baseValue: 100 }),
      ),
    ).toBeNull();
    expect(
      extractSourceAccountId(
        makePosition({
          id: "h",
          symbol: "X",
          pf: { scheme: "CPF", sourceAccountId: "   " },
          baseValue: 100,
        }),
      ),
    ).toBeNull();
  });
});

describe("rollupByScheme", () => {
  it("§23 fixture: CPF + SRS sums grouped by scheme desc", () => {
    const holdings = [
      makePosition({
        id: "h1",
        symbol: "CPF-OA",
        pf: { scheme: "CPF", subAccount: "OA" },
        baseValue: 80_000,
      }),
      makePosition({
        id: "h2",
        symbol: "CPF-SA",
        pf: { scheme: "CPF", subAccount: "SA" },
        baseValue: 60_000,
      }),
      makePosition({
        id: "h3",
        symbol: "CPF-MA",
        pf: { scheme: "CPF", subAccount: "MA" },
        baseValue: 30_000,
      }),
      makePosition({
        id: "h4",
        symbol: "CPF-RA",
        pf: { scheme: "CPF", subAccount: "RA" },
        baseValue: 20_000,
      }),
      makePosition({
        id: "h5",
        symbol: "SRS-VOO",
        pf: { scheme: "OTHER" },
        baseValue: 40_000,
      }),
    ];
    const rows = rollupByScheme(holdings);
    expect(rows).toEqual([
      { scheme: "CPF", label: "CPF", totalValueBase: 190_000, positionCount: 4 },
      { scheme: "OTHER", label: "Other", totalValueBase: 40_000, positionCount: 1 },
    ]);
  });

  it("ignores non-PF holdings", () => {
    expect(
      rollupByScheme([
        makePosition({ id: "h", symbol: "AAPL", baseValue: 5000 }),
      ]),
    ).toEqual([]);
  });

  it("skips zero/negative values", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "X", pf: { scheme: "CPF" }, baseValue: 0 }),
      makePosition({ id: "h2", symbol: "Y", pf: { scheme: "CPF" }, baseValue: -10 }),
      makePosition({ id: "h3", symbol: "Z", pf: { scheme: "CPF" }, baseValue: 100 }),
    ];
    expect(rollupByScheme(holdings)).toEqual([
      { scheme: "CPF", label: "CPF", totalValueBase: 100, positionCount: 1 },
    ]);
  });
});

describe("rollupByCpfSubAccount", () => {
  it("returns OA/SA/MA/RA in canonical sequence (NOT desc by value)", () => {
    const holdings = [
      // Out of order on purpose to confirm the sort order is canonical
      makePosition({
        id: "h1",
        symbol: "RA",
        pf: { scheme: "CPF", subAccount: "RA" },
        baseValue: 200_000,
      }),
      makePosition({
        id: "h2",
        symbol: "MA",
        pf: { scheme: "CPF", subAccount: "MA" },
        baseValue: 30_000,
      }),
      makePosition({
        id: "h3",
        symbol: "OA",
        pf: { scheme: "CPF", subAccount: "OA" },
        baseValue: 50_000,
      }),
      makePosition({
        id: "h4",
        symbol: "SA",
        pf: { scheme: "CPF", subAccount: "SA" },
        baseValue: 100_000,
      }),
    ];
    expect(rollupByCpfSubAccount(holdings)).toEqual([
      { subAccount: "OA", totalValueBase: 50_000, positionCount: 1 },
      { subAccount: "SA", totalValueBase: 100_000, positionCount: 1 },
      { subAccount: "MA", totalValueBase: 30_000, positionCount: 1 },
      { subAccount: "RA", totalValueBase: 200_000, positionCount: 1 },
    ]);
  });

  it("omits sub-accounts with no balance", () => {
    const holdings = [
      makePosition({
        id: "h",
        symbol: "OA",
        pf: { scheme: "CPF", subAccount: "OA" },
        baseValue: 1000,
      }),
    ];
    const rows = rollupByCpfSubAccount(holdings);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.subAccount).toBe("OA");
  });

  it("excludes non-CPF holdings even if scheme.subAccount is OA", () => {
    const h = makePosition({
      id: "h",
      symbol: "X",
      pf: { scheme: "EPF", subAccount: "OA" },
      baseValue: 1000,
    });
    expect(rollupByCpfSubAccount([h])).toEqual([]);
  });

  it("excludes CPF holdings without a sub-account", () => {
    const h = makePosition({
      id: "h",
      symbol: "CPF-misc",
      pf: { scheme: "CPF" },
      baseValue: 1000,
    });
    expect(rollupByCpfSubAccount([h])).toEqual([]);
  });

  it("aggregates multiple positions in the same sub-account", () => {
    const holdings = [
      makePosition({
        id: "h1",
        symbol: "SGS-bond-1",
        pf: { scheme: "CPF", subAccount: "SA" },
        baseValue: 50_000,
      }),
      makePosition({
        id: "h2",
        symbol: "SGS-bond-2",
        pf: { scheme: "CPF", subAccount: "SA" },
        baseValue: 30_000,
      }),
    ];
    const rows = rollupByCpfSubAccount(holdings);
    expect(rows).toEqual([
      { subAccount: "SA", totalValueBase: 80_000, positionCount: 2 },
    ]);
  });
});

describe("rollupByPosition", () => {
  it("emits one row per holding desc by value with full metadata", () => {
    const holdings = [
      makePosition({
        id: "small",
        symbol: "MA-position",
        pf: { scheme: "CPF", subAccount: "MA", sourceAccountId: "acc-cpf" },
        baseValue: 10_000,
      }),
      makePosition({
        id: "big",
        symbol: "OA-position",
        pf: { scheme: "CPF", subAccount: "OA", sourceAccountId: "acc-cpf" },
        baseValue: 100_000,
      }),
    ];
    const rows = rollupByPosition(holdings);
    expect(rows.map((r) => r.holdingId)).toEqual(["big", "small"]);
    expect(rows[0]?.sourceAccountId).toBe("acc-cpf");
    expect(rows[0]?.subAccount).toBe("OA");
    expect(rows[0]?.schemeLabel).toBe("CPF");
  });

  it("name→symbol fallback when name missing", () => {
    const h = makePosition({
      id: "h",
      symbol: "EPF-RAW",
      pf: { scheme: "EPF" },
      baseValue: 1000,
    });
    if (h.instrument) h.instrument.name = null;
    expect(rollupByPosition([h])[0]?.label).toBe("EPF-RAW");
  });
});

describe("totalProvidentFundExposure", () => {
  it("sums PF-only holdings", () => {
    const holdings = [
      makePosition({ id: "h1", symbol: "X", pf: { scheme: "CPF" }, baseValue: 100 }),
      makePosition({ id: "h2", symbol: "Y", pf: { scheme: "EPF" }, baseValue: 50 }),
      makePosition({ id: "h3", symbol: "AAPL", baseValue: 9_999_999 }),
    ];
    expect(totalProvidentFundExposure(holdings)).toBe(150);
  });

  it("empty input returns 0", () => {
    expect(totalProvidentFundExposure([])).toBe(0);
  });
});
