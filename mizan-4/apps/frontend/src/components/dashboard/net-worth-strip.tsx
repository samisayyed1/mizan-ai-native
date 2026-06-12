/**
 * Net Worth Strip — Track A PR-A5 / Goal v3 §V Phase 1 step A5.
 *
 * Pinned-near-top dashboard strip showing the user's net worth headline +
 * a toggleable delta across five preset windows (24h / 7d / 30d / YTD /
 * All) with a sparkline below. The whole strip is a tap target to the
 * Net Worth detail page (`/net-worth`) for the Sankey + liabilities view.
 *
 * §23 step advanced: when the Singapore millionaire opens Mizan during
 * Ramadan, the very first thing on screen is "this is what you're worth
 * and which way it's moving". The strip's delta toggle is the substrate
 * the agent answers from when he says "show me my net worth this week".
 *
 * Data sources (all already wired):
 *   - `useValuationHistory(undefined)` — full history, cached
 *   - `settings.baseCurrency`         — display currency
 *   - `<Sparkline />` from PR-A3      — mini trend chart
 *
 * Delta math is local + cheap: bisect the history by the window's
 * cutoff date (newest entry on or before cutoff), then `last - cutoff`.
 * "All" uses the first entry. Net-contribution drift inside the window
 * is intentionally NOT subtracted here — this is a "what is it worth
 * now vs window-start" delta, matching how every other consumer-finance
 * app frames the headline. TWR-adjusted return lives on the Net Worth
 * page (PR-NW1+) where the chart context makes the methodology obvious.
 */
import { Sparkline } from "@/components/charts";
import type { AccountValuation, CategoryAllocation } from "@/lib/types";
import { Icons } from "@mizan/ui/components/ui/icons";
import { formatAmount } from "@mizan/ui/lib/utils";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { AllocationDonut } from "./allocation-donut";

/** Preset windows offered by the strip toggle. Order = display order. */
export const NET_WORTH_STRIP_WINDOWS = ["24h", "7d", "30d", "YTD", "All"] as const;
export type NetWorthStripWindow = (typeof NET_WORTH_STRIP_WINDOWS)[number];

export interface NetWorthStripProps {
  /** Full valuation history (most-recent-last). May be empty during initial load. */
  history: readonly AccountValuation[] | undefined | null;
  /** Display currency for absolute values + delta. */
  baseCurrency: string;
  /** Privacy mode hides absolute values; deltas + sparkline stay visible. */
  isPrivacyMode?: boolean;
  /** Loading flag — shows a skeleton bar in place of the headline. */
  isLoading?: boolean;
  /** Initial selected window. Defaults to `'30d'` (matches existing dashboard interval default). */
  defaultWindow?: NetWorthStripWindow;
  /** Override the tap-through destination. Defaults to `/net-worth`. */
  toHref?: string;
  /**
   * Asset-class allocation categories from `usePortfolioAllocations`.
   * When provided, an interactive donut renders to the right of the
   * headline (ADR 0018b·v4). Passing `null`/`undefined`/empty hides
   * the donut without breaking the strip.
   */
  allocationCategories?: readonly CategoryAllocation[] | null;
  /** Loading flag for the allocation donut specifically. */
  isAllocationLoading?: boolean;
}

/**
 * Cutoff timestamp for each window, computed against `nowMs`.
 * "All" returns `null` — caller compares against history[0].
 */
function cutoffMsFor(window: NetWorthStripWindow, nowMs: number): number | null {
  const ms = (days: number) => nowMs - days * 24 * 60 * 60 * 1000;
  switch (window) {
    case "24h":
      return ms(1);
    case "7d":
      return ms(7);
    case "30d":
      return ms(30);
    case "YTD": {
      const d = new Date(nowMs);
      // Use UTC year-boundary so the cutoff is stable regardless of the
      // user's local timezone — valuation dates are stored as UTC dates.
      return Date.UTC(d.getUTCFullYear(), 0, 1);
    }
    case "All":
      return null;
  }
}

/**
 * Bisect history to find the newest entry on or before `cutoffMs`.
 * Returns `null` when no entry qualifies (window starts before history).
 *
 * History is sorted ascending by `valuationDate` (the existing
 * `getHistoricalValuations` adapter contract). We linear-scan from the
 * end because the typical window-target is recent.
 */
function findEntryAtOrBefore(
  history: readonly AccountValuation[],
  cutoffMs: number,
): AccountValuation | null {
  for (let i = history.length - 1; i >= 0; i--) {
    const entry = history[i];
    if (!entry) continue;
    const entryMs = Date.parse(entry.valuationDate);
    if (Number.isNaN(entryMs)) continue;
    if (entryMs <= cutoffMs) return entry;
  }
  return null;
}

export interface NetWorthDelta {
  /** Absolute change in base currency (positive = up). */
  amount: number;
  /** Percent change. `null` when prior value was zero (avoids div-by-zero). */
  pct: number | null;
  /** When true the strip should suppress the delta chip (no comparable prior). */
  noBaseline: boolean;
}

/**
 * Compute the delta for a given window. Exported for direct testing —
 * the strip's UI rendering composes this with the React tree.
 *
 * @param history must be ascending by `valuationDate` (most-recent-last)
 * @param window window preset
 * @param nowMs timestamp baseline — caller-supplied for test determinism
 */
export function computeWindowDelta(
  history: readonly AccountValuation[],
  window: NetWorthStripWindow,
  nowMs: number,
): NetWorthDelta {
  if (history.length === 0) {
    return { amount: 0, pct: null, noBaseline: true };
  }
  const last = history[history.length - 1];
  if (!last) return { amount: 0, pct: null, noBaseline: true };

  const cutoffMs = cutoffMsFor(window, nowMs);

  // "All" → first history entry is the baseline. For all other windows
  // we want the most recent entry on-or-before the cutoff (handles
  // sparse history — e.g. weekend gaps in daily snapshots).
  const baseline =
    cutoffMs === null
      ? history[0]
      : findEntryAtOrBefore(history, cutoffMs);

  if (!baseline) {
    // Window starts before our earliest data point — no comparable baseline.
    return { amount: 0, pct: null, noBaseline: true };
  }
  // If the matched baseline IS the last entry (e.g. asking 24h with only
  // today's snapshot), there's no movement to show.
  if (baseline.valuationDate === last.valuationDate) {
    return { amount: 0, pct: null, noBaseline: true };
  }

  const lastValue = Number(last.totalValue);
  const baseValue = Number(baseline.totalValue);
  const amount = lastValue - baseValue;
  const pct = baseValue !== 0 ? (amount / baseValue) * 100 : null;
  return { amount, pct, noBaseline: false };
}

/**
 * Build sparkline data from the history slice that contributes to the
 * selected window. For "All" we sample the whole history; for shorter
 * windows we only render entries on or after the window cutoff (so the
 * sparkline visually reflects the delta being shown).
 *
 * Returns at most ~120 points (downsampled by stride) so the sparkline
 * stays paint-cheap on the dashboard.
 */
function buildSparklineSeries(
  history: readonly AccountValuation[],
  window: NetWorthStripWindow,
  nowMs: number,
): { x: number; y: number }[] {
  if (history.length === 0) return [];
  const cutoffMs = cutoffMsFor(window, nowMs);
  const slice =
    cutoffMs === null
      ? history
      : history.filter((e) => {
          const ms = Date.parse(e.valuationDate);
          return !Number.isNaN(ms) && ms >= cutoffMs;
        });
  if (slice.length === 0) {
    // Always include at least the last point so the sparkline isn't blank.
    const last = history[history.length - 1];
    return last ? [{ x: Date.parse(last.valuationDate), y: Number(last.totalValue) }] : [];
  }
  const MAX_POINTS = 120;
  const stride = Math.max(1, Math.ceil(slice.length / MAX_POINTS));
  const out: { x: number; y: number }[] = [];
  for (let i = 0; i < slice.length; i += stride) {
    const e = slice[i];
    if (!e) continue;
    const ms = Date.parse(e.valuationDate);
    if (Number.isNaN(ms)) continue;
    out.push({ x: ms, y: Number(e.totalValue) });
  }
  // Always include the final point so the right edge of the sparkline
  // matches the headline total.
  const last = slice[slice.length - 1];
  if (last && out[out.length - 1]?.x !== Date.parse(last.valuationDate)) {
    out.push({ x: Date.parse(last.valuationDate), y: Number(last.totalValue) });
  }
  return out;
}

/**
 * Net worth strip — headline + window toggle + delta chip + sparkline.
 *
 * The component reads `Date.now()` once per render via the `nowMs` prop
 * fallback so tests can pass a deterministic baseline.
 */
export function NetWorthStrip({
  history,
  baseCurrency,
  isPrivacyMode = false,
  isLoading = false,
  defaultWindow = "30d",
  toHref = "/net-worth",
  allocationCategories,
  isAllocationLoading = false,
}: NetWorthStripProps) {
  const [selectedWindow, setSelectedWindow] = useState<NetWorthStripWindow>(defaultWindow);
  const safeHistory = useMemo(() => history ?? [], [history]);

  // Compute Date.now() once per render; passing it through pure helpers
  // keeps the math fully testable.
  const nowMs = Date.now();

  const headline = useMemo(() => {
    if (safeHistory.length === 0) return 0;
    const last = safeHistory[safeHistory.length - 1];
    return last ? Number(last.totalValue) : 0;
  }, [safeHistory]);

  const delta = useMemo(
    () => computeWindowDelta(safeHistory, selectedWindow, nowMs),
    [safeHistory, selectedWindow, nowMs],
  );

  const sparklineData = useMemo(
    () => buildSparklineSeries(safeHistory, selectedWindow, nowMs),
    [safeHistory, selectedWindow, nowMs],
  );

  const isPositive = delta.amount >= 0;
  const deltaColor =
    delta.noBaseline || delta.amount === 0
      ? "text-muted-foreground"
      : isPositive
        ? "text-success"
        : "text-destructive";

  return (
    <section
      aria-label="Net worth"
      className="bg-card relative flex flex-col gap-3 rounded-2xl border p-4"
    >
      {/* Top row: headline (link) + allocation donut + chevron. The
          allocation donut sits OUTSIDE the headline link so its arcs
          stay independently interactive (hover/focus per segment)
          without hijacking the strip's tap-to-detail affordance. */}
      <div className="flex items-start justify-between gap-3">
        <Link
          to={toHref}
          className="focus-visible:ring-ring -m-1 flex-1 rounded-lg p-1 focus-visible:outline-none focus-visible:ring-2"
          aria-label="Open Net Worth detail page"
          data-testid="net-worth-strip-link"
        >
          <div className="text-muted-foreground text-xs uppercase tracking-wider">Net Worth</div>
          <div className="text-foreground mt-1 text-3xl font-semibold tabular-nums">
            {isLoading ? (
              <span className="bg-muted inline-block h-8 w-40 animate-pulse rounded" />
            ) : isPrivacyMode ? (
              "•••••"
            ) : (
              formatAmount(headline, baseCurrency)
            )}
          </div>
          <div className={`mt-1 text-sm tabular-nums ${deltaColor}`} data-testid="net-worth-delta">
            {delta.noBaseline ? (
              <span>— no prior data in this window</span>
            ) : (
              <>
                {isPositive ? "▲" : "▼"} {isPrivacyMode ? "•••" : formatAmount(Math.abs(delta.amount), baseCurrency)}
                {delta.pct !== null && (
                  <span className="text-muted-foreground ml-1">
                    ({isPositive ? "+" : ""}
                    {delta.pct.toFixed(2)}%)
                  </span>
                )}
                <span className="text-muted-foreground ml-2 text-xs">past {selectedWindow}</span>
              </>
            )}
          </div>
        </Link>
        {/* Allocation donut — sits to the right of the headline. Hidden
            below the `sm:` breakpoint so a phone gets a clean stacked
            headline + sparkline only; on desktop it's the visual anchor
            that turns the strip into a real net-worth dashboard. */}
        {allocationCategories && allocationCategories.length > 0 ? (
          <div className="hidden sm:block">
            <AllocationDonut
              categories={allocationCategories}
              baseCurrency={baseCurrency}
              isLoading={isAllocationLoading}
              isPrivacyMode={isPrivacyMode}
              size={132}
            />
          </div>
        ) : null}
        <Icons.ChevronRight
          className="text-muted-foreground mt-1 h-5 w-5 shrink-0"
          aria-hidden="true"
        />
      </div>

      {/* Window toggle */}
      <div
        className="flex flex-wrap gap-1"
        role="group"
        aria-label="Net worth time window"
        data-testid="net-worth-window-toggle"
      >
        {NET_WORTH_STRIP_WINDOWS.map((w) => {
          const selected = w === selectedWindow;
          return (
            <button
              key={w}
              type="button"
              onClick={() => setSelectedWindow(w)}
              aria-pressed={selected}
              className={
                "rounded-full px-3 py-1 text-xs font-medium transition-colors " +
                (selected
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:bg-muted/80")
              }
            >
              {w}
            </button>
          );
        })}
      </div>

      {/* Sparkline */}
      {sparklineData.length >= 2 && (
        <div className="h-12" aria-hidden="true">
          <Sparkline
            data={sparklineData}
            ariaLabel={`Net worth trend over the past ${selectedWindow}`}
            palette={delta.amount >= 0 ? "mono" : "divergent"}
            height={48}
          />
        </div>
      )}
    </section>
  );
}
