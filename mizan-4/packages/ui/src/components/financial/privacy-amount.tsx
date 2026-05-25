import { cn, formatAmount } from "../../lib/utils";
import { useBalancePrivacy } from "../../hooks/use-balance-privacy";

interface PrivacyAmountProps extends React.HTMLAttributes<HTMLSpanElement> {
  value: number;
  currency: string;
}

export function PrivacyAmount({ value, currency, className, ...props }: PrivacyAmountProps) {
  const { isBalanceHidden } = useBalancePrivacy();

  // tabular-nums keeps digits equal-width so the amount doesn't shimmy
  // as numbers update — the same fix applied to AmountDisplay.
  return (
    <span className={cn("tabular-nums", className)} {...props}>
      {isBalanceHidden ? "••••" : formatAmount(value, currency)}
    </span>
  );
}
