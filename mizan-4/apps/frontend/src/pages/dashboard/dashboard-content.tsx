import { getEstimatedHistoricalValuation } from "@/adapters";
import { HistoryChart } from "@/components/history-chart";
import { TickerConveyor } from "@/components/ticker-conveyor";
import { useHapticFeedback } from "@/hooks";
import { useHoldings } from "@/hooks/use-holdings";
import { useValuationExtent } from "@/hooks/use-valuation-extent";
import { useValuationHistory } from "@/hooks/use-valuation-history";
import { useEntitlements } from "@/features/mizan-connect";
import type { AccountValuation } from "@/lib/types";
import { isAlternativeAssetKind, PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { DateRange, TimePeriod } from "@/lib/types";
import { calculatePerformanceMetrics } from "@/lib/utils";
import { PortfolioUpdateTrigger } from "@/pages/dashboard/portfolio-update-trigger";
import type { TimePeriod as UITimePeriod } from "@mizan/ui";
import {
  GainAmount,
  GainPercent,
  getInitialIntervalData,
  IntervalSelector,
  usePersistentState,
} from "@mizan/ui";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { differenceInDays, format, parseISO } from "date-fns";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { AccountsSummary } from "./accounts-summary";
import { HoldingsHeatmap } from "./holdings-heatmap";
import { NewsHomeWidget } from "./news-home-widget";
import { PortfolioHealthCard } from "./portfolio-health-card";
import { ZakatCard } from "./zakat-card";
import Balance from "./balance";
import SavingGoals from "./goals";
import { AssetClassPanelGrid } from "@/components/asset-class-panels";
import { NetWorthStrip } from "@/components/dashboard/net-worth-strip";
import { AiCommandBar } from "@/components/dashboard/ai-command-bar";

const DEFAULT_INTERVAL: UITimePeriod = "3M";
const INTERVAL_STORAGE_KEY = "dashboard-interval";

export function DashboardContent() {
  // Use the same persisted state as IntervalSelector for the interval code
  const [intervalCode] = usePersistentState<UITimePeriod>(INTERVAL_STORAGE_KEY, DEFAULT_INTERVAL);

  // Derive initial values from the persisted interval code
  const [dateRange, setDateRange] = useState<DateRange | undefined>(
    () => getInitialIntervalData(intervalCode).range,
  );
  const [selectedIntervalDescription, setSelectedIntervalDescription] = useState<string>(
    () => getInitialIntervalData(intervalCode).description,
  );
  const [isAllTime, setIsAllTime] = useState<boolean>(() => intervalCode === "ALL");

  const { holdings: allHoldings, isLoading: isHoldingsLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { triggerHaptic } = useHapticFeedback();
  // Data extent for gating IntervalSelector — hides 5Y/1Y/6M etc. when
  // the user only has a few weeks of history. Cached aggressively so
  // toggling between intervals doesn't re-fetch.
  const dataEarliestDate = useValuationExtent(PORTFOLIO_ACCOUNT_ID);

  // Total portfolio value (includes cash, excludes alternative assets)
  const totalValue = useMemo(() => {
    if (!allHoldings) return 0;
    return allHoldings
      .filter((h) => {
        return !(h.assetKind && isAlternativeAssetKind(h.assetKind));
      })
      .reduce((acc, holding) => acc + (holding.marketValue?.base ?? 0), 0);
  }, [allHoldings]);

  // Toggle: when ON, the chart shows an *estimated* historical curve
  // computed by pricing current holdings against historical quotes (good
  // for accounts where the broker only delivered a current snapshot).
  //
  // UX-10 gating: the "Estimate full history" button is gated behind
  // brokerSync (Plaid / broker imports enabled) per user request. Free
  // users see only the actual valuation history they've recorded —
  // promising 5-year synthesised charts on a fresh install over-promises
  // what the dataset can support. Paid users who've connected via Plaid
  // get the option because they typically only have a starting-snapshot
  // worth of history and need the synthesis to populate the chart.
  const { entitlements } = useEntitlements();
  const showEstimateToggle = entitlements.brokerSync;
  const [useEstimatedHistory, setUseEstimatedHistory] = useState(false);

  const { valuationHistory: realValuationHistory, isLoading: isRealHistoryLoading } =
    useValuationHistory(useEstimatedHistory ? undefined : dateRange);

  const { data: estimatedValuationHistory, isLoading: isEstimatedLoading } = useQuery<
    AccountValuation[]
  >({
    queryKey: ["estimated-historical-valuation", PORTFOLIO_ACCOUNT_ID],
    queryFn: () => getEstimatedHistoricalValuation(PORTFOLIO_ACCOUNT_ID),
    enabled: useEstimatedHistory,
    staleTime: 5 * 60 * 1000,
  });

  const valuationHistory = useEstimatedHistory ? estimatedValuationHistory : realValuationHistory;
  const isValuationHistoryLoading = useEstimatedHistory ? isEstimatedLoading : isRealHistoryLoading;

  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  // Calculate gainLossAmount and simpleReturn from valuationHistory
  const { gainLossAmount, simpleReturn } = useMemo(() => {
    return calculatePerformanceMetrics(valuationHistory, isAllTime);
  }, [valuationHistory, isAllTime]);

  const currentValuation = useMemo(() => {
    return valuationHistory && valuationHistory.length > 0
      ? valuationHistory[valuationHistory.length - 1]
      : null;
  }, [valuationHistory]);

  const chartData = useMemo(() => {
    return (
      valuationHistory?.map((item) => ({
        date: item.valuationDate,
        totalValue: item.totalValue,
        netContribution: item.netContribution,
        currency: item.baseCurrency ?? baseCurrency,
      })) ?? []
    );
  }, [valuationHistory, baseCurrency]);

  const isNegative = totalValue < 0;

  // Sparse-data hint: when the user requests a wider window than they
  // actually have data for, the chart looks identical across multiple
  // intervals (1Y vs 3M etc) which feels broken. Detect that and surface
  // a subtle "data starts X" annotation so the chart's silence makes sense.
  const sparseDataHint = useMemo(() => {
    if (isAllTime || !valuationHistory?.length || !dateRange?.from) {
      return null;
    }
    const earliestRow = valuationHistory[0];
    if (!earliestRow?.valuationDate) return null;
    const earliestDataDate = parseISO(earliestRow.valuationDate);
    // Threshold: if the earliest data point is more than 7 days after the
    // requested window start, the chart is effectively clamped. Tell the
    // user with short, readable text — long hint strings overflow narrow
    // viewports and look bad in the monospace font we use here.
    const gapDays = differenceInDays(earliestDataDate, dateRange.from);
    if (gapDays <= 7) return null;
    const days = valuationHistory.length;
    return `Earliest data ${format(earliestDataDate, "MMM d, yyyy")} · ${days} day${days === 1 ? "" : "s"}`;
  }, [valuationHistory, dateRange, isAllTime]);

  // Headline interval label — when data is sparse vs the requested window,
  // override "past 5 years" / "past year" with the actual extent so the
  // gain/loss percentage isn't misleadingly labeled. e.g. requesting 5Y
  // on a 3-month-old account: instead of "-14.57% past 5 years" (which
  // sounds like a 5-year drawdown), render "-14.57% past 92 days".
  const displayedIntervalDescription = useMemo(() => {
    if (sparseDataHint && valuationHistory?.length) {
      const days = valuationHistory.length;
      if (days < 31) return `past ${days} days`;
      const months = Math.round(days / 30);
      return `past ${months} month${months === 1 ? "" : "s"}`;
    }
    return selectedIntervalDescription;
  }, [sparseDataHint, valuationHistory, selectedIntervalDescription]);

  // Callback for IntervalSelector
  const handleIntervalSelect = (
    code: TimePeriod,
    description: string,
    range: DateRange | undefined,
  ) => {
    setSelectedIntervalDescription(description);
    setDateRange(range);
    setIsAllTime(code === "ALL");
  };

  return (
    <div className="flex min-h-screen flex-col">
      <TickerConveyor />
      <div className="px-4 pb-1 pt-2 md:px-6 md:pb-2 lg:px-8">
        <PortfolioUpdateTrigger lastCalculatedAt={currentValuation?.calculatedAt}>
          <div className="flex items-start gap-2">
            <div>
              <Balance
                isLoading={isHoldingsLoading}
                targetValue={totalValue}
                currency={baseCurrency}
                displayCurrency={true}
              />
              <div className="text-md flex space-x-3">
                {isValuationHistoryLoading && !valuationHistory ? (
                  <div className="flex items-center gap-3 pt-1">
                    <Skeleton className="h-4 w-24" />
                    <div className="border-secondary my-1 border-r pr-2" />
                    <Skeleton className="h-4 w-16" />
                  </div>
                ) : (
                  <>
                    <GainAmount
                      className="lg:text-md text-sm font-light"
                      value={gainLossAmount}
                      currency={baseCurrency}
                      displayCurrency={false}
                    ></GainAmount>
                    <div className="border-secondary my-1 border-r pr-2" />
                    <GainPercent
                      className="lg:text-md text-sm font-light"
                      value={simpleReturn}
                      animated={true}
                    ></GainPercent>
                  </>
                )}
                {displayedIntervalDescription && (
                  <span className="lg:text-md text-muted-foreground ml-1 text-sm font-light">
                    {displayedIntervalDescription}
                  </span>
                )}
              </div>
            </div>
          </div>
        </PortfolioUpdateTrigger>
      </div>

      <div
        className={`bg-linear-to-t flex grow flex-col ${
          isNegative
            ? "from-destructive/30 via-destructive/15 to-transparent"
            : "from-success/30 via-success/15 to-transparent"
        }`}
      >
        <div className="h-[280px]">
          <HistoryChart data={chartData} isLoading={isValuationHistoryLoading} />
          {valuationHistory && chartData.length > 0 && (
            <div className="flex w-full flex-col items-center gap-3 pt-1">
              {!useEstimatedHistory && (
                <IntervalSelector
                  className="pointer-events-auto relative z-20 w-full max-w-screen-sm sm:max-w-screen-md md:max-w-2xl lg:max-w-3xl"
                  onIntervalSelect={handleIntervalSelect}
                  onHaptic={triggerHaptic}
                  isLoading={isValuationHistoryLoading}
                  storageKey={INTERVAL_STORAGE_KEY}
                  defaultValue={DEFAULT_INTERVAL}
                  dataEarliestDate={dataEarliestDate}
                />
              )}
              <div className="flex w-full max-w-md flex-col items-center gap-2 px-4">
                {sparseDataHint && !useEstimatedHistory && (
                  <p
                    className="text-muted-foreground pointer-events-auto max-w-full truncate text-center text-[11px] leading-tight tracking-wide sm:text-xs"
                    title={sparseDataHint}
                  >
                    {sparseDataHint}
                  </p>
                )}
                {useEstimatedHistory && (
                  <p className="pointer-events-auto text-balance text-center text-[11px] leading-snug tracking-wide text-amber-500/85 sm:text-xs">
                    Estimated. Current holdings priced against historical quotes &mdash; past
                    trades, splits, and contributions are not reflected.
                  </p>
                )}
                {showEstimateToggle && (
                <button
                  type="button"
                  onClick={() => {
                    setUseEstimatedHistory((v) => !v);
                    triggerHaptic();
                  }}
                  className="border-border/60 bg-background/60 text-muted-foreground hover:text-foreground hover:border-border pointer-events-auto cursor-pointer rounded-full border px-3 py-1 text-[11px] font-medium tracking-wide transition-colors sm:text-xs"
                >
                  {useEstimatedHistory ? "Back to actual history" : "Estimate full history"}
                </button>
                )}
              </div>
            </div>
          )}
        </div>

        <div className="grow px-4 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] pt-12 md:px-6 md:pb-6 md:pt-6 lg:px-10 lg:pb-8 lg:pt-8">
          <div className="grid grid-cols-1 gap-8 lg:grid-cols-3 lg:gap-20">
            <div className="space-y-6 lg:col-span-2">
              {/* Track A PR-A6 — AI Command Bar per Goal v3 §V step A6.
                  Pinned full-width input; submit → /assistant with the
                  prompt pre-seeded. Voice button routes the user to the
                  assistant page's existing dictation surface until
                  PR-C18 wires direct voice transcription on the bar. */}
              <AiCommandBar />
              {/* Track A PR-A5 — Net Worth Strip per Goal v3 §V step A5.
                  Uses the all-time valuation history so toggling window
                  preset (24h / 7d / 30d / YTD / All) is a pure client
                  recompute — no extra fetch. Tap → /net-worth. */}
              <NetWorthStrip
                history={realValuationHistory}
                baseCurrency={baseCurrency}
                isLoading={isRealHistoryLoading}
                defaultWindow="30d"
              />
              <AccountsSummary dateRange={dateRange} isAllTime={isAllTime} />
              <HoldingsHeatmap
                holdings={allHoldings ?? []}
                isLoading={isHoldingsLoading}
                baseCurrency={baseCurrency}
              />
              {/* Track A PR-A4 — twelve-panel asset class grid per ADR 0021.
                  Skeleton tiles for now; Track B PR-B1..B7 swaps the
                  `/holdings?panel=...` link targets for dedicated panel
                  surfaces panel-by-panel. */}
              <AssetClassPanelGrid
                holdings={allHoldings ?? []}
                baseCurrency={baseCurrency}
              />
            </div>
            <div className="space-y-6 lg:col-span-1">
              <SavingGoals />
              <PortfolioHealthCard />
              <ZakatCard />
              <NewsHomeWidget />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default DashboardContent;
