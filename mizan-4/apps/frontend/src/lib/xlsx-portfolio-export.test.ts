import { describe, expect, it } from "vitest";
import ExcelJS from "exceljs";

// Buffer/Uint8Array compat note: ExcelJS v4's `wb.xlsx.load(...)`
// declares its parameter against an older `@types/node` Buffer shape
// (Symbol.toStringTag === "ArrayBuffer"). Under @types/node v24+ the
// runtime Buffer extends Uint8Array (toStringTag === "Uint8Array"),
// so no cast through any Buffer<T> shape satisfies ExcelJS's old type.
// Each `wb.xlsx.load(bytes)` below uses `@ts-expect-error` to bypass
// the structural check — at runtime ExcelJS treats the argument as a
// byte view, so passing a `Uint8Array<ArrayBufferLike>` works as it
// always has.

import type { Account, AccountValuation, ActivityDetails, Goal, Holding } from "@/lib/types";

import {
  SHEET_NAMES,
  buildPortfolioWorkbook,
  defaultPortfolioFileName,
  sumLatestAccountValue,
  type PortfolioWorkbookInput,
} from "./xlsx-portfolio-export";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function account(over: Partial<Account> = {}): Account {
  return {
    id: "ACC-1",
    name: "Main",
    accountType: "SECURITIES",
    group: "Investing",
    balance: 0,
    currency: "USD",
    isDefault: true,
    isActive: true,
    isArchived: false,
    trackingMode: "PORTFOLIO",
    createdAt: new Date("2026-01-01T00:00:00Z"),
    updatedAt: new Date("2026-01-01T00:00:00Z"),
    ...over,
  } as Account;
}

function activity(over: Partial<ActivityDetails> = {}): ActivityDetails {
  return {
    id: "ACT-1",
    activityType: "BUY",
    date: new Date("2026-05-01T00:00:00Z"),
    quantity: "10",
    unitPrice: "150.00",
    amount: "1500.00",
    fee: "1.99",
    currency: "USD",
    needsReview: false,
    createdAt: new Date(),
    updatedAt: new Date(),
    assetId: "AAPL",
    accountId: "ACC-1",
    accountName: "Main",
    accountCurrency: "USD",
    assetSymbol: "AAPL",
    ...over,
  } as ActivityDetails;
}

function holding(over: Partial<Holding> = {}): Holding {
  return {
    id: "H-1",
    holdingType: "POSITION",
    accountId: "ACC-1",
    instrument: { id: "AAPL", symbol: "AAPL", name: "Apple Inc." },
    assetKind: "INVESTMENT",
    quantity: 10,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: { local: 1700, base: 1700 },
    costBasis: { local: 1500, base: 1500 },
    unrealizedGain: { local: 200, base: 200 },
    ...over,
  } as Holding;
}

function goal(over: Partial<Goal> = {}): Goal {
  return {
    id: "GOAL-1",
    goalType: "RETIREMENT",
    title: "Retirement",
    targetAmount: 1_000_000,
    statusLifecycle: "ACTIVE",
    statusHealth: "ON_TRACK",
    priority: 1,
    currency: "USD",
    targetDate: "2055-01-01",
    summaryCurrentValue: 250_000,
    summaryProgress: 0.25,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
    ...over,
  } as Goal;
}

function valuation(over: Partial<AccountValuation> = {}): AccountValuation {
  return {
    id: "VAL-1",
    accountId: "ACC-1",
    valuationDate: "2026-05-01",
    accountCurrency: "USD",
    baseCurrency: "USD",
    fxRateToBase: 1,
    cashBalance: 100,
    investmentMarketValue: 1700,
    totalValue: 1800,
    costBasis: 1500,
    netContribution: 1600,
    calculatedAt: "2026-05-01T00:00:00Z",
    ...over,
  } as AccountValuation;
}

function makeInput(over: Partial<PortfolioWorkbookInput> = {}): PortfolioWorkbookInput {
  return {
    generatedAt: new Date(Date.UTC(2026, 4, 15, 10, 0, 0)),
    baseCurrency: "USD",
    accounts: [account()],
    activities: [activity()],
    holdings: [holding()],
    goals: [goal()],
    portfolioHistory: [valuation()],
    ...over,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("buildPortfolioWorkbook", () => {
  it("produces a non-empty Uint8Array that parses back into a valid xlsx", async () => {
    const bytes = await buildPortfolioWorkbook(makeInput());
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(bytes.byteLength).toBeGreaterThan(0);

    // Round-trip: read back with ExcelJS to confirm the file is valid.
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);
    const names = wb.worksheets.map((s) => s.name);
    expect(names).toEqual([
      SHEET_NAMES.summary,
      SHEET_NAMES.accounts,
      SHEET_NAMES.holdings,
      SHEET_NAMES.activities,
      SHEET_NAMES.goals,
      SHEET_NAMES.history,
    ]);
  });

  it("Accounts sheet has the expected header + one row per account", async () => {
    const bytes = await buildPortfolioWorkbook(
      makeInput({ accounts: [account({ id: "ACC-1" }), account({ id: "ACC-2", name: "Cash" })] }),
    );
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);

    const ws = wb.getWorksheet(SHEET_NAMES.accounts);
    if (!ws) throw new Error("accounts sheet missing");
    const header = (ws.getRow(1).values as unknown[]).slice(1, 8);
    expect(header).toEqual(["ID", "Name", "Type", "Group", "Currency", "Default", "Active"]);
    expect(ws.rowCount).toBe(3); // header + 2 rows
  });

  it("Holdings sheet uses the base currency in column headers", async () => {
    const input = makeInput({ baseCurrency: "EUR" });
    const bytes = await buildPortfolioWorkbook(input);
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);
    const ws = wb.getWorksheet(SHEET_NAMES.holdings);
    if (!ws) throw new Error("holdings sheet missing");
    const headerJoined = (ws.getRow(1).values as unknown[]).join(" | ");
    expect(headerJoined).toContain("Market value (EUR)");
    expect(headerJoined).toContain("Cost basis (EUR)");
    expect(headerJoined).toContain("Unrealized gain (EUR)");
  });

  it("Activities sheet parses string-decimal fields back to numbers", async () => {
    const bytes = await buildPortfolioWorkbook(makeInput());
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);
    const ws = wb.getWorksheet(SHEET_NAMES.activities);
    if (!ws) throw new Error("activities sheet missing");

    // Column keys are stripped when a workbook is round-tripped through
    // xlsx bytes, so we read by 1-based column number. Layout (per
    // writeActivities): 1 ID · 2 Date · 3 Account ID · 4 Account
    // · 5 Symbol · 6 Asset name · 7 Type · 8 Status · 9 Quantity
    // · 10 Unit price · 11 Amount · 12 Currency · 13 Fee
    // · 14 FX rate · 15 Comment.
    const dataRow = ws.getRow(2);
    expect(typeof dataRow.getCell(9).value).toBe("number");
    expect(dataRow.getCell(9).value).toBe(10);
    expect(dataRow.getCell(10).value).toBe(150);
    expect(dataRow.getCell(11).value).toBe(1500);
    expect(dataRow.getCell(13).value).toBe(1.99);
  });

  it("Summary sheet captures generated-at + base-currency + counts", async () => {
    const input = makeInput({
      accounts: [account(), account({ id: "ACC-2", isActive: false })],
      activities: [activity(), activity({ id: "ACT-2" })],
    });
    const bytes = await buildPortfolioWorkbook(input);
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);
    const ws = wb.getWorksheet(SHEET_NAMES.summary);
    if (!ws) throw new Error("summary sheet missing");

    const pairs: Record<string, unknown> = {};
    for (let r = 2; r <= ws.rowCount; r++) {
      // Summary layout: column 1 = "Field", column 2 = "Value".
      // ExcelJS CellValue is a union (string | number | Date | RichText
      // | …) so coerce only the string case — otherwise `String(obj)`
      // would silently turn a stray object into "[object Object]".
      const rawField = ws.getRow(r).getCell(1).value;
      const field = typeof rawField === "string" ? rawField : "";
      const value = ws.getRow(r).getCell(2).value;
      if (field) pairs[field] = value;
    }
    expect(pairs["Base currency"]).toBe("USD");
    expect(pairs["Portfolios (active / total)"]).toBe("1 / 2");
    expect(pairs["Activities rows"]).toBe(2);
    expect(pairs.Goals).toBe(1);
  });

  it("renders an empty-but-valid workbook when the user has no data yet", async () => {
    const bytes = await buildPortfolioWorkbook(
      makeInput({ accounts: [], activities: [], holdings: [], goals: [], portfolioHistory: [] }),
    );
    const wb = new ExcelJS.Workbook();
    // @ts-expect-error – see Buffer/Uint8Array compat note at top of file
    await wb.xlsx.load(bytes);
    expect(wb.worksheets).toHaveLength(6);
    // Every data sheet should have at least the header row.
    for (const name of [
      SHEET_NAMES.accounts,
      SHEET_NAMES.holdings,
      SHEET_NAMES.activities,
      SHEET_NAMES.goals,
      SHEET_NAMES.history,
    ]) {
      const ws = wb.getWorksheet(name);
      if (!ws) throw new Error(`${name} sheet missing`);
      expect(ws.rowCount).toBe(1);
    }
  });
});

describe("sumLatestAccountValue", () => {
  it("sums the latest valuation row per account, converted into base currency", () => {
    const v: AccountValuation[] = [
      valuation({ accountId: "A", valuationDate: "2026-04-01", totalValue: 100, fxRateToBase: 1 }),
      valuation({
        accountId: "A",
        valuationDate: "2026-05-01",
        totalValue: 200,
        fxRateToBase: 1,
      }),
      valuation({
        accountId: "B",
        valuationDate: "2026-05-01",
        totalValue: 1000,
        fxRateToBase: 0.8,
      }),
    ];
    expect(sumLatestAccountValue(v)).toBe(200 + 800);
  });

  it("returns 0 when there is no history", () => {
    expect(sumLatestAccountValue([])).toBe(0);
  });
});

describe("defaultPortfolioFileName", () => {
  it("builds Mizan-portfolio-YYYY-MM-DD.xlsx in UTC", () => {
    expect(defaultPortfolioFileName(new Date(Date.UTC(2026, 4, 15)))).toBe(
      "Mizan-portfolio-2026-05-15.xlsx",
    );
  });
});
