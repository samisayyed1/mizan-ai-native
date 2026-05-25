/**
 * Portfolio detail page — asset-classes view.
 *
 * Replaces the legacy "flat list of holdings" rendering inside a
 * portfolio with Feroz's two-level shape:
 *
 *   default view  → portfolio summary + active asset class grid +
 *                   "Add another asset class" rail
 *   ?class=STOCKS → drill-down list of just that class's holdings
 *
 * The component is intentionally self-contained: it fetches its own
 * holdings (cache shared with AccountHoldings via QueryKeys.HOLDINGS),
 * derives groupings via the pure classifier in `lib/asset-classes.ts`,
 * and exposes an `onAddForClass(cls)` prop for the drill-down/empty-state
 * CTAs (which the parent page routes to the correct per-class editor).
 *
 * Reference: .claude/product-notes/feroz-meeting-2026-05-17.md
 * decisions #5, #9, #10, #13.
 */

import { getHoldings } from "@/adapters";
import { useAccounts } from "@/hooks/use-accounts";
import {
  AssetClass,
  ASSET_CLASS_ICON_NAMES,
  ASSET_CLASS_LABELS,
  assetClassColor,
  groupHoldingsByAssetClass,
  parseAssetClassParam,
  partitionBuckets,
  type AssetClassBucket,
} from "@/lib/asset-classes";
import { QueryKeys } from "@/lib/query-keys";
import type { Holding } from "@/lib/types";
import { Badge } from "@mizan/ui/components/ui/badge";
import { Button } from "@mizan/ui/components/ui/button";
import { Card, CardContent } from "@mizan/ui/components/ui/card";
import { Icons, type IconName } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { formatCompactAmount } from "@mizan/ui";
import { useQuery } from "@tanstack/react-query";
import { differenceInCalendarDays, format, formatDistanceToNowStrict, isValid } from "date-fns";
import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { AddHoldingMenu } from "./add-holding-menu";
import { AssetClassHistoryChart } from "./asset-class-history-chart";

interface AssetClassesViewProps {
  accountId: string;
  onAddForClass?: (cls: AssetClass) => void;
}

export function AssetClassesView({ accountId, onAddForClass }: AssetClassesViewProps) {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedClass = parseAssetClassParam(searchParams.get("class"));

  const { data: holdings, isLoading } = useQuery<Holding[], Error>({
    queryKey: [QueryKeys.HOLDINGS, accountId],
    queryFn: () => getHoldings(accountId),
  });

  const { accounts } = useAccounts();
  const account = useMemo(
    () => accounts?.find((a) => a.id === accountId) ?? null,
    [accounts, accountId],
  );
  const portfolioCurrency = account?.currency ?? "USD";

  const buckets = useMemo(() => groupHoldingsByAssetClass(holdings ?? []), [holdings]);
  const { active, empty } = useMemo(() => partitionBuckets(buckets), [buckets]);

  // Active buckets sorted by value desc — investors care about
  // where the money actually is before they care about anything else.
  const activeSorted = useMemo(
    () => [...active].sort((a, b) => b.totalValue - a.totalValue),
    [active],
  );

  const portfolioTotal = useMemo(
    () => buckets.reduce((acc, b) => acc + b.totalValue, 0),
    [buckets],
  );
  const totalHoldings = useMemo(() => buckets.reduce((acc, b) => acc + b.count, 0), [buckets]);

  const handleSelectClass = (cls: AssetClass) => {
    const next = new URLSearchParams(searchParams);
    next.set("class", cls);
    setSearchParams(next, { replace: false });
  };

  const handleBackToClasses = () => {
    const next = new URLSearchParams(searchParams);
    next.delete("class");
    setSearchParams(next, { replace: false });
  };

  if (isLoading) {
    return <AssetClassesSkeleton />;
  }

  if (selectedClass) {
    const bucket = buckets.find((b) => b.cls === selectedClass);
    return (
      <AssetClassDrilldown
        cls={selectedClass}
        bucket={bucket}
        portfolioCurrency={portfolioCurrency}
        accountId={accountId}
        onBack={handleBackToClasses}
        onAddForClass={onAddForClass}
      />
    );
  }

  return (
    <div className="space-y-6">
      <PortfolioSummaryLine
        portfolioTotal={portfolioTotal}
        portfolioCurrency={portfolioCurrency}
        totalHoldings={totalHoldings}
        activeClassCount={activeSorted.length}
      />

      {activeSorted.length > 0 && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {activeSorted.map((bucket) => (
            <AssetClassCard
              key={bucket.cls}
              bucket={bucket}
              portfolioCurrency={portfolioCurrency}
              onClick={() => handleSelectClass(bucket.cls)}
            />
          ))}
        </div>
      )}

      {empty.length > 0 && (
        <AddAnotherAssetClassRail buckets={empty} onSelect={handleSelectClass} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Portfolio summary line — orientation header above the grid
// ---------------------------------------------------------------------------

interface PortfolioSummaryLineProps {
  portfolioTotal: number;
  portfolioCurrency: string;
  totalHoldings: number;
  activeClassCount: number;
}

function PortfolioSummaryLine({
  portfolioTotal,
  portfolioCurrency,
  totalHoldings,
  activeClassCount,
}: PortfolioSummaryLineProps) {
  return (
    <div className="flex items-baseline justify-between gap-2 px-1">
      <h2 className="text-base font-semibold tracking-tight">Asset Classes</h2>
      <p className="text-muted-foreground text-xs sm:text-sm">
        {totalHoldings === 0 ? (
          <span>No holdings yet &middot; pick a class to start</span>
        ) : (
          <>
            <span className="text-foreground font-medium tabular-nums">
              {formatCompactAmount(portfolioTotal, portfolioCurrency)}
            </span>{" "}
            &middot; {totalHoldings} {totalHoldings === 1 ? "holding" : "holdings"} across{" "}
            {activeClassCount} {activeClassCount === 1 ? "class" : "classes"}
          </>
        )}
      </p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Active asset class card — full-fat tile with value, gain, weight bar
// ---------------------------------------------------------------------------

interface AssetClassCardProps {
  bucket: AssetClassBucket;
  portfolioCurrency: string;
  onClick: () => void;
}

function AssetClassCard({ bucket, portfolioCurrency, onClick }: AssetClassCardProps) {
  const labels = ASSET_CLASS_LABELS[bucket.cls];
  const Icon = Icons[ASSET_CLASS_ICON_NAMES[bucket.cls] as IconName];

  return (
    <Card
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      aria-label={`Open ${labels.plural}`}
      className={[
        "group cursor-pointer transition-colors",
        "hover:border-primary/40 hover:bg-accent/30",
        "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2",
      ].join(" ")}
    >
      {/* Flex column + h-full so cards in a grid row stay equal height
          and the weight bar always pins to the bottom edge, regardless
          of how tall the header content is (gain badge present/absent,
          a future two-line label, etc). Keeps the bottom rule of every
          card in a row perfectly aligned. */}
      <CardContent className="flex h-full flex-col gap-3 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <div className="bg-primary/10 text-primary flex h-10 w-10 shrink-0 items-center justify-center rounded-lg">
              {Icon ? <Icon className="h-5 w-5" /> : null}
            </div>
            <div className="min-w-0">
              <p className="truncate text-sm font-medium leading-tight">{labels.plural}</p>
              <p className="text-muted-foreground mt-0.5 text-xs">
                {bucket.count} {bucket.count === 1 ? labels.singular : labels.plural}
              </p>
            </div>
          </div>
          <div className="shrink-0 text-right">
            <p className="text-sm font-semibold tabular-nums">
              {formatCompactAmount(bucket.totalValue, portfolioCurrency)}
            </p>
            <GainBadge value={bucket.totalGain} currency={portfolioCurrency} />
          </div>
        </div>

        <div className="mt-auto">
          <WeightBar percent={bucket.weightPercent} color={assetClassColor(bucket.cls)} />
        </div>
      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// "Add another asset class" rail — empty buckets as compact chips
// ---------------------------------------------------------------------------

interface AddAnotherAssetClassRailProps {
  buckets: readonly AssetClassBucket[];
  onSelect: (cls: AssetClass) => void;
}

function AddAnotherAssetClassRail({ buckets, onSelect }: AddAnotherAssetClassRailProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3">
        <h3 className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
          Add another asset class
        </h3>
        <div className="bg-border h-px flex-1" />
      </div>
      <div className="flex flex-wrap gap-2">
        {buckets.map((bucket) => (
          <EmptyAssetClassChip
            key={bucket.cls}
            cls={bucket.cls}
            onClick={() => onSelect(bucket.cls)}
          />
        ))}
      </div>
    </div>
  );
}

interface EmptyAssetClassChipProps {
  cls: AssetClass;
  onClick: () => void;
}

function EmptyAssetClassChip({ cls, onClick }: EmptyAssetClassChipProps) {
  const labels = ASSET_CLASS_LABELS[cls];
  const Icon = Icons[ASSET_CLASS_ICON_NAMES[cls] as IconName];

  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={`Add ${labels.singular}`}
      className={[
        "border-input bg-background hover:border-primary/40 hover:bg-accent/30",
        "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2",
        "inline-flex items-center gap-2 rounded-full border border-dashed px-3 py-1.5",
        "text-muted-foreground hover:text-foreground text-xs transition-colors",
      ].join(" ")}
    >
      <Icons.Plus className="h-3.5 w-3.5" />
      {Icon ? <Icon className="h-3.5 w-3.5" /> : null}
      <span>{labels.plural}</span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Drill-down view — holdings inside a single class
// ---------------------------------------------------------------------------

interface AssetClassDrilldownProps {
  cls: AssetClass;
  bucket: AssetClassBucket | undefined;
  portfolioCurrency: string;
  accountId: string;
  onBack: () => void;
  onAddForClass?: (cls: AssetClass) => void;
}

function AssetClassDrilldown({
  cls,
  bucket,
  portfolioCurrency,
  accountId,
  onBack,
  onAddForClass,
}: AssetClassDrilldownProps) {
  const labels = ASSET_CLASS_LABELS[cls];
  const Icon = Icons[ASSET_CLASS_ICON_NAMES[cls] as IconName];
  const totalValue = bucket?.totalValue ?? 0;
  const totalGain = bucket?.totalGain ?? null;
  const weightPercent = bucket?.weightPercent ?? 0;

  // Largest-first inside the drill-down. The grouping helper preserves
  // input order so the caller controls sort — here we want the biggest
  // positions at the top so the user can see what matters first.
  // Pull `bucket?.holdings` *inside* the memo to keep deps stable —
  // the ternary fallback otherwise produces a new [] reference on every
  // render and the memo recomputes every time.
  const sortedHoldings = useMemo(() => {
    const list = bucket?.holdings ?? [];
    return [...list].sort((a, b) => (b.marketValue?.base ?? 0) - (a.marketValue?.base ?? 0));
  }, [bucket]);

  const classTotalForWeights = totalValue > 0 ? totalValue : 1;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-2">
        <Button variant="ghost" size="sm" onClick={onBack} className="-ml-2">
          <Icons.ArrowLeft className="mr-1 h-4 w-4" />
          Asset Classes
        </Button>
        {onAddForClass ? (
          <AddHoldingMenu cls={cls} onManualAdd={() => onAddForClass(cls)} size="inline" />
        ) : null}
      </div>

      <div className="space-y-3">
        <div className="flex items-center gap-3">
          <div className="bg-primary/10 text-primary flex h-12 w-12 items-center justify-center rounded-xl">
            {Icon ? <Icon className="h-6 w-6" /> : null}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline justify-between gap-2">
              <h2 className="text-lg font-semibold leading-tight">{labels.plural}</h2>
              {sortedHoldings.length > 0 && (
                <span className="text-muted-foreground text-xs tabular-nums">
                  {weightPercent.toFixed(1)}% of portfolio
                </span>
              )}
            </div>
            <p className="text-muted-foreground mt-0.5 flex items-center gap-2 text-sm">
              {sortedHoldings.length === 0 ? (
                <span>No {labels.plural.toLowerCase()} yet</span>
              ) : (
                <>
                  <span className="text-foreground font-medium tabular-nums">
                    {formatCompactAmount(totalValue, portfolioCurrency)}
                  </span>
                  <span>
                    {sortedHoldings.length}{" "}
                    {sortedHoldings.length === 1
                      ? labels.singular.toLowerCase()
                      : labels.plural.toLowerCase()}
                  </span>
                  <GainBadge value={totalGain} currency={portfolioCurrency} />
                </>
              )}
            </p>
          </div>
        </div>
        {sortedHoldings.length > 0 && (
          <WeightBar percent={weightPercent} color={assetClassColor(cls)} />
        )}
      </div>

      {sortedHoldings.length === 0 ? (
        <AssetClassEmptyState cls={cls} onAddForClass={onAddForClass} />
      ) : (
        <>
          {bucket && bucket.weightPercent > 0 && (
            <AssetClassHistoryChart
              cls={cls}
              bucket={bucket}
              accountId={accountId}
              portfolioCurrency={portfolioCurrency}
            />
          )}
          <ul className="bg-card divide-border divide-y overflow-hidden rounded-md border">
            {sortedHoldings.map((h) => (
              <HoldingRow
                key={h.id}
                holding={h}
                portfolioCurrency={portfolioCurrency}
                classTotalForWeights={classTotalForWeights}
                fallbackLabel={labels.singular}
              />
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

interface HoldingRowProps {
  holding: Holding;
  portfolioCurrency: string;
  classTotalForWeights: number;
  fallbackLabel: string;
}

function HoldingRow({
  holding,
  portfolioCurrency,
  classTotalForWeights,
  fallbackLabel,
}: HoldingRowProps) {
  const value = holding.marketValue?.base ?? 0;
  const weightWithinClass = (value / classTotalForWeights) * 100;
  const dayChange = holding.dayChange?.base ?? null;
  const totalGain = holding.totalGain?.base ?? null;

  return (
    <li className="flex items-center justify-between gap-3 px-4 py-3">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">
          {holding.instrument?.name || holding.instrument?.symbol || fallbackLabel}
        </p>
        <div className="text-muted-foreground mt-0.5 flex items-center gap-2 text-xs">
          {holding.instrument?.symbol ? (
            <span className="font-mono">{holding.instrument.symbol}</span>
          ) : null}
          {holding.localCurrency && holding.localCurrency !== portfolioCurrency ? (
            <Badge variant="outline" className="px-1.5 py-0 text-[10px]">
              {holding.localCurrency}
            </Badge>
          ) : null}
          {holding.quantity ? (
            <span className="tabular-nums">
              {holding.quantity.toLocaleString(undefined, { maximumFractionDigits: 4 })} ×
            </span>
          ) : null}
        </div>
      </div>
      <div className="shrink-0 text-right">
        <p className="text-sm font-semibold tabular-nums">
          {formatCompactAmount(value, portfolioCurrency)}
        </p>
        <div className="text-muted-foreground mt-0.5 flex items-center justify-end gap-2 text-[11px] tabular-nums">
          <span>{weightWithinClass.toFixed(1)}%</span>
          {dayChange !== null ? (
            <GainText value={dayChange} currency={portfolioCurrency} prefix="d" />
          ) : totalGain !== null ? (
            <GainText value={totalGain} currency={portfolioCurrency} />
          ) : null}
        </div>
        <PriceAsOf asOfDate={holding.asOfDate} />
      </div>
    </li>
  );
}

/**
 * Provenance label for a holding's price (enterprise §5/§10). Renders
 * "as of <date>" so a price is never shown without its as-of time. When
 * the quote is stale (older than the freshness window) it switches to an
 * amber "as of N days ago" so a not-recently-synced price is visible
 * rather than silently trusted. Pairs with the backend change that now
 * values stale holdings at the last-known price instead of swapping in
 * cost basis — the number and this label always correspond.
 */
const PRICE_STALE_AFTER_DAYS = 7;

function PriceAsOf({ asOfDate }: { asOfDate: string }) {
  const parsed = new Date(asOfDate);
  if (!isValid(parsed)) return null;
  const ageDays = differenceInCalendarDays(new Date(), parsed);
  const stale = ageDays > PRICE_STALE_AFTER_DAYS;
  return (
    <p
      className={`mt-0.5 text-[10px] tabular-nums ${stale ? "text-amber-500" : "text-muted-foreground/70"}`}
      title={format(parsed, "PPP")}
    >
      as of {ageDays <= 0 ? "today" : formatDistanceToNowStrict(parsed, { addSuffix: true })}
    </p>
  );
}

interface AssetClassEmptyStateProps {
  cls: AssetClass;
  onAddForClass?: (cls: AssetClass) => void;
}

function AssetClassEmptyState({ cls, onAddForClass }: AssetClassEmptyStateProps) {
  const labels = ASSET_CLASS_LABELS[cls];
  const Icon = Icons[ASSET_CLASS_ICON_NAMES[cls] as IconName];

  return (
    <Card className="border-dashed">
      <CardContent className="flex flex-col items-center justify-center px-6 py-10 text-center">
        <div className="bg-muted text-muted-foreground mb-4 flex h-12 w-12 items-center justify-center rounded-full">
          {Icon ? <Icon className="h-6 w-6" /> : null}
        </div>
        <h3 className="text-sm font-medium">No {labels.plural.toLowerCase()} yet</h3>
        <p className="text-muted-foreground mt-1 max-w-sm text-xs">
          You don&apos;t have any {labels.plural.toLowerCase()}. Please add now.
        </p>
        {onAddForClass ? (
          <div className="mt-4">
            <AddHoldingMenu cls={cls} onManualAdd={() => onAddForClass(cls)} size="cta" />
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Shared visual atoms
// ---------------------------------------------------------------------------

interface WeightBarProps {
  percent: number;
  /** Per-asset-class accent so the bar matches that class's history chart. */
  color?: string;
}

function WeightBar({ percent, color }: WeightBarProps) {
  // Cosmetic clamp — defensive against off-by-floating-point overshoot
  // and any future caller that passes 0..1 by mistake.
  const clamped = Math.max(0, Math.min(100, percent));
  const display = clamped >= 10 ? clamped.toFixed(0) : clamped.toFixed(1);

  return (
    <div className="space-y-1">
      <div className="bg-muted h-1.5 w-full overflow-hidden rounded-full">
        <div
          className={`h-full rounded-full transition-all ${color ? "" : "bg-primary"}`}
          style={{ width: `${clamped}%`, ...(color ? { backgroundColor: color } : {}) }}
          aria-hidden
        />
      </div>
      <p className="text-muted-foreground text-[10px] tabular-nums">{display}% of portfolio</p>
    </div>
  );
}

interface GainBadgeProps {
  value: number | null;
  currency: string;
}

/**
 * Compact +/-/zero gain pill. `null` collapses to nothing — keeps the
 * card layout from showing a misleading "$0" when the underlying
 * holdings simply don't report gain data yet (manual-pricing assets,
 * fresh imports, etc).
 */
function GainBadge({ value, currency }: GainBadgeProps) {
  if (value === null || !Number.isFinite(value)) return null;
  const tone =
    value > 0 ? "text-success" : value < 0 ? "text-destructive" : "text-muted-foreground";
  const sign = value > 0 ? "+" : value < 0 ? "−" : "";
  return (
    <span className={`mt-0.5 block text-[11px] tabular-nums ${tone}`}>
      {sign}
      {formatCompactAmount(Math.abs(value), currency)}
    </span>
  );
}

interface GainTextProps {
  value: number;
  currency: string;
  prefix?: string;
}

function GainText({ value, currency, prefix }: GainTextProps) {
  if (!Number.isFinite(value)) return null;
  const tone =
    value > 0 ? "text-success" : value < 0 ? "text-destructive" : "text-muted-foreground";
  const sign = value > 0 ? "+" : value < 0 ? "−" : "";
  return (
    <span className={tone}>
      {prefix ? `${prefix} ` : ""}
      {sign}
      {formatCompactAmount(Math.abs(value), currency)}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Loading state
// ---------------------------------------------------------------------------

function AssetClassesSkeleton() {
  return (
    <div className="space-y-6">
      <div className="flex items-baseline justify-between px-1">
        <Skeleton className="h-5 w-32" />
        <Skeleton className="h-3 w-48" />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {Array.from({ length: 3 }, (_, i) => (
          <Skeleton key={i} className="h-[112px] w-full rounded-lg" />
        ))}
      </div>
      <div className="flex flex-wrap gap-2">
        {Array.from({ length: 5 }, (_, i) => (
          <Skeleton key={i} className="h-7 w-24 rounded-full" />
        ))}
      </div>
    </div>
  );
}

export default AssetClassesView;
