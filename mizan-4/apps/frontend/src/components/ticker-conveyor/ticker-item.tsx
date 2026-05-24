import { formatPercent } from "@mizan/ui";

import { cn } from "@/lib/utils";

export interface TickerDatum {
  key: string;
  label: string;
  /** Per-unit price in `currency`, or null when unresolved. */
  price: number | null;
  /** Day change as a fraction (0.0123 = +1.23%), or null. */
  changePct: number | null;
  currency?: string;
}

function formatPrice(price: number | null, currency?: string): string {
  if (price == null) return "—";
  const maximumFractionDigits = Math.abs(price) < 10 ? 4 : 2;
  try {
    return new Intl.NumberFormat(undefined, {
      style: currency ? "currency" : "decimal",
      currency: currency || undefined,
      maximumFractionDigits,
    }).format(price);
  } catch {
    return price.toFixed(2);
  }
}

export function TickerItem({ datum }: { datum: TickerDatum }) {
  const { label, price, changePct, currency } = datum;
  const dir = changePct == null ? 0 : Math.sign(changePct);
  return (
    <div className="flex shrink-0 items-baseline gap-2 text-sm tabular-nums">
      <span className="font-semibold">{label}</span>
      <span className="text-foreground/80">{formatPrice(price, currency)}</span>
      {changePct != null && (
        <span
          className={cn(
            dir > 0 && "text-success",
            dir < 0 && "text-destructive",
            dir === 0 && "text-muted-foreground",
          )}
        >
          {formatPercent(changePct)}
        </span>
      )}
    </div>
  );
}
