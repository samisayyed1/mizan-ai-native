import { parse, parseISO, isValid, format as formatDate } from "date-fns";

import type { HoldingsSnapshotInput, HoldingsPositionInput } from "@/lib/types";
import type { DraftActivity } from "../context";
import { HoldingsFormat } from "../steps/holdings-mapping-step";
import { getDateFnsPattern } from "./date-format-options";

export const CASH_SYMBOL = "$CASH";

export interface ParseOptions {
  dateFormat: string;
  decimalSeparator: string;
  thousandsSeparator: string;
  defaultCurrency: string;
}

export interface HoldingsRowResolution {
  symbol?: string;
  exchangeMic?: string;
  assetId?: string;
}

export function buildHoldingsRowResolutionMap(
  drafts: DraftActivity[],
  assetIdByKey: Record<string, string> = {},
): Record<number, HoldingsRowResolution> {
  const resolutions: Record<number, HoldingsRowResolution> = {};

  for (const draft of drafts) {
    if (draft.rowIndex < 0) continue;

    const resolvedAssetId =
      draft.assetId ||
      (draft.importAssetKey ? assetIdByKey[draft.importAssetKey] : undefined) ||
      (draft.assetCandidateKey ? assetIdByKey[draft.assetCandidateKey] : undefined);

    if (!draft.symbol && !draft.exchangeMic && !resolvedAssetId) continue;

    resolutions[draft.rowIndex] = {
      symbol: draft.symbol,
      exchangeMic: draft.exchangeMic,
      assetId: resolvedAssetId,
    };
  }

  return resolutions;
}

export function parseNumericValue(
  value: string | undefined,
  decimalSeparator: string,
  thousandsSeparator: string,
): string | undefined {
  if (!value || value.trim() === "") return undefined;

  let normalized = value.trim();
  let isNegative = false;

  if (normalized.startsWith("(") && normalized.endsWith(")")) {
    isNegative = true;
    normalized = normalized.slice(1, -1);
  }

  const lastComma = normalized.lastIndexOf(",");
  const lastDot = normalized.lastIndexOf(".");
  let resolvedDecimal = decimalSeparator;
  if (decimalSeparator === "auto") {
    if (lastComma !== -1 && lastDot !== -1) {
      resolvedDecimal = lastComma > lastDot ? "," : ".";
    } else if (lastComma !== -1) {
      resolvedDecimal = ",";
    } else {
      resolvedDecimal = ".";
    }
  }

  let cleaned = normalized.replace(/[^\d.,+-]/g, "");

  if (thousandsSeparator !== "none" && thousandsSeparator !== "auto") {
    cleaned = cleaned.replace(new RegExp(`\\${thousandsSeparator}`, "g"), "");
  } else {
    const defaultThousands = resolvedDecimal === "," ? "." : ",";
    cleaned = cleaned.replace(new RegExp(`\\${defaultThousands}`, "g"), "");
  }

  if (resolvedDecimal === ",") {
    const parts = cleaned.split(",");
    if (parts.length > 1) {
      const decimalPart = parts.pop() ?? "";
      cleaned = `${parts.join("")}.${decimalPart}`;
    }
  } else {
    const parts = cleaned.split(".");
    if (parts.length > 1) {
      const decimalPart = parts.pop() ?? "";
      cleaned = `${parts.join("")}.${decimalPart}`;
    }
  }

  let candidate = cleaned;
  if (isNegative && candidate && !candidate.startsWith("-")) {
    candidate = `-${candidate}`;
  }

  if (candidate === "" || candidate === "-" || candidate === "+") {
    return undefined;
  }

  const numericCheck = Number(candidate);
  return Number.isFinite(numericCheck) ? candidate : undefined;
}

export function parseDateToYMD(dateStr: string, dateFormat: string): string | null {
  const trimmed = dateStr.trim();
  if (!trimmed) return null;

  const pattern = getDateFnsPattern(dateFormat);
  if (pattern) {
    try {
      const parsed = parse(trimmed, pattern, new Date());
      if (isValid(parsed)) return formatDate(parsed, "yyyy-MM-dd");
    } catch {
      // fall through to auto-detection
    }
  }

  if (dateFormat === "ISO8601") {
    try {
      const parsed = parseISO(trimmed);
      if (isValid(parsed)) return formatDate(parsed, "yyyy-MM-dd");
    } catch {
      // fall through
    }
  }

  const isoMatch = /^(\d{4})-(\d{1,2})-(\d{1,2})/.exec(trimmed);
  if (isoMatch) {
    try {
      const parsed = parseISO(trimmed);
      if (isValid(parsed)) return formatDate(parsed, "yyyy-MM-dd");
    } catch {
      // fall through
    }
  }

  const commonPatterns = [
    "MM/dd/yyyy",
    "dd/MM/yyyy",
    "MM-dd-yyyy",
    "dd-MM-yyyy",
    "dd.MM.yyyy",
    "MM.dd.yyyy",
    "yyyy/MM/dd",
  ];
  for (const p of commonPatterns) {
    try {
      const parsed = parse(trimmed, p, new Date());
      if (isValid(parsed)) return formatDate(parsed, "yyyy-MM-dd");
    } catch {
      continue;
    }
  }

  const date = new Date(trimmed);
  if (!isNaN(date.getTime())) {
    return formatDate(date, "yyyy-MM-dd");
  }

  return null;
}

/**
 * Detect whether the input CSV is a TRANSACTION LOG (one row per
 * BUY/SELL lot, e.g. Yahoo Portfolio export) versus a flat HOLDINGS
 * SNAPSHOT (one row per current position).
 *
 * A transaction log has a "Transaction Type" / "Trade Type" / "Action"
 * column with values like BUY / SELL. A flat snapshot doesn't.
 *
 * Returns `null` when no such column is found — the caller falls
 * back to the legacy "each row = one position" behaviour.
 *
 * We scan headers case-insensitively for a few common spellings
 * (Yahoo: "Transaction Type"; Zerodha: "Trade Type"; Robinhood:
 * "Trans Code"; Schwab: "Action").
 */
function findTransactionTypeColumn(headers: string[]): number | null {
  const candidates = [
    /^transaction\s*type$/i,
    /^trade\s*type$/i,
    /^trans(?:\s*code)?$/i,
    /^action$/i,
    /^activity\s*type$/i,
  ];
  for (let i = 0; i < headers.length; i++) {
    const h = headers[i]?.trim() ?? "";
    if (candidates.some((re) => re.test(h))) return i;
  }
  return null;
}

/** Canonicalise a transaction type string. Returns "BUY", "SELL", or null. */
function normaliseTransactionType(raw: string): "BUY" | "SELL" | null {
  const v = raw.trim().toUpperCase();
  if (!v) return null;
  if (/^(BUY|PURCHASE|LONG\s*BUY|COVER)$/.test(v)) return "BUY";
  if (/^(SELL|SALE|SHORT\s*SELL)$/.test(v)) return "SELL";
  return null;
}

/**
 * Parse rows from a CSV into one or more snapshots ready for the
 * `import_holdings_csv` Tauri command.
 *
 * Two input shapes are supported:
 *
 * 1. **Snapshot CSV** (legacy / explicit holdings-list path):
 *    Each row is one position-at-a-date. No Transaction Type column.
 *    Behaviour unchanged: rows grouped by date, pushed as positions.
 *
 * 2. **Transaction log CSV** (Yahoo Portfolio export and similar):
 *    Each row is a BUY or SELL lot with its own Trade Date. Detected
 *    by the presence of a "Transaction Type" / "Trade Type" / "Action"
 *    column with BUY/SELL values. We aggregate the log into a single
 *    snapshot dated **today** containing **net positions** per symbol:
 *      net qty   = Σ BUY qty − Σ SELL qty
 *      avg cost  = (Σ BUY qty × purchase_price) / (Σ BUY qty)
 *    Symbols whose net qty rounds to zero are dropped — they were
 *    fully closed out historically and shouldn't appear as positions.
 *
 *    Without this branch, a Yahoo Portfolio CSV imported via the
 *    holdings path used to:
 *      * treat SELL rows as positive quantity (inflating positions),
 *      * create one snapshot per Trade Date (hundreds of phantom
 *        snapshot rows, only the latest used by the dashboard),
 *      * silently undervalue the dashboard because the latest
 *        snapshot only captured the trades made on the latest date.
 *    Verified against uncle's 381-row Yahoo CSV: the bug produced
 *    a $189K dashboard total against an actual $260K market value.
 */
export function parseHoldingsSnapshots(
  headers: string[],
  rows: string[][],
  mapping: Record<string, string>,
  parseOptions: ParseOptions,
  symbolMappings?: Record<string, string>,
  symbolMeta?: Record<string, { exchangeMic?: string }>,
  rowResolutions?: Record<number, HoldingsRowResolution>,
): HoldingsSnapshotInput[] {
  const { dateFormat, decimalSeparator, thousandsSeparator, defaultCurrency } = parseOptions;

  const dateHeader = mapping[HoldingsFormat.DATE];
  const symbolHeader = mapping[HoldingsFormat.SYMBOL];
  const quantityHeader = mapping[HoldingsFormat.QUANTITY];
  const avgCostHeader = mapping[HoldingsFormat.AVG_COST];
  const currencyHeader = mapping[HoldingsFormat.CURRENCY];

  const dateIndex = dateHeader ? headers.indexOf(dateHeader) : -1;
  const symbolIndex = symbolHeader ? headers.indexOf(symbolHeader) : -1;
  const quantityIndex = quantityHeader ? headers.indexOf(quantityHeader) : -1;
  const avgCostIndex = avgCostHeader ? headers.indexOf(avgCostHeader) : -1;
  const currencyIndex = currencyHeader ? headers.indexOf(currencyHeader) : -1;
  const txTypeIndex = findTransactionTypeColumn(headers);

  // ── TRANSACTION LOG MODE ─────────────────────────────────────────
  // When the CSV looks like a transaction log (has a BUY/SELL column),
  // collapse it into a single net-positions snapshot dated today.
  if (txTypeIndex !== null) {
    type Accumulator = {
      buyQty: number;
      sellQty: number;
      buyCostBasis: number;
      currency: string;
      exchangeMic?: string;
      assetId?: string;
      symbol: string;
    };
    const bySymbol = new Map<string, Accumulator>();
    let cashBuyTotal = 0;
    let cashSellTotal = 0;
    const cashCurrency = defaultCurrency;

    for (let rowIndex = 0; rowIndex < rows.length; rowIndex++) {
      const row = rows[rowIndex];
      const rowResolution = rowResolutions?.[rowIndex];
      const rawSymbol = symbolIndex >= 0 ? row[symbolIndex]?.trim().toUpperCase() : "";
      const rawQuantity = quantityIndex >= 0 ? row[quantityIndex]?.trim() : "";
      const rawAvgCost = avgCostIndex >= 0 ? row[avgCostIndex]?.trim() : undefined;
      const rawTxType = row[txTypeIndex]?.trim() ?? "";
      const currency = (currencyIndex >= 0 ? row[currencyIndex]?.trim() : "") || defaultCurrency;

      // Skip watchlist rows: any of symbol / quantity / tx-type empty.
      if (!rawSymbol || !rawQuantity || !rawTxType) continue;

      const txType = normaliseTransactionType(rawTxType);
      if (!txType) continue; // unknown tx type (e.g. DIV/INT) — handled
      // by the activity-import path; not relevant to holdings snapshot.

      const qtyParsed = parseNumericValue(rawQuantity, decimalSeparator, thousandsSeparator);
      const priceParsed = parseNumericValue(rawAvgCost, decimalSeparator, thousandsSeparator);
      if (!qtyParsed) continue;
      const qty = parseFloat(qtyParsed);
      const price = priceParsed ? parseFloat(priceParsed) : 0;
      if (!Number.isFinite(qty) || qty <= 0) continue;
      // Zero-price rows are usually placeholder/junk ("free shares"
      // typed with no cost basis); skip to avoid polluting the
      // weighted-avg cost calculation.
      if (!Number.isFinite(price) || price <= 0) continue;

      const symbol = rowResolution?.symbol || symbolMappings?.[rawSymbol] || rawSymbol;

      if (symbol === CASH_SYMBOL) {
        if (txType === "BUY") cashBuyTotal += qty;
        else cashSellTotal += qty;
        continue;
      }

      const exchangeMic =
        rowResolution?.exchangeMic ??
        symbolMeta?.[rawSymbol]?.exchangeMic ??
        symbolMeta?.[symbol]?.exchangeMic;
      const assetId = rowResolution?.assetId;

      const acc = bySymbol.get(symbol) ?? {
        buyQty: 0,
        sellQty: 0,
        buyCostBasis: 0,
        currency,
        exchangeMic,
        assetId,
        symbol,
      };
      if (txType === "BUY") {
        acc.buyQty += qty;
        acc.buyCostBasis += qty * price;
      } else {
        acc.sellQty += qty;
      }
      // Late-arriving exchange / asset metadata wins (the parser
      // resolved it for one row but not another — first non-empty
      // resolution sticks).
      if (!acc.exchangeMic && exchangeMic) acc.exchangeMic = exchangeMic;
      if (!acc.assetId && assetId) acc.assetId = assetId;
      bySymbol.set(symbol, acc);
    }

    const positions: HoldingsPositionInput[] = [];
    for (const acc of bySymbol.values()) {
      const netQty = acc.buyQty - acc.sellQty;
      // Drop fully closed positions (net qty rounds to zero).
      if (Math.abs(netQty) < 1e-9) continue;
      // Weighted-average cost basis from BUY lots only. Note: this
      // does NOT account for FIFO/LIFO disposal of sold shares —
      // that would require the activity-import path. This is a
      // reasonable approximation for a snapshot when the user
      // explicitly chose the holdings-import flow.
      const avgCost = acc.buyQty > 0 ? acc.buyCostBasis / acc.buyQty : 0;
      positions.push({
        symbol: acc.symbol,
        quantity: String(netQty),
        avgCost: avgCost > 0 ? String(avgCost) : undefined,
        currency: acc.currency,
        ...(acc.exchangeMic ? { exchangeMic: acc.exchangeMic } : {}),
        ...(acc.assetId ? { assetId: acc.assetId } : {}),
      });
    }

    const cashBalances: Record<string, string> = {};
    const netCash = cashBuyTotal - cashSellTotal;
    if (Math.abs(netCash) > 1e-9) {
      cashBalances[cashCurrency] = String(netCash);
    }

    // Single snapshot dated today — the user wants to know what they
    // currently hold, not a history.
    const today = formatDate(new Date(), "yyyy-MM-dd");
    return [{ date: today, positions, cashBalances }];
  }

  // ── SNAPSHOT MODE (legacy / explicit) ────────────────────────────
  // No Transaction Type column — treat each row as a position-at-date.

  const snapshotsByDate = new Map<
    string,
    { positions: HoldingsPositionInput[]; cashBalances: Record<string, string> }
  >();

  for (let rowIndex = 0; rowIndex < rows.length; rowIndex++) {
    const row = rows[rowIndex];
    const rowResolution = rowResolutions?.[rowIndex];
    const rawDate = dateIndex >= 0 ? row[dateIndex]?.trim() : "";
    const rawSymbol = symbolIndex >= 0 ? row[symbolIndex]?.trim().toUpperCase() : "";
    const rawQuantity = quantityIndex >= 0 ? row[quantityIndex]?.trim() : "";
    const rawAvgCost = avgCostIndex >= 0 ? row[avgCostIndex]?.trim() : undefined;
    const currency = currencyIndex >= 0 ? row[currencyIndex]?.trim() : defaultCurrency;

    if (!rawDate || !rawSymbol || !rawQuantity) continue;

    const normalizedDate = parseDateToYMD(rawDate, dateFormat);
    if (!normalizedDate) continue;

    const quantity = parseNumericValue(rawQuantity, decimalSeparator, thousandsSeparator);
    if (!quantity) continue;
    const avgCost = parseNumericValue(rawAvgCost, decimalSeparator, thousandsSeparator);

    if (!snapshotsByDate.has(normalizedDate)) {
      snapshotsByDate.set(normalizedDate, { positions: [], cashBalances: {} });
    }

    const snapshot = snapshotsByDate.get(normalizedDate)!;
    const symbol = rowResolution?.symbol || symbolMappings?.[rawSymbol] || rawSymbol;

    if (symbol === CASH_SYMBOL) {
      const cashCurrency = currency || defaultCurrency;
      const existingAmount = parseFloat(snapshot.cashBalances[cashCurrency] || "0");
      const newAmount = parseFloat(quantity) || 0;
      snapshot.cashBalances[cashCurrency] = String(existingAmount + newAmount);
    } else {
      const exchangeMic =
        rowResolution?.exchangeMic ??
        symbolMeta?.[rawSymbol]?.exchangeMic ??
        symbolMeta?.[symbol]?.exchangeMic;
      const assetId = rowResolution?.assetId;
      snapshot.positions.push({
        symbol,
        quantity,
        avgCost: avgCost || undefined,
        currency: currency || defaultCurrency,
        ...(exchangeMic ? { exchangeMic } : {}),
        ...(assetId ? { assetId } : {}),
      });
    }
  }

  const snapshots: HoldingsSnapshotInput[] = [];
  for (const [date, data] of snapshotsByDate.entries()) {
    snapshots.push({
      date,
      positions: data.positions,
      cashBalances: data.cashBalances,
    });
  }

  snapshots.sort((left, right) => right.date.localeCompare(left.date));

  return snapshots;
}
