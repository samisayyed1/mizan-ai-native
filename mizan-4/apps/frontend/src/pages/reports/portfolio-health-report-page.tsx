/**
 * Portfolio Health Report — fourth of the four Reports tiles Sami listed
 * by name. Sibling to the Liability Payoff Report page.
 *
 * Reports tile spec:
 *   "0–100 composite score across concentration, FX exposure, cash drag,
 *    and allocation drift. Highlights the weakest driver."
 *
 * That maps directly to the existing `compute_portfolio_health` Tauri
 * command (`adapters/shared/reports.ts::computePortfolioHealth`). The
 * existing `/health` route is for DATA-QUALITY checks (missing prices,
 * unbalanced ledger entries, etc.) — a different surface. This page is
 * for PORTFOLIO RISK health.
 */
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  EmptyPlaceholder,
  Page,
  PageContent,
  PageHeader,
  Skeleton,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";

import { computePortfolioHealth, type HealthDriver, type HealthPosition } from "@/adapters";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import type { Holding } from "@/lib/types";

function holdingToPosition(h: Holding, baseCurrency: string): HealthPosition | null {
  const valueBase = h.marketValue?.base;
  if (!Number.isFinite(valueBase) || (valueBase ?? 0) <= 0) return null;
  const assetClassKey = h.instrument?.classifications?.assetType?.key ?? "OTHER";
  return {
    label: h.instrument?.name?.trim() || h.instrument?.symbol || h.id,
    assetClass: assetClassKey,
    valueBase: String(valueBase),
    isForeignCurrency: h.localCurrency !== baseCurrency,
    // HoldingType is lowercase 'cash' in TS — see lib/constants.ts. Also
    // treat a holding whose symbol matches the base currency as cash
    // (cash positions in some adapters serialize that way).
    isCash: h.holdingType === "cash" || (h.instrument?.symbol ?? "") === baseCurrency,
  };
}

function scoreColor(score: number): string {
  if (score >= 80) return "hsl(140 50% 45%)"; // green
  if (score >= 60) return "hsl(45 80% 50%)"; // amber
  if (score >= 40) return "hsl(25 80% 50%)"; // orange
  return "hsl(0 70% 50%)"; // red
}

function ScoreRing({ score }: { score: number }) {
  // Single conic-gradient ring + the score in the middle. SVG-free so
  // the visual stays sharp on hi-DPI and respects the CSS color tokens.
  const color = scoreColor(score);
  return (
    <div
      className="relative flex h-32 w-32 items-center justify-center rounded-full"
      style={{
        background: `conic-gradient(${color} ${score * 3.6}deg, hsl(var(--muted)) 0deg)`,
      }}
    >
      <div className="bg-background flex h-24 w-24 items-center justify-center rounded-full">
        <div className="text-center">
          <p className="text-3xl font-bold tabular-nums" style={{ color }}>
            {score}
          </p>
          <p className="text-muted-foreground text-[10px] uppercase tracking-wide">/ 100</p>
        </div>
      </div>
    </div>
  );
}

function DriverCard({ driver, isWorst }: { driver: HealthDriver; isWorst: boolean }) {
  const score = Number(driver.score);
  const metricPct = Number(driver.metric) * 100;
  return (
    <Card className={isWorst ? "border-destructive/40" : ""}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{driver.label}</CardTitle>
          {isWorst && (
            <span className="text-destructive text-[10px] font-semibold uppercase tracking-wide">
              Weakest
            </span>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        <div className="flex items-baseline gap-2">
          <span
            className="text-2xl font-semibold tabular-nums"
            style={{ color: scoreColor(score) }}
          >
            {score}
          </span>
          <span className="text-muted-foreground text-xs">/100</span>
          <span className="text-muted-foreground ml-auto text-xs tabular-nums">
            {metricPct.toFixed(1)}%
          </span>
        </div>
        <div className="bg-muted h-1.5 overflow-hidden rounded-full">
          <div
            className="h-full transition-all"
            style={{ width: `${score}%`, background: scoreColor(score) }}
          />
        </div>
        <p className="text-muted-foreground text-xs leading-snug">{driver.note}</p>
      </CardContent>
    </Card>
  );
}

export default function PortfolioHealthReportPage() {
  const { holdings, isLoading: holdingsLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const positions = useMemo(() => {
    if (!holdings) return [];
    return holdings
      .map((h) => holdingToPosition(h, baseCurrency))
      .filter((p): p is HealthPosition => p !== null);
  }, [holdings, baseCurrency]);

  const reportQuery = useQuery({
    queryKey: ["portfolio-health", baseCurrency, positions.length],
    enabled: positions.length > 0,
    queryFn: () =>
      computePortfolioHealth({
        positions,
        baseCurrency,
      }),
    staleTime: 5 * 60_000,
  });

  if (holdingsLoading || reportQuery.isLoading) {
    return (
      <Page>
        <PageHeader heading="Portfolio health" />
        <PageContent>
          <Skeleton className="h-64 w-full" />
        </PageContent>
      </Page>
    );
  }

  if (positions.length === 0) {
    return (
      <Page>
        <PageHeader heading="Portfolio health" />
        <PageContent>
          <EmptyPlaceholder>
            <EmptyPlaceholder.Icon name="PieChart" />
            <EmptyPlaceholder.Title>Add a position first</EmptyPlaceholder.Title>
            <EmptyPlaceholder.Description>
              The health report scores your portfolio across concentration, FX exposure, cash
              drag, and allocation drift. Add at least one holding with a known market value to
              compute a score.
            </EmptyPlaceholder.Description>
          </EmptyPlaceholder>
        </PageContent>
      </Page>
    );
  }

  if (reportQuery.error || !reportQuery.data) {
    return (
      <Page>
        <PageHeader heading="Portfolio health" />
        <PageContent>
          <Card className="border-destructive/30 bg-destructive/5">
            <CardContent className="py-4">
              <p className="text-destructive text-sm font-medium">Couldn't compute health.</p>
              <p className="text-muted-foreground mt-1 text-xs">
                {reportQuery.error instanceof Error
                  ? reportQuery.error.message
                  : "The backend rejected the call. Verify your provider settings + try again."}
              </p>
            </CardContent>
          </Card>
        </PageContent>
      </Page>
    );
  }

  const report = reportQuery.data;
  const score = report.score === null ? 0 : Number(report.score);
  const worstId = report.worstDriver?.id ?? null;

  return (
    <Page>
      <PageHeader heading="Portfolio health" />
      <PageContent>
        <p className="text-muted-foreground mb-4 text-sm">
          A 0–100 composite score across concentration, FX exposure, cash drag, and allocation
          drift. Computed locally from your current holdings — your data never leaves the device.
        </p>

        <Card className="mb-6">
          <CardContent className="flex flex-col items-center gap-4 py-6 md:flex-row md:justify-between">
            <div className="flex items-center gap-6">
              <ScoreRing score={score} />
              <div>
                <p className="text-muted-foreground text-xs uppercase tracking-wide">
                  Composite score
                </p>
                <p className="text-2xl font-semibold">
                  {score >= 80
                    ? "Strong"
                    : score >= 60
                      ? "Healthy"
                      : score >= 40
                        ? "Watch"
                        : "Attention needed"}
                </p>
                {report.worstDriver && (
                  <p className="text-muted-foreground mt-1 text-xs">
                    Weakest driver:{" "}
                    <span className="text-foreground font-medium">
                      {report.worstDriver.label}
                    </span>{" "}
                    ({Number(report.worstDriver.score)}/100)
                  </p>
                )}
              </div>
            </div>
            <div className="text-muted-foreground text-xs">
              {report.drivers.length} driver{report.drivers.length === 1 ? "" : "s"} ·{" "}
              {positions.length} position{positions.length === 1 ? "" : "s"}
            </div>
          </CardContent>
        </Card>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {report.drivers.map((d) => (
            <DriverCard key={d.id} driver={d} isWorst={d.id === worstId} />
          ))}
        </div>

        {report.notes.length > 0 && (
          <Card className="mt-6">
            <CardHeader>
              <CardTitle className="text-sm">Notes</CardTitle>
              <CardDescription className="text-xs">
                Caveats the calculation surfaced.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-1 text-xs">
              {report.notes.map((n, i) => (
                <p key={i} className="text-muted-foreground">
                  · {n}
                </p>
              ))}
            </CardContent>
          </Card>
        )}

        <p className="text-muted-foreground mt-6 text-center text-xs">
          <Icons.Info className="mr-1 inline size-3" />
          Computed via the local <code>compute_portfolio_health</code> backend.
        </p>
      </PageContent>
    </Page>
  );
}
