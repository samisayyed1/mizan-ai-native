import * as React from "react";
import { useBalancePrivacy } from "../../hooks/use-balance-privacy";
import { cn } from "../../lib/utils";

const isValidCurrencyCode = (code: string) => /^[A-Za-z]{3}$/.test(code);

interface GainAmountProps extends React.HTMLAttributes<HTMLDivElement> {
  value: number;
  displayCurrency?: boolean;
  currency: string;
  displayDecimal?: boolean;
  showSign?: boolean;
}

/**
 * Gain/loss amount rendered as a plain formatted number.
 *
 * We used to wrap this in `@number-flow/react` for a digit-roll
 * animation, but its custom element exposes the raw 0–9 reel in the
 * Tauri 2 webview, producing garbage like "$01234567890123456789..."
 * instead of the real value. Until that's diagnosed upstream we render
 * plain `Intl.NumberFormat` output.
 */
export function GainAmount({
  value,
  currency,
  displayCurrency = true,
  className,
  displayDecimal = true,
  showSign = true,
  ...props
}: GainAmountProps) {
  const { isBalanceHidden } = useBalancePrivacy();
  const validCurrency = isValidCurrencyCode(currency);
  const useCurrencyStyle = displayCurrency && validCurrency;

  const formatOptions: Intl.NumberFormatOptions = {
    ...(useCurrencyStyle ? { currency, currencyDisplay: "narrowSymbol" as const } : {}),
    style: useCurrencyStyle ? "currency" : "decimal",
    minimumFractionDigits: displayDecimal ? 2 : 0,
    maximumFractionDigits: displayDecimal ? 2 : 0,
  };

  const formatted = (() => {
    try {
      return new Intl.NumberFormat(
        typeof navigator !== "undefined" ? navigator.language : "en-US",
        formatOptions,
      ).format(Math.abs(value));
    } catch {
      return Math.abs(value).toFixed(displayDecimal ? 2 : 0);
    }
  })();

  return (
    <div className={cn("flex flex-col items-end text-right text-sm tabular-nums", className)} {...props}>
      <div
        className={cn(
          "flex items-center",
          value > 0 ? "text-success" : value < 0 ? "text-destructive" : "text-foreground",
        )}
      >
        {isBalanceHidden ? (
          <span>••••</span>
        ) : (
          <span>
            {showSign && (value > 0 ? "+" : value < 0 ? "-" : null)}
            {formatted}
          </span>
        )}
      </div>
    </div>
  );
}
