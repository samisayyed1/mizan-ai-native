import { cn, formatAmount, formatPrice } from "../../lib/utils";

interface AmountDisplayProps {
  // Accepts string too because money values cross IPC as
  // Decimal-as-string in some adapters; coercing to number at the
  // boundary would silently drop sub-cent precision before display.
  value: number | string | null | undefined;
  currency: string;
  isHidden?: boolean;
  displayCurrency?: boolean;
  colorFormat?: boolean;
  invertColor?: boolean;
  className?: string;
  /**
   * Formatting precision:
   *   - `"amount"` (default) → 2 decimal places, suitable for totals,
   *     account balances, gain/loss values.
   *   - `"price"` → up to 8 decimal places for sub-dollar values (crypto
   *     / penny tokens / micro-priced instruments), 2 decimals otherwise.
   *
   * Using `"price"` for a per-unit price prevents the dashboard from
   * silently rendering "$0.00" for a real holding worth fractions of
   * a cent per unit.
   */
  precision?: "amount" | "price";
}

export function AmountDisplay({
  value,
  currency = "USD",
  isHidden,
  displayCurrency = true,
  colorFormat,
  invertColor = false,
  className,
  precision = "amount",
}: AmountDisplayProps) {
  const formattedAmount =
    precision === "price"
      ? formatPrice(value, currency, displayCurrency)
      : formatAmount(value, currency, displayCurrency);
  // Skip color tinting when the underlying value is missing or non-
  // finite. Previously `Number(null) = 0` painted the "-" dash
  // success-green and `Number(undefined) = NaN` painted it destructive-
  // red — both misleading colour-cues on a no-data display.
  const numericValue = typeof value === "number" ? value : Number(value);
  const hasNumericValue = value != null && Number.isFinite(numericValue);
  const positive = invertColor ? "text-destructive" : "text-success";
  const negative = invertColor ? "text-success" : "text-destructive";
  const colorClass = colorFormat && hasNumericValue ? (numericValue >= 0 ? positive : negative) : "";

  return <span className={cn(colorClass, className)}>{isHidden ? "••••" : formattedAmount}</span>;
}
