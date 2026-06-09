/**
 * Single asset class panel card — Polish v3 density pass (PR-DENSITY-1).
 *
 * Compressed two-row layout that lets 12 tiles fit on a single screen
 * view at professional fintech density (Linear / Stripe Dashboard /
 * Robinhood Gold tier):
 *
 *   ┌───────────────────────────────────────────────┐
 *   │ [icon 20]  Label (semibold 13)    Value (14)  │  row 1 (24px)
 *   │                                               │
 *   │ {n} holdings              ±delta · ±pct% (12) │  row 2 (16px)
 *   └───────────────────────────────────────────────┘
 *
 * Height fixed at 72px (was 104px in PR-POLISH-1), padding 12×12,
 * border-radius 10px, gap between cards 8px (handled by grid).
 *
 * Color discipline (Spec §13):
 *   - Value text-foreground when positive or zero
 *   - Value text-destructive when negative (so a bank overdraft is RED)
 *   - Delta text-success / text-destructive with `+` / `−` sign always
 *     prefixed — the sign is the a11y cue, never color alone
 *   - All numbers use `tabular-nums`
 *
 * Empty state collapses the value to em-dash and demotes the delta
 * row to a `+ Add` affordance that brightens on hover.
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

/**
 * Visual hierarchy tier — PR-DENSITY-3.
 *
 * Three tiers across the 12 tile grid so the eye finds the funded
 * positions first and the empties recede:
 *
 *   primary    — |total| ≥ $10k → full opacity, full color contrast
 *   secondary  — total > 0 but |total| < $10k → full opacity, value 0.85
 *   empty      — no holdings → 0.6 opacity card, em-dash, "+ Add" CTA
 *
 * The $10k threshold is intentionally crude: it captures the §23
 * reference user's "empty / starter / serious" distinction without
 * needing a per-user threshold (which would require user prefs
 * plumbing for a purely visual signal).
 */
type TileTier = "primary" | "secondary" | "empty";

const SECONDARY_THRESHOLD_BASE = 10_000;

function classifyTier(rollup: AssetClassPanelRollup): TileTier {
  if (rollup.holdingsCount === 0) return "empty";
  if (Math.abs(rollup.totalValue) >= SECONDARY_THRESHOLD_BASE) return "primary";
  return "secondary";
}

export function AssetClassPanelCard({
  descriptor,
  rollup,
  isPrivacyMode = false,
}: AssetClassPanelCardProps) {
  const Icon = resolveIcon(descriptor.iconKey);
  const tier = classifyTier(rollup);
  const hasHoldings = rollup.holdingsCount > 0;
  const isValueNegative = rollup.totalValue < 0;
  const isDeltaPositive = rollup.dayChange > 0;
  const isDeltaFlat = rollup.dayChange === 0;
  const isDeltaNegative = rollup.dayChange < 0;
  const deltaSign = isDeltaPositive ? "+" : isDeltaNegative ? "−" : "";

  const formattedDelta = formatAmount(
    Math.abs(rollup.dayChange),
    rollup.baseCurrency,
  );
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
      className={`group bg-card hover:bg-muted/40 hover:border-border border-border/60 focus-visible:ring-ring relative flex h-[72px] flex-col justify-between rounded-[10px] border px-3 py-3 transition-[background-color,border-color,transform,box-shadow,opacity] duration-150 ease-out hover:-translate-y-0.5 hover:shadow-md hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 active:translate-y-0 active:scale-[0.99] motion-reduce:transition-colors motion-reduce:hover:translate-y-0 motion-reduce:hover:shadow-none motion-reduce:active:scale-100 ${
        tier === "empty" ? "opacity-60" : "opacity-100"
      }`}
      aria-label={`Open ${descriptor.label} panel`}
      data-panel-id={descriptor.id}
      data-tier={tier}
    >
      {/* Row 1 — icon + label (left) / value (right) */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Icon className="text-muted-foreground h-4 w-4 shrink-0" />
          <span className="text-foreground truncate text-[13px] font-semibold leading-tight">
            {descriptor.label}
          </span>
        </div>
        <span
          className={`shrink-0 text-sm font-bold leading-none tabular-nums ${
            !hasHoldings
              ? "text-muted-foreground"
              : isValueNegative
                ? "text-destructive"
                : tier === "primary"
                  ? "text-foreground"
                  : "text-foreground/85"
          }`}
        >
          {valueDisplay}
        </span>
      </div>

      {/* Row 2 — holdings count (left) / delta or +Add (right) */}
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-muted-foreground text-[11px] leading-none">
          {hasHoldings
            ? `${rollup.holdingsCount.toLocaleString()} ${rollup.holdingsCount === 1 ? "holding" : "holdings"}`
            : "No holdings"}
        </span>
        {hasHoldings ? (
          <span
            className={`text-xs leading-none tabular-nums ${
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
          <span className="text-muted-foreground/70 group-hover:text-primary inline-flex items-center gap-0.5 text-xs leading-none transition-colors">
            <Icons.Plus className="h-3 w-3" aria-hidden="true" />
            <span>Add</span>
          </span>
        )}
      </div>
    </Link>
  );
}
