/**
 * Liability Payoff Report — one of the four Reports tiles Sami flagged
 * as 404 (PR #221 fixed the routing; this gives the tile a proper
 * destination).
 *
 * For every fixed-rate liability the user owns (mortgage, auto loan,
 * student loan, personal loan), this page surfaces:
 *
 *   - Monthly payment (derived if not stored)
 *   - Total interest over the remaining term
 *   - Projected payoff date
 *   - Balance trajectory chart
 *   - Full amortization schedule (collapsed by default; expand on click)
 *
 * Backed by the existing `compute_amortization` Tauri command + the
 * `AmortizationReport` shape already wired in adapters/shared/reports.ts.
 * No new backend work needed; this is the missing UI surface.
 */
import { useMemo, useState } from "react";
import { useQueries } from "@tanstack/react-query";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  EmptyPlaceholder,
  Page,
  PageContent,
  PageHeader,
  Skeleton,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";

import { computeAmortization, type AmortizationReport } from "@/adapters";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import type { Holding } from "@/lib/types";

interface LiabilityLite {
  id: string;
  name: string;
  liabilityType: string;
  principal: number;
  ratePct: number | null;
  monthlyPayment: number | null;
  originatedAt: string | null;
  currency: string;
}

function liabilityFromHolding(h: Holding): LiabilityLite | null {
  if (h.assetKind !== "LIABILITY") return null;
  const meta = (h.instrument?.metadata ?? {}) as Record<string, unknown>;
  const principal = Number(meta.principal);
  if (!Number.isFinite(principal) || principal <= 0) return null;
  const ratePctRaw = Number(meta.rate_pct);
  const ratePct = Number.isFinite(ratePctRaw) ? ratePctRaw : null;
  const monthlyRaw = Number(meta.monthly_payment);
  const monthlyPayment = Number.isFinite(monthlyRaw) && monthlyRaw > 0 ? monthlyRaw : null;
  const originatedAt =
    typeof meta.originated_at === "string" && meta.originated_at.length >= 10
      ? meta.originated_at.slice(0, 10)
      : null;
  return {
    id: h.instrument?.id ?? h.id,
    name: h.instrument?.name?.trim() || "Liability",
    liabilityType: typeof meta.sub_type === "string" ? meta.sub_type : "LIABILITY",
    principal,
    ratePct,
    monthlyPayment,
    originatedAt,
    currency: h.localCurrency,
  };
}

function todayISO(): string {
  return new Date().toISOString().slice(0, 10);
}

function fmtCurrency(amount: number, currency: string): string {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(amount);
}

function fmtRate(ratePct: number): string {
  return `${ratePct.toFixed(2)}%`;
}

interface LiabilityReportCardProps {
  liability: LiabilityLite;
  report: AmortizationReport | null;
  isLoading: boolean;
  error: string | null;
}

function LiabilityReportCard({ liability, report, isLoading, error }: LiabilityReportCardProps) {
  const [showSchedule, setShowSchedule] = useState(false);

  if (error) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-base">{liability.name}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-destructive text-sm">{error}</p>
          <p className="text-muted-foreground mt-2 text-xs">
            Make sure the liability has principal + an annual rate set. Edit it through{" "}
            <code>update_liability</code> in the AI assistant or the Holdings page.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (isLoading || !report) {
    return (
      <Card>
        <CardHeader>
          <Skeleton className="h-5 w-44" />
        </CardHeader>
        <CardContent className="space-y-3">
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-48 w-full" />
        </CardContent>
      </Card>
    );
  }

  const monthlyPayment = Number(report.monthlyPayment);
  const totalInterest = Number(report.totalInterest);
  const totalPaid = Number(report.totalPaid);
  const currency = report.currency ?? liability.currency;

  const chartData = report.schedule.map((row) => ({
    period: row.period,
    balance: Number(row.balanceAfter),
    cumulativeInterest: report.schedule
      .slice(0, row.period)
      .reduce((acc, r) => acc + Number(r.interest), 0),
  }));

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-base">{liability.name}</CardTitle>
          <span className="text-muted-foreground text-xs">
            {liability.liabilityType} · {fmtRate(liability.ratePct ?? 0)}
          </span>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <Metric label="Principal" value={fmtCurrency(liability.principal, currency)} />
          <Metric label="Monthly payment" value={fmtCurrency(monthlyPayment, currency)} />
          <Metric label="Total interest" value={fmtCurrency(totalInterest, currency)} />
          <Metric label="Payoff date" value={report.payoffDate} />
        </div>

        <div className="h-48">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData} margin={{ top: 8, right: 8, left: 8, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" opacity={0.2} />
              <XAxis dataKey="period" tick={{ fontSize: 10 }} />
              <YAxis
                tick={{ fontSize: 10 }}
                tickFormatter={(v: number) => `${Math.round(v / 1000)}k`}
              />
              <Tooltip
                formatter={(value) => fmtCurrency(Number(value), currency) as string}
                labelFormatter={(period) => `Month ${period}`}
              />
              <Line
                type="monotone"
                dataKey="balance"
                stroke="hsl(31 38% 46%)"
                strokeWidth={2}
                dot={false}
                name="Remaining balance"
              />
              <Line
                type="monotone"
                dataKey="cumulativeInterest"
                stroke="hsl(0 70% 50%)"
                strokeWidth={1.5}
                dot={false}
                strokeDasharray="4 2"
                name="Cumulative interest paid"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {report.notes.length > 0 && (
          <div className="border-warning/40 bg-warning/10 text-warning rounded-md border px-3 py-2 text-xs">
            {report.notes.map((n, i) => (
              <div key={i}>{n}</div>
            ))}
          </div>
        )}

        <div className="flex items-center justify-between">
          <p className="text-muted-foreground text-xs">
            Total paid over remaining term: {fmtCurrency(totalPaid, currency)} (
            {fmtCurrency(liability.principal, currency)} principal +{" "}
            {fmtCurrency(totalInterest, currency)} interest)
          </p>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowSchedule((v) => !v)}
            className="text-xs"
          >
            {showSchedule ? "Hide schedule" : `Show ${report.schedule.length}-month schedule`}
          </Button>
        </div>

        {showSchedule && (
          <div className="max-h-64 overflow-y-auto rounded-md border">
            <table className="text-xs w-full">
              <thead className="bg-muted/50 sticky top-0 text-left">
                <tr>
                  <th className="px-3 py-2">#</th>
                  <th className="px-3 py-2">Due</th>
                  <th className="px-3 py-2 text-right">Payment</th>
                  <th className="px-3 py-2 text-right">Interest</th>
                  <th className="px-3 py-2 text-right">Principal</th>
                  <th className="px-3 py-2 text-right">Balance after</th>
                </tr>
              </thead>
              <tbody>
                {report.schedule.map((row) => (
                  <tr key={row.period} className="border-t">
                    <td className="px-3 py-1.5">{row.period}</td>
                    <td className="px-3 py-1.5">{row.dueDate}</td>
                    <td className="px-3 py-1.5 text-right tabular-nums">
                      {fmtCurrency(Number(row.payment), currency)}
                    </td>
                    <td className="px-3 py-1.5 text-right tabular-nums">
                      {fmtCurrency(Number(row.interest), currency)}
                    </td>
                    <td className="px-3 py-1.5 text-right tabular-nums">
                      {fmtCurrency(Number(row.principal), currency)}
                    </td>
                    <td className="px-3 py-1.5 text-right tabular-nums">
                      {fmtCurrency(Number(row.balanceAfter), currency)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-muted-foreground text-xs uppercase tracking-wide">{label}</p>
      <p className="mt-0.5 text-sm font-semibold tabular-nums">{value}</p>
    </div>
  );
}

export default function LiabilityPayoffReportPage() {
  const { holdings, isLoading: holdingsLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const liabilities = useMemo(() => {
    if (!holdings) return [];
    return holdings
      .map(liabilityFromHolding)
      .filter((l): l is LiabilityLite => l !== null)
      .sort((a, b) => b.principal - a.principal);
  }, [holdings]);

  // One amortization query per liability — react-query handles caching,
  // retries, and the per-liability loading state independently.
  const queries = useQueries({
    queries: liabilities.map((l) => ({
      queryKey: ["amortization", l.id, l.principal, l.ratePct, l.monthlyPayment],
      queryFn: async (): Promise<AmortizationReport> => {
        if (l.ratePct === null) {
          throw new Error("Annual rate is missing — amortization can't be computed.");
        }
        return computeAmortization({
          currentBalance: String(l.principal),
          annualRate: String(l.ratePct / 100),
          monthlyPayment: l.monthlyPayment != null ? String(l.monthlyPayment) : undefined,
          remainingMonths: null, // backend defaults to 360 (50-year cap)
          startDate: l.originatedAt ?? todayISO(),
          currency: l.currency,
        });
      },
      // Inputs are deterministic — no need to re-run on focus or stale window.
      staleTime: Infinity,
      retry: 0,
    })),
  });

  const totalInterestAcross = useMemo(() => {
    let sum = 0;
    queries.forEach((q, i) => {
      if (q.data && liabilities[i]) {
        sum += Number(q.data.totalInterest);
      }
    });
    return sum;
  }, [queries, liabilities]);

  const totalPrincipal = liabilities.reduce((acc, l) => acc + l.principal, 0);

  if (holdingsLoading) {
    return (
      <Page>
        <PageHeader heading="Liability payoff" />
        <PageContent>
          <Skeleton className="h-32 w-full" />
        </PageContent>
      </Page>
    );
  }

  if (liabilities.length === 0) {
    return (
      <Page>
        <PageHeader heading="Liability payoff" />
        <PageContent>
          <EmptyPlaceholder>
            <EmptyPlaceholder.Icon name="Activity2" />
            <EmptyPlaceholder.Title>No liabilities yet</EmptyPlaceholder.Title>
            <EmptyPlaceholder.Description>
              Add a mortgage, loan, or credit card through the AI assistant or the Holdings page to
              see its amortization schedule + projected payoff here.
            </EmptyPlaceholder.Description>
          </EmptyPlaceholder>
        </PageContent>
      </Page>
    );
  }

  return (
    <Page>
      <PageHeader heading="Liability payoff" />
      <PageContent>
        <p className="text-muted-foreground mb-4 text-sm">
          Amortization schedule, total interest cost, and projected payoff date for every fixed-rate
          liability you own. Numbers are computed locally from each liability's principal, annual
          rate, and (optional) monthly payment.
        </p>

        <Card className="mb-6">
          <CardContent className="grid grid-cols-2 gap-4 py-4 md:grid-cols-3">
            <Metric
              label="Total principal"
              value={fmtCurrency(totalPrincipal, baseCurrency)}
            />
            <Metric
              label="Projected total interest"
              value={fmtCurrency(totalInterestAcross, baseCurrency)}
            />
            <Metric label="Liabilities tracked" value={`${liabilities.length}`} />
          </CardContent>
        </Card>

        <Tabs defaultValue={liabilities[0]?.id ?? ""} className="space-y-4">
          <TabsList>
            {liabilities.map((l) => (
              <TabsTrigger key={l.id} value={l.id}>
                {l.name}
              </TabsTrigger>
            ))}
          </TabsList>
          {liabilities.map((l, i) => {
            const q = queries[i];
            return (
              <TabsContent key={l.id} value={l.id}>
                <LiabilityReportCard
                  liability={l}
                  report={q?.data ?? null}
                  isLoading={Boolean(q?.isLoading)}
                  error={q?.error instanceof Error ? q.error.message : null}
                />
              </TabsContent>
            );
          })}
        </Tabs>

        <p className="text-muted-foreground mt-6 text-center text-xs">
          <Icons.Info className="mr-1 inline size-3" />
          Computed via the local <code>compute_amortization</code> backend — your liability data
          never leaves the device.
        </p>
      </PageContent>
    </Page>
  );
}
