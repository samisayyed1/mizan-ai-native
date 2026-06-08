/**
 * Single asset class panel card — World-class polish bar (PR-POLISH-1).
 *
 * Three-row layout, no horizontal splits, designed so labels never
 * truncate and the value/delta never collide:
 *
 *   ┌─────────────────────────────────────────────────────┐
 *   │ [icon 32]  Label (semibold 14, never truncated)     │ ← row 1
 *   │                                                     │
 *   │ Value (bold 22 tabular-num)              [chevron]  │ ← row 2
 *   │                                                     │
 *   │ {n} holdings                       ±delta · ±pct%   │ ← row 3
 *   └─────────────────────────────────────────────────────┘
 *
 * Height: ~100px, padding 16x14, radius 12, subtle border.
 *
 * Color discipline (Spec §13):
 * - Value text-foreground when positive or zero
 * - Value text-destructive when negative (so a bank overdraft is RED)
 * - Delta text-success / text-destructive with `+` / `−` sign always
 *   prefixed — the sign is the a11y cue, never color alone
 * - All numbers use `tabular-nums`
 *
 * Reference benchmarks: Linear card density + Stripe Dashboard
 * typographic polish + Robinhood Gold tile rhythm.
 */
import { Icons } from "@mizan/ui/components/ui/icons";
import { formatAmount } from "@mizan/ui/lib/utils";
import { Link } from "react-router-dom";
import type { ComponentType } from "react";

import type { AssetClassPanelDescriptor, AssetClassPanelRollup } from "./taxonomy";

/** Resolve an icon by string key from the Mizan `Icons` registry. */
function resolveIcon(iconKey: string): ComponentType<{ className?: string }> {
  const registry = Icons as unknown as Record<string, ComponentType<{ className?: string }>>;
  return registry[iconKey] ?? registry.Box ?? (() => null);
}

export interface AssetClassPanelCardProps {
  descriptor: AssetClassPanelDescriptor;
  rollup: AssetClassPanelRollup;
  /**
   * Privacy mode hides absolute values per the existing `PrivacyToggle`
   * pattern; deltas and counts remain visible so the user still has
   * directional signal.
   */
  isPrivacyMode?: boolean;
}

export function AssetClassPanelCard({
  descriptor,
  rollup,
  isPrivacyMode = false,
}: AssetClassPanelCardProps) {
  const Icon = resolveIcon(descriptor.iconKey);
  const hasHoldings = rollup.holdingsCount > 0;
  const isValueNegative = rollup.totalValue < 0;
  const isDeltaPositive = rollup.dayChange > 0;
  const isDeltaFlat = rollup.dayChange === 0;
  const isDeltaNegative = rollup.dayChange < 0;
  const deltaSign = isDeltaPositive ? "+" : isDeltaNegative ? "−" : "";
  // Use absolute value + our own sign so we control U+2212 vs hyphen.
  const formattedDelta = formatAmount(
    Math.abs(rollup.dayChange),
    rollup.baseCurrency,
  );
  // Same for the absolute value when negative — strip formatAmount's
  // hyphen-minus and prefix our own U+2212 so the typography matches.
  const formattedValueAbs = formatAmount(
    Math.abs(rollup.totalValue),
    rollup.baseCurrency,
  );
  const valueDisplay = isPrivacyMode
    ? "•••••"
    : hasHoldings
      ? isValueNegative
        ? `−${formattedValueAbs}`
        : formattedValueAbs
      : "—";

  return (
    <Link
      to={descriptor.holdingsHref}
      className="group bg-card hover:bg-muted/40 border-border/60 focus-visible:ring-ring relative flex h-[104px] flex-col justify-between rounded-xl border px-4 py-3.5 transition-[background-color,transform,box-shadow] duration-150 ease-out hover:-translate-y-0.5 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 active:translate-y-0 active:scale-[0.99] motion-reduce:transition-colors motion-reduce:hover:translate-y-0 motion-reduce:hover:shadow-none motion-reduce:active:scale-100"
      aria-label={`Open ${descriptor.label} panel`}
      data-panel-id={descriptor.id}
    >
      {/* Row 1 — icon + label */}
      <div className="flex min-w-0 items-center gap-2.5">
        <span className="bg-muted text-foreground flex h-8 w-8 shrink-0 items-center justify-center rounded-full">
          <Icon className="h-4 w-4" />
        </span>
        <span className="text-foreground min-w-0 flex-1 text-sm font-semibold leading-tight">
          {descriptor.label}
        </span>
      </div>

      {/* Row 2 — value (left) + chevron (right) */}
      <div className="flex items-baseline justify-between gap-2">
        <span
          className={`text-[22px] font-bold leading-none tabular-nums ${
            hasHoldings && isValueNegative
              ? "text-destructive"
              : hasHoldings
                ? "text-foreground"
                : "text-muted-foreground"
          }`}
        >
          {valueDisplay}
        </span>
        <Icons.ChevronRight
          className="text-muted-foreground/70 group-hover:text-foreground h-4 w-4 shrink-0 transition-colors"
          aria-hidden="true"
        />
      </div>

      {/* Row 3 — holdings count (left) + delta (right) */}
      <div className="flex items-baseline justify-between gap-2 text-xs">
        <span className="text-muted-foreground">
          {hasHoldings
            ? `${rollup.holdingsCount.toLocaleString()} ${rollup.holdingsCount === 1 ? "holding" : "holdings"}`
            : "No holdings"}
        </span>
        {hasHoldings ? (
          <span
            className={`text-sm tabular-nums ${
              isDeltaFlat
                ? "text-muted-foreground"
                : isDeltaPositive
                  ? "text-success"
                  : "text-destructive"
            }`}
          >
            <span aria-hidden="true">{deltaSign}</span>
            <span className="sr-only">
              {isDeltaPositive ? "up" : isDeltaNegative ? "down" : "flat"}{" "}
            </span>
            {formattedDelta}
            {rollup.dayChangePct !== null && !isDeltaFlat && (
              <span className="text-muted-foreground ml-1">
                {deltaSign}
                {Math.abs(rollup.dayChangePct).toFixed(2)}%
              </span>
            )}
          </span>
        ) : (
          <span className="text-muted-foreground group-hover:text-foreground inline-flex items-center gap-1 transition-colors">
            <Icons.Plus className="h-3 w-3" aria-hidden="true" />
            <span>Add</span>
          </span>
        )}
      </div>
    </Link>
  );
}
