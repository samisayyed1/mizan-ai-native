/**
 * Single asset class panel card per Spec §13 (Visual / interaction polish).
 *
 * The card is full-width within its grid cell. Composition:
 *   - icon chip (left)
 *   - asset class name (semibold 16px) + holdings count (subdued 12px)
 *   - total value (bold 24px, tabular-nums) + base currency code (subdued 12px)
 *   - 24h delta (14px, sign-prefixed, semantic-token color) + percent
 *   - chevron (right-aligned, subdued)
 *
 * Accessibility:
 * - The sign (+ / −) is always shown, so the direction is readable
 *   without relying on color alone.
 * - Numbers use `tabular-nums` so columns align across the grid.
 * - Dark / light parity is automatic via Flexoki semantic tokens.
 *
 * Tap targets navigate to the dedicated `/panels/{panelId}` page; the
 * route stays stable so deep links keep working.
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
  const isPositive = rollup.dayChange > 0;
  const isFlat = rollup.dayChange === 0;
  const isNegative = rollup.dayChange < 0;
  const sign = isPositive ? "+" : isNegative ? "−" : "";
  // Strip the sign off the absolute formatted value so we control it
  // ourselves — formatAmount may emit "-$1.23" with a hyphen-minus, and
  // we prefer the U+2212 typographic minus for visual consistency.
  const absDayChange = Math.abs(rollup.dayChange);
  const formattedDelta = formatAmount(absDayChange, rollup.baseCurrency);

  return (
    <Link
      to={descriptor.holdingsHref}
      className="group bg-card hover:bg-muted/40 focus-visible:ring-ring relative flex items-center justify-between gap-4 rounded-2xl border p-5 transition-colors focus-visible:outline-none focus-visible:ring-2 md:p-6"
      aria-label={`Open ${descriptor.label} panel`}
      data-panel-id={descriptor.id}
    >
      {/* Left: icon + name + holdings count */}
      <div className="flex min-w-0 items-center gap-3">
        <div className="bg-muted text-foreground flex h-11 w-11 shrink-0 items-center justify-center rounded-xl">
          <Icon className="h-5 w-5" />
        </div>
        <div className="min-w-0">
          <div className="text-foreground truncate text-base font-semibold leading-tight">
            {descriptor.label}
          </div>
          <div className="text-muted-foreground mt-0.5 text-xs">
            {hasHoldings
              ? `${rollup.holdingsCount.toLocaleString()} ${
                  rollup.holdingsCount === 1 ? "holding" : "holdings"
                }`
              : "No holdings"}
          </div>
        </div>
      </div>

      {/* Right: value + delta + chevron */}
      <div className="flex shrink-0 items-center gap-3">
        <div className="text-right">
          {hasHoldings ? (
            <>
              <div className="text-foreground flex items-baseline justify-end gap-1.5">
                <span className="text-xl font-bold tabular-nums leading-tight md:text-2xl">
                  {isPrivacyMode ? "•••••" : formatAmount(rollup.totalValue, rollup.baseCurrency)}
                </span>
              </div>
              <div
                className={`mt-1 text-sm tabular-nums ${
                  isFlat
                    ? "text-muted-foreground"
                    : isPositive
                      ? "text-success"
                      : "text-destructive"
                }`}
              >
                <span aria-hidden="true">{sign}</span>
                <span className="sr-only">
                  {isPositive ? "up" : isNegative ? "down" : "flat"}{" "}
                </span>
                {formattedDelta}
                {rollup.dayChangePct !== null && !isFlat && (
                  <span className="text-muted-foreground ml-1.5">
                    ({sign}
                    {Math.abs(rollup.dayChangePct).toFixed(2)}%)
                  </span>
                )}
              </div>
            </>
          ) : (
            // Empty state — keep the visual rhythm of the populated card
            // by reserving the same right column. The "Add" affordance
            // routes to the panel detail page, where the existing per-
            // panel "Add holding" CTA picks up.
            <div className="flex flex-col items-end gap-0.5">
              <div className="text-muted-foreground text-xl font-semibold tabular-nums leading-tight md:text-2xl">
                —
              </div>
              <div className="text-muted-foreground group-hover:text-foreground inline-flex items-center gap-1 text-xs transition-colors">
                <Icons.Plus className="h-3 w-3" aria-hidden="true" />
                <span>Add</span>
              </div>
            </div>
          )}
        </div>
        <Icons.ChevronRight
          className="text-muted-foreground group-hover:text-foreground h-4 w-4 transition-colors"
          aria-hidden="true"
        />
      </div>
    </Link>
  );
}
