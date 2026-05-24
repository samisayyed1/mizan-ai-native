/**
 * Asset-class history chart — Feroz Step 8.
 *
 * Drops in above the holdings list inside an asset-class drill-down,
 * showing how that class's value moved over time.
 *
 * Honest math
 * -----------
 * The backend does not yet expose a per-class historical valuation.
 * What we have is the portfolio's `useValuationHistory(...)` total.
 * This chart projects that total onto the class by **scaling each
 * point by the class's CURRENT weight** — the constant-weight
 * approximation.
 *
 * That approximation captures the SHAPE of how the class's allocated
 * value would have moved if the user had held today's mix throughout
 * the period. It is wrong during rebalancing periods and during
 * acquisitions / sells inside this class. We do NOT hide that. The
 * footer carries a visible "Estimated allocation" disclaimer + a
 * promise that real per-class history is coming.
 *
 * No-mistakes contract: the scaling math lives in
 * `lib/asset-classes.ts::scaleHistoryByWeight`, which is pure +
 * unit-tested (12 cases). The component is responsible only for
 * fetching, plumbing, and rendering — not for any number-crunching.
 *
 * Reference: .claude/product-notes/feroz-meeting-2026-05-17.md #8.
 */

import { HistoryChart } from "@/components/history-chart";
import { useValuationHistory } from "@/hooks/use-valuation-history";
import {
  ASSET_CLASS_LABELS,
  assetClassColor,
  scaleHistoryByWeight,
  type AssetClass,
  type AssetClassBucket,
} from "@/lib/asset-classes";
import type { DateRange } from "@/lib/types";
import {
  getInitialIntervalData,
  IntervalSelector,
  usePersistentState,
  type TimePeriod,
} from "@mizan/ui";
import { Card, CardContent } from "@mizan/ui/components/ui/card";
import { useMemo, useState } from "react";

const INTERVAL_STORAGE_KEY = "asset-class-history-interval";
const DEFAULT_INTERVAL: TimePeriod = "3M";

interface AssetClassHistoryChartProps {
  cls: AssetClass;
  bucket: AssetClassBucket;
  accountId: string;
  portfolioCurrency: string;
}

export function AssetClassHistoryChart({
  cls,
  bucket,
  accountId,
  portfolioCurrency,
}: AssetClassHistoryChartProps) {
  const labels = ASSET_CLASS_LABELS[cls];

  // Each class drill-down owns its own interval state, persisted so a
  // user who set Stocks to 1Y doesn't get reset to 3M next time they
  // navigate in. Shares the storage key across classes intentionally
  // — most users want the same lens across all of them.
  const [intervalCode] = usePersistentState<TimePeriod>(INTERVAL_STORAGE_KEY, DEFAULT_INTERVAL);
  const [dateRange, setDateRange] = useState<DateRange | undefined>(
    () => getInitialIntervalData(intervalCode).range,
  );

  const { valuationHistory, isLoading } = useValuationHistory(dateRange, accountId);

  // The class-level data is a pure derivation of the portfolio history
  // and the current weight — both stable for a given render — so we
  // memoise across (valuationHistory, weight, currency) only. The
  // scaling function returns `[]` when weight is 0 / non-finite, which
  // the wrapping conditional render handles by not mounting at all.
  const chartData = useMemo(
    () => scaleHistoryByWeight(valuationHistory ?? [], bucket.weightPercent, portfolioCurrency),
    [valuationHistory, bucket.weightPercent, portfolioCurrency],
  );

  const handleIntervalSelect = (
    _code: TimePeriod,
    _description: string,
    range: DateRange | undefined,
  ) => {
    setDateRange(range);
  };

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="text-sm font-semibold leading-tight">{labels.plural} history</h3>
          <span className="text-muted-foreground text-[10px] uppercase tracking-wider">
            Estimated allocation
          </span>
        </div>

        <div className="h-[200px]">
          <HistoryChart data={chartData} isLoading={isLoading} accentColor={assetClassColor(cls)} />
        </div>

        <IntervalSelector
          className="w-full"
          onIntervalSelect={handleIntervalSelect}
          isLoading={isLoading}
          storageKey={INTERVAL_STORAGE_KEY}
          defaultValue={DEFAULT_INTERVAL}
        />

        <p className="text-muted-foreground/80 text-[11px] leading-snug">
          Your current allocation ({bucket.weightPercent.toFixed(1)}%) applied to portfolio history.
          Per-class history with full transaction precision is coming with the next backend update.
        </p>
      </CardContent>
    </Card>
  );
}

export default AssetClassHistoryChart;
