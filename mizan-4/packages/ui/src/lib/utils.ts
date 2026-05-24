import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import { DECIMAL_PRECISION, DISPLAY_DECIMAL_PRECISION } from "./constants";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Format amount with currency support, including special handling for pence (GBp/GBX)
 */
const DECIMAL_FORMAT_OPTIONS: Intl.NumberFormatOptions = {
  minimumFractionDigits: DISPLAY_DECIMAL_PRECISION,
  maximumFractionDigits: DISPLAY_DECIMAL_PRECISION,
};

const decimalFormatter = new Intl.NumberFormat("en-US", DECIMAL_FORMAT_OPTIONS);
const currencyFormatterCache = new Map<string, Intl.NumberFormat>();
const compactCurrencyFormatterCache = new Map<string, Intl.NumberFormat>();

const getCurrencyFormatter = (currency: string) => {
  const normalizedCurrency = currency?.toUpperCase?.() ?? "USD";
  const cacheKey = normalizedCurrency;

  if (currencyFormatterCache.has(cacheKey)) {
    return currencyFormatterCache.get(cacheKey)!;
  }

  let formatter: Intl.NumberFormat;
  try {
    formatter = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: normalizedCurrency,
      ...DECIMAL_FORMAT_OPTIONS,
    });
  } catch {
    formatter = decimalFormatter;
  }

  currencyFormatterCache.set(cacheKey, formatter);
  return formatter;
};

const getCompactCurrencyFormatter = (currency: string, maximumFractionDigits: number) => {
  const normalizedCurrency = currency?.toUpperCase?.() ?? "USD";
  const cacheKey = `${normalizedCurrency}:${maximumFractionDigits}`;

  if (compactCurrencyFormatterCache.has(cacheKey)) {
    return compactCurrencyFormatterCache.get(cacheKey)!;
  }

  let formatter: Intl.NumberFormat;
  try {
    formatter = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: normalizedCurrency,
      notation: "compact",
      maximumFractionDigits,
    });
  } catch {
    formatter = decimalFormatter;
  }

  compactCurrencyFormatterCache.set(cacheKey, formatter);
  return formatter;
};

export function formatAmount(
  amount: number | string | null | undefined,
  currency: string,
  displayCurrency = true,
) {
  if (amount == null) return "-";
  const numericAmount = typeof amount === "string" ? Number(amount) : amount;
  if (!Number.isFinite(numericAmount)) return "-";
  const displayAmount = Math.abs(numericAmount) < 0.005 ? 0 : numericAmount;
  const rawCurrency = currency ?? "USD";
  const isPenceCurrency = rawCurrency === "GBp" || rawCurrency === "GBX";

  if (isPenceCurrency) {
    const formattedNumber = decimalFormatter.format(displayAmount);
    return displayCurrency ? `${formattedNumber}p` : formattedNumber;
  }

  if (!displayCurrency) {
    return decimalFormatter.format(displayAmount);
  }

  return getCurrencyFormatter(rawCurrency).format(displayAmount);
}

/**
 * Format a per-unit price. Unlike `formatAmount` which rounds to the
 * currency's display precision (2 dp), this preserves sub-cent precision
 * for crypto / penny stocks / micro-priced tokens.
 *
 * Rule:
 *   - |price| >= 1   → standard currency format (2 decimals).
 *   - |price| < 1    → up to 8 decimals, trailing zeros past the
 *                       2-decimal minimum are trimmed.
 *   - non-finite / nullish / empty string → "-".
 *
 * Accepts string or number on input; a precision-preserving
 * Decimal-as-string from the CSV import path survives until display.
 */
export function formatPrice(
  price: number | string | null | undefined,
  currency: string,
  displayCurrency = true,
): string {
  if (price == null || price === "") return "-";
  const numericPrice = typeof price === "string" ? Number(price) : price;
  if (!Number.isFinite(numericPrice)) return "-";

  if (Math.abs(numericPrice) >= 1) {
    return formatAmount(numericPrice, currency, displayCurrency);
  }

  const rawCurrency = currency ?? "USD";
  const isPenceCurrency = rawCurrency === "GBp" || rawCurrency === "GBX";

  const precisionFormatter = new Intl.NumberFormat("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 8,
    useGrouping: false,
  });
  const formattedNumber = precisionFormatter.format(numericPrice);

  if (isPenceCurrency) {
    return displayCurrency ? `${formattedNumber}p` : formattedNumber;
  }
  if (!displayCurrency) {
    return formattedNumber;
  }

  try {
    const parts = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: rawCurrency.toUpperCase(),
    }).formatToParts(0);
    const symbol = parts.find((p) => p.type === "currency")?.value ?? "";
    return `${symbol}${formattedNumber}`;
  } catch {
    return formattedNumber;
  }
}

export function formatCompactAmount(
  amount: number | string | null | undefined,
  currency: string,
  displayCurrency = true,
) {
  if (amount == null) return "-";
  const numericAmount = typeof amount === "string" ? Number(amount) : amount;
  if (!Number.isFinite(numericAmount)) return "-";
  const rawCurrency = currency ?? "USD";
  const abs = Math.abs(numericAmount);
  const maximumFractionDigits = abs >= 1_000_000 ? 2 : abs >= 100_000 ? 0 : abs >= 1_000 ? 1 : 0;

  if (!displayCurrency) {
    return new Intl.NumberFormat("en-US", {
      notation: "compact",
      maximumFractionDigits,
    }).format(numericAmount);
  }

  return getCompactCurrencyFormatter(rawCurrency, maximumFractionDigits).format(numericAmount);
}

/**
 * Format percentage values with proper formatting
 */
export function formatPercent(value: number | null | undefined) {
  if (value == null) return "-";
  try {
    // Use Intl.NumberFormat for correct percentage formatting (handles x100 and % sign)
    return new Intl.NumberFormat("en-US", {
      style: "percent",
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(value);
  } catch (error) {
    console.error(`Error formatting percent ${value}: ${error}`);
    // Fallback to simple string conversion if formatting fails
    return `${value}%`;
  }
}

export function formatQuantity(quantity: string | number | null | undefined): string {
  if (quantity == null) return "-";
  const numQuantity = parseFloat(String(quantity));
  if (!Number.isFinite(numQuantity)) return "-";
  return new Intl.NumberFormat("en-US", {
    minimumFractionDigits: 0,
    maximumFractionDigits: DECIMAL_PRECISION,
    useGrouping: true,
  }).format(numQuantity);
}
