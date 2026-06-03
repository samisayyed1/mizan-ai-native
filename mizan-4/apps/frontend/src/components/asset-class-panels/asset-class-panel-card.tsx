/**
 * Single asset class panel card — Track A PR-A4.
 *
 * Skeleton tile per Goal §V.A4 + ADR 0021: header (icon + label) +
 * total value + 24h delta (gain pill) + holdings-count chip + chevron.
 * Sparkline + 30d delta land in PR-A6 once the price-history feed is
 * wired into rollups (kept out per Goal §III step 5 "minimum viable").
 *
 * Taps navigate to the existing holdings page filtered to this panel's
 * predicate via `holdingsHref` from [`AssetClassPanelDescriptor`]; the
 * dedicated Track B panel surfaces (PR-B1..B7) replace these routes
 * panel-by-panel as they ship.
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
  const isPositive = rollup.dayChange >= 0;
  const hasHoldings = rollup.holdingsCount > 0;

  return (
    <Link
      to={descriptor.holdingsHref}
      className="group bg-card hover:bg-muted/40 focus-visible:ring-ring relative flex items-center justify-between rounded-2xl border p-4 transition-colors focus-visible:outline-none focus-visible:ring-2"
      aria-label={`Open ${descriptor.label} panel`}
      data-panel-id={descriptor.id}
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="bg-muted text-foreground flex h-10 w-10 shrink-0 items-center justify-center rounded-full">
          <Icon className="h-5 w-5" />
        </div>
        <div className="min-w-0">
          <div className="text-foreground truncate text-sm font-medium">{descriptor.label}</div>
          <div className="text-muted-foreground text-xs">
            {hasHoldings
              ? `${rollup.holdingsCount.toLocaleString()} ${
                  rollup.holdingsCount === 1 ? "holding" : "holdings"
                }`
              : "No holdings"}
          </div>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <div className="text-right">
          <div className="text-foreground text-sm font-semibold tabular-nums">
            {isPrivacyMode ? "•••••" : formatAmount(rollup.totalValue, rollup.baseCurrency)}
          </div>
          {hasHoldings && (
            <div
              className={`text-xs tabular-nums ${
                rollup.dayChange === 0
                  ? "text-muted-foreground"
                  : isPositive
                    ? "text-emerald-600 dark:text-emerald-400"
                    : "text-rose-600 dark:text-rose-400"
              }`}
            >
              {isPositive ? "+" : ""}
              {formatAmount(rollup.dayChange, rollup.baseCurrency)}
              {rollup.dayChangePct !== null && (
                <span className="text-muted-foreground ml-1">
                  ({isPositive ? "+" : ""}
                  {rollup.dayChangePct.toFixed(2)}%)
                </span>
              )}
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
