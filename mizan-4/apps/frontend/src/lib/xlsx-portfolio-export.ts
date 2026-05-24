// Portfolio → XLSX workbook with one tab per data section.
//
// Built for Feroz: the older HNW users who prefer Excel/Numbers and
// want a single file with everything inside. Pure function — takes
// the data already fetched by TanStack Query elsewhere on the page
// and returns the workbook bytes. No network calls here; no shell
// calls. Fully unit-testable.
//
// ExcelJS is loaded dynamically by the caller so the main bundle is
// unaffected for users who never open the export screen.

import type ExcelJS from "exceljs";

import type { Account, AccountValuation, ActivityDetails, Goal, Holding } from "@/lib/types";

export interface PortfolioWorkbookInput {
  generatedAt: Date;
  baseCurrency: string;
  accounts: readonly Account[];
  activities: readonly ActivityDetails[];
  holdings: readonly Holding[];
  goals: readonly Goal[];
  portfolioHistory: readonly AccountValuation[];
}

/** Names of the sheets we produce. Stable contract for tests + UI. */
export const SHEET_NAMES = {
  summary: "Summary",
  accounts: "Portfolios",
  holdings: "Holdings",
  activities: "Activities",
  goals: "Goals",
  history: "Portfolio History",
} as const;

const DECIMAL_FMT = "#,##0.00";
const QUANTITY_FMT = "#,##0.0000";
const DATE_FMT = "yyyy-mm-dd";

const HEADER_STYLE: Partial<ExcelJS.Style> = {
  font: { bold: true },
  fill: {
    type: "pattern",
    pattern: "solid",
    fgColor: { argb: "FFF1F1F1" },
  },
  alignment: { vertical: "middle" },
};

/**
 * Build the multi-tab portfolio workbook. Returns the XLSX file as
 * a Uint8Array suitable for `openFileSaveDialog`.
 *
 * Dynamic-imports ExcelJS so the main bundle stays lean — users who
 * never visit the export screen never pay the cost.
 */
export async function buildPortfolioWorkbook(input: PortfolioWorkbookInput): Promise<Uint8Array> {
  const ExcelJSMod = await import("exceljs");
  const wb = new ExcelJSMod.Workbook();
  wb.creator = "Mizan";
  wb.created = input.generatedAt;

  writeSummary(wb, input);
  writeAccounts(wb, input);
  writeHoldings(wb, input);
  writeActivities(wb, input);
  writeGoals(wb, input);
  writeHistory(wb, input);

  const buffer = await wb.xlsx.writeBuffer();
  return new Uint8Array(buffer as ArrayBuffer);
}

// ---------------------------------------------------------------------------
// Sheet builders — each one is small, pure, and individually exported so
// tests can exercise the column layout without re-running the workbook.
// ---------------------------------------------------------------------------

function writeSummary(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.summary);
  ws.columns = [
    { header: "Field", key: "field", width: 32 },
    { header: "Value", key: "value", width: 40 },
  ];
  applyHeader(ws.getRow(1));

  const totalCurrentValue = sumLatestAccountValue(input.portfolioHistory);
  const accountsActive = input.accounts.filter((a) => a.isActive).length;

  ws.addRow({ field: "Generated at", value: input.generatedAt.toISOString() });
  ws.addRow({ field: "Base currency", value: input.baseCurrency });
  ws.addRow({
    field: "Portfolios (active / total)",
    value: `${accountsActive} / ${input.accounts.length}`,
  });
  ws.addRow({ field: "Holdings rows", value: input.holdings.length });
  ws.addRow({ field: "Activities rows", value: input.activities.length });
  ws.addRow({ field: "Goals", value: input.goals.length });
  ws.addRow({ field: "Portfolio history rows", value: input.portfolioHistory.length });

  const totalRow = ws.addRow({
    field: `Latest aggregate value (${input.baseCurrency})`,
    value: totalCurrentValue,
  });
  totalRow.getCell("value").numFmt = DECIMAL_FMT;
  totalRow.font = { bold: true };

  // A tiny "Sheets" map so future generations of the workbook can
  // tell what was in this build without re-running the export.
  ws.addRow({});
  const sectionsHeader = ws.addRow({ field: "Sheets included", value: "" });
  sectionsHeader.font = { bold: true };
  for (const name of Object.values(SHEET_NAMES)) {
    if (name === SHEET_NAMES.summary) continue;
    ws.addRow({ field: "  · " + name, value: "" });
  }
}

function writeAccounts(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.accounts);
  ws.columns = [
    { header: "ID", key: "id", width: 24 },
    { header: "Name", key: "name", width: 32 },
    { header: "Type", key: "type", width: 14 },
    { header: "Group", key: "group", width: 18 },
    { header: "Currency", key: "currency", width: 10 },
    { header: "Default", key: "isDefault", width: 10 },
    { header: "Active", key: "isActive", width: 10 },
    { header: "Provider", key: "provider", width: 16 },
    { header: "Platform ID", key: "platformId", width: 18 },
    { header: "Account number", key: "accountNumber", width: 22 },
    { header: "Tracking mode", key: "trackingMode", width: 16 },
    { header: "Created at", key: "createdAt", width: 22 },
  ];
  applyHeader(ws.getRow(1));

  for (const a of input.accounts) {
    ws.addRow({
      id: a.id,
      name: a.name,
      type: a.accountType,
      group: a.group ?? "",
      currency: a.currency,
      isDefault: a.isDefault ? "Yes" : "No",
      isActive: a.isActive ? "Yes" : "No",
      provider: a.provider ?? "",
      platformId: a.platformId ?? "",
      accountNumber: a.accountNumber ?? "",
      trackingMode: a.trackingMode,
      createdAt:
        a.createdAt instanceof Date ? a.createdAt.toISOString() : String(a.createdAt ?? ""),
    });
  }
}

function writeHoldings(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.holdings);
  ws.columns = [
    { header: "Symbol", key: "symbol", width: 14 },
    { header: "Name", key: "name", width: 30 },
    { header: "Account ID", key: "accountId", width: 24 },
    { header: "Asset kind", key: "assetKind", width: 14 },
    { header: "Quantity", key: "quantity", width: 14 },
    { header: "Local currency", key: "localCurrency", width: 12 },
    { header: "Market value (local)", key: "marketValueLocal", width: 18 },
    { header: `Market value (${input.baseCurrency})`, key: "marketValueBase", width: 22 },
    { header: `Cost basis (${input.baseCurrency})`, key: "costBasisBase", width: 22 },
    { header: `Unrealized gain (${input.baseCurrency})`, key: "unrealizedBase", width: 22 },
    { header: "Open date", key: "openDate", width: 14 },
  ];
  applyHeader(ws.getRow(1));

  for (const h of input.holdings) {
    const row = ws.addRow({
      symbol: h.instrument?.symbol ?? "",
      name: h.instrument?.name ?? "",
      accountId: h.accountId,
      assetKind: h.assetKind ?? "",
      quantity: h.quantity,
      localCurrency: h.localCurrency,
      marketValueLocal: h.marketValue?.local ?? null,
      marketValueBase: h.marketValue?.base ?? null,
      costBasisBase: h.costBasis?.base ?? null,
      unrealizedBase: h.unrealizedGain?.base ?? null,
      openDate: (h.openDate as unknown as string) ?? "",
    });
    row.getCell("quantity").numFmt = QUANTITY_FMT;
    for (const k of ["marketValueLocal", "marketValueBase", "costBasisBase", "unrealizedBase"]) {
      row.getCell(k).numFmt = DECIMAL_FMT;
    }
  }
}

function writeActivities(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.activities);
  ws.columns = [
    { header: "ID", key: "id", width: 22 },
    { header: "Date", key: "date", width: 14 },
    { header: "Account ID", key: "accountId", width: 24 },
    { header: "Account", key: "accountName", width: 20 },
    { header: "Symbol", key: "symbol", width: 14 },
    { header: "Asset name", key: "assetName", width: 28 },
    { header: "Type", key: "type", width: 14 },
    { header: "Status", key: "status", width: 12 },
    { header: "Quantity", key: "quantity", width: 14 },
    { header: "Unit price", key: "unitPrice", width: 14 },
    { header: "Amount", key: "amount", width: 14 },
    { header: "Currency", key: "currency", width: 10 },
    { header: "Fee", key: "fee", width: 12 },
    { header: "FX rate", key: "fxRate", width: 12 },
    { header: "Comment", key: "comment", width: 40 },
  ];
  applyHeader(ws.getRow(1));

  for (const t of input.activities) {
    const row = ws.addRow({
      id: t.id,
      date: t.date,
      accountId: t.accountId,
      accountName: t.accountName,
      symbol: t.assetSymbol ?? "",
      assetName: t.assetName ?? "",
      type: t.activityType,
      status: t.status ?? "",
      quantity: parseNumOrNull(t.quantity),
      unitPrice: parseNumOrNull(t.unitPrice),
      amount: parseNumOrNull(t.amount),
      currency: t.currency,
      fee: parseNumOrNull(t.fee),
      fxRate: parseNumOrNull(t.fxRate),
      comment: t.comment ?? "",
    });
    row.getCell("date").numFmt = DATE_FMT;
    row.getCell("quantity").numFmt = QUANTITY_FMT;
    for (const k of ["unitPrice", "amount", "fee", "fxRate"]) {
      row.getCell(k).numFmt = DECIMAL_FMT;
    }
  }
}

function writeGoals(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.goals);
  ws.columns = [
    { header: "ID", key: "id", width: 22 },
    { header: "Title", key: "title", width: 32 },
    { header: "Type", key: "type", width: 14 },
    { header: "Lifecycle", key: "lifecycle", width: 14 },
    { header: "Health", key: "health", width: 14 },
    { header: "Priority", key: "priority", width: 10 },
    { header: "Target amount", key: "target", width: 16 },
    { header: "Current value", key: "current", width: 16 },
    { header: "Progress", key: "progress", width: 12 },
    { header: "Currency", key: "currency", width: 10 },
    { header: "Target date", key: "targetDate", width: 14 },
  ];
  applyHeader(ws.getRow(1));

  for (const g of input.goals) {
    const row = ws.addRow({
      id: g.id,
      title: g.title,
      type: g.goalType,
      lifecycle: g.statusLifecycle,
      health: g.statusHealth,
      priority: g.priority,
      target: g.targetAmount ?? null,
      current: g.summaryCurrentValue ?? null,
      progress: g.summaryProgress ?? null,
      currency: g.currency ?? "",
      targetDate: g.targetDate ?? "",
    });
    row.getCell("target").numFmt = DECIMAL_FMT;
    row.getCell("current").numFmt = DECIMAL_FMT;
    row.getCell("progress").numFmt = "0.00%";
  }
}

function writeHistory(wb: ExcelJS.Workbook, input: PortfolioWorkbookInput): void {
  const ws = wb.addWorksheet(SHEET_NAMES.history);
  ws.columns = [
    { header: "Date", key: "date", width: 14 },
    { header: "Account ID", key: "accountId", width: 24 },
    { header: "Account currency", key: "accountCurrency", width: 14 },
    { header: "Base currency", key: "baseCurrency", width: 14 },
    { header: "FX rate to base", key: "fxRate", width: 14 },
    { header: "Cash balance", key: "cash", width: 18 },
    { header: "Investment market value", key: "invest", width: 22 },
    { header: "Total value", key: "total", width: 18 },
    { header: "Cost basis", key: "costBasis", width: 18 },
    { header: "Net contribution", key: "netContribution", width: 18 },
  ];
  applyHeader(ws.getRow(1));

  for (const v of input.portfolioHistory) {
    const row = ws.addRow({
      date: v.valuationDate,
      accountId: v.accountId,
      accountCurrency: v.accountCurrency,
      baseCurrency: v.baseCurrency,
      fxRate: v.fxRateToBase,
      cash: v.cashBalance,
      invest: v.investmentMarketValue,
      total: v.totalValue,
      costBasis: v.costBasis,
      netContribution: v.netContribution,
    });
    row.getCell("date").numFmt = DATE_FMT;
    for (const k of ["fxRate", "cash", "invest", "total", "costBasis", "netContribution"]) {
      row.getCell(k).numFmt = DECIMAL_FMT;
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers (exported for tests)
// ---------------------------------------------------------------------------

export function sumLatestAccountValue(history: readonly AccountValuation[]): number {
  // For each account, take the most recent valuation row (by date) and
  // sum its `totalValue` in the base currency.
  const latestByAccount = new Map<string, AccountValuation>();
  for (const row of history) {
    const prev = latestByAccount.get(row.accountId);
    if (!prev || (row.valuationDate ?? "") > (prev.valuationDate ?? "")) {
      latestByAccount.set(row.accountId, row);
    }
  }
  let total = 0;
  for (const row of latestByAccount.values()) {
    const v = parseNumOrNull(row.totalValue);
    const fx = parseNumOrNull(row.fxRateToBase) ?? 1;
    if (v !== null) total += v * fx;
  }
  return Math.round(total * 100) / 100;
}

function parseNumOrNull(value: unknown): number | null {
  if (value === null || value === undefined || value === "") return null;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

function applyHeader(row: ExcelJS.Row): void {
  row.eachCell((cell) => {
    cell.font = HEADER_STYLE.font ?? cell.font;
    cell.fill = HEADER_STYLE.fill ?? cell.fill;
    cell.alignment = HEADER_STYLE.alignment ?? cell.alignment;
  });
}

/**
 * Default filename — Mizan-portfolio-YYYY-MM-DD.xlsx, stable across
 * platforms.
 */
export function defaultPortfolioFileName(d: Date): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `Mizan-portfolio-${y}-${m}-${day}.xlsx`;
}
