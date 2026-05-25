import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { useBalancePrivacy } from "@/hooks/use-balance-privacy";
import { useMemo } from "react";

// NOTE: We used to wrap the headline number in `@number-flow/react` for a
// digit-roll animation, but its custom element exposes the raw 0–9 reel in
// the Tauri 2 webview (custom-element registration races / CSP interplay)
// — producing strings like "$01234567890123456789..." instead of the real
// value. Until that's diagnosed upstream, render plain formatted text.

const isValidCurrencyCode = (code: string) => /^[A-Za-z]{3}$/.test(code);

interface BalanceProps {
  targetValue: number;
  currency: string;
  displayCurrency?: boolean;
  displayDecimal?: boolean;
  isLoading?: boolean;
}

const Balance: React.FC<BalanceProps> = ({
  targetValue,
  currency = "USD",
  displayCurrency = false,
  displayDecimal = true,
  isLoading = false,
}) => {
  const { isBalanceHidden } = useBalancePrivacy();
  const validCurrency = isValidCurrencyCode(currency);

  const currencySymbol = useMemo(() => {
    if (!validCurrency) return currency;
    try {
      const formatter = new Intl.NumberFormat(undefined, {
        style: "currency",
        currency,
        currencyDisplay: "narrowSymbol",
        minimumFractionDigits: 0,
        maximumFractionDigits: 0,
      });
      const parts = formatter.formatToParts(0);
      return parts.find((part) => part.type === "currency")?.value ?? currency;
    } catch {
      return currency;
    }
  }, [currency, validCurrency]);

  const formattedValue = useMemo(() => {
    const useCurrencyStyle = displayCurrency && validCurrency;
    try {
      const formatter = new Intl.NumberFormat(undefined, {
        ...(useCurrencyStyle ? { currency, currencyDisplay: "narrowSymbol" } : {}),
        style: useCurrencyStyle ? "currency" : "decimal",
        minimumFractionDigits: displayDecimal ? 2 : 0,
        maximumFractionDigits: displayDecimal ? 2 : 0,
      });
      return formatter.format(targetValue);
    } catch {
      return targetValue.toFixed(displayDecimal ? 2 : 0);
    }
  }, [currency, validCurrency, displayCurrency, displayDecimal, targetValue]);

  if (isLoading) {
    return <Skeleton className="h-9 w-48" />;
  }

  return (
    <h1 className="font-heading text-3xl font-bold tracking-tight" data-testid="portfolio-balance">
      {isBalanceHidden ? (
        <span>
          {displayCurrency ? currencySymbol : ""}
          •••••••
        </span>
      ) : (
        <span data-testid="portfolio-balance-value">{formattedValue}</span>
      )}
    </h1>
  );
};

export default Balance;
