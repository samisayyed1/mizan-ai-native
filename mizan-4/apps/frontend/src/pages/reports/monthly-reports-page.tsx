import { Page, PageContent, PageHeader } from "@mizan/ui";
import { Button } from "@mizan/ui/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format, parseISO } from "date-fns";
import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { listMonthlyReports, type MonthlyReport, requestMonthlyReport } from "@/adapters";
import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";
import { QueryKeys } from "@/lib/query-keys";

/**
 * Monthly AI Wealth Report (M3.6).
 *
 * The cloud cron writes a row at the start of each month; the user can also
 * request an on-demand regeneration via the header CTA. We render the
 * rendered markdown as-is — the AI proxy's `kind=monthly_report` system
 * prompt constrains the LLM to summarize-only (no advice, no predictions).
 *
 * Pro+ gate is on the cloud (`managed_ai`), so Free/Basic see an
 * UpgradeGate-driven empty state rather than the list.
 */
export default function MonthlyReportsPage() {
  const queryClient = useQueryClient();
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();
  const [activeId, setActiveId] = useState<string | null>(null);

  const reportsQuery = useQuery({
    queryKey: [QueryKeys.MONTHLY_REPORTS],
    queryFn: () => listMonthlyReports(12),
    enabled: entitlements.managedAi,
  });

  const requestMutation = useMutation({
    mutationFn: requestMonthlyReport,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.MONTHLY_REPORTS] });
    },
  });

  if (!entitlements.managedAi) {
    return (
      <Page>
        <PageHeader heading="Monthly wealth reports" />
        <PageContent>
          <Card className="border-primary/20 from-primary/5 to-card bg-linear-to-br mx-auto max-w-2xl p-8 text-center">
            <Icons.Sparkles className="text-primary mx-auto mb-3 h-10 w-10" />
            <h2 className="text-lg font-semibold">Monthly AI wealth report</h2>
            <p className="text-muted-foreground mx-auto mt-2 max-w-md text-sm leading-relaxed">
              A one-page summary of your net-worth delta, top movers, income received, goal
              progress, and liability trend — automatically generated each month from your own data.
              Included with a Mizan subscription.
            </p>
            <Button className="mt-4" onClick={() => requestUpgrade("managed_ai")}>
              Upgrade to unlock
            </Button>
          </Card>
        </PageContent>
      </Page>
    );
  }

  const reports = reportsQuery.data?.reports ?? [];
  const active =
    reports.find((r) => r.id === activeId) ??
    reports.find((r) => r.status === "succeeded") ??
    reports[0] ??
    null;

  return (
    <Page>
      <PageHeader
        heading="Monthly wealth reports"
        actions={
          <Button
            size="sm"
            onClick={() => requestMutation.mutate()}
            disabled={requestMutation.isPending}
          >
            {requestMutation.isPending ? (
              <>
                <Icons.Spinner className="mr-1.5 h-4 w-4 animate-spin" />
                Requesting…
              </>
            ) : (
              <>
                <Icons.Sparkles className="mr-1.5 h-4 w-4" />
                Generate now
              </>
            )}
          </Button>
        }
      />
      <PageContent>
        <div className="mx-auto grid w-full max-w-5xl grid-cols-1 gap-4 lg:grid-cols-[220px_1fr]">
          {/* Sidebar — report list */}
          <aside>
            {reportsQuery.isLoading ? (
              <div className="space-y-2">
                <Skeleton className="h-10 w-full" />
                <Skeleton className="h-10 w-full" />
              </div>
            ) : reports.length === 0 ? (
              <Card className="p-4 text-center">
                <p className="text-muted-foreground text-xs leading-relaxed">
                  No reports yet. Reports generate automatically on the 1st of each month; you can
                  also request one now.
                </p>
              </Card>
            ) : (
              <ul className="space-y-1">
                {reports.map((r) => (
                  <li key={r.id}>
                    <ReportListItem
                      report={r}
                      isActive={active?.id === r.id}
                      onClick={() => setActiveId(r.id)}
                    />
                  </li>
                ))}
              </ul>
            )}
          </aside>

          {/* Main pane */}
          <main>
            {!active ? (
              <Card className="p-8 text-center">
                <p className="text-muted-foreground text-sm">
                  Select a report from the list to read it.
                </p>
              </Card>
            ) : (
              <ReportDetail report={active} />
            )}
          </main>
        </div>
      </PageContent>
    </Page>
  );
}

function ReportListItem({
  report,
  isActive,
  onClick,
}: {
  report: MonthlyReport;
  isActive: boolean;
  onClick: () => void;
}) {
  const label = format(parseISO(report.periodStart), "MMM yyyy");
  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full rounded-md px-3 py-2 text-left transition-colors ${
        isActive ? "bg-secondary" : "hover:bg-muted"
      }`}
    >
      <p className="text-sm font-medium">{label}</p>
      <p className="text-muted-foreground text-[11px]">
        {report.status === "succeeded"
          ? "Ready"
          : report.status === "pending"
            ? "Generating…"
            : "Failed"}
      </p>
    </button>
  );
}

function ReportDetail({ report }: { report: MonthlyReport }) {
  const periodLabel = format(parseISO(report.periodStart), "MMMM yyyy");

  if (report.status === "pending") {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{periodLabel}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <Skeleton className="h-5 w-1/2" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-4/5" />
          <Skeleton className="h-4 w-3/4" />
          <p className="text-muted-foreground pt-2 text-xs">
            <Icons.Spinner className="mr-1 inline-block h-3 w-3 animate-spin" />
            Generating your report — usually 10–30 seconds.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (report.status === "failed") {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{periodLabel}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-destructive text-sm">Report generation failed.</p>
          {report.error && <p className="text-muted-foreground mt-2 text-xs">{report.error}</p>}
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardContent className="pt-6">
        <article className="prose prose-sm dark:prose-invert max-w-none">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>
            {report.summaryMd ?? "_(No content)_"}
          </ReactMarkdown>
        </article>
        {report.generatedAt && (
          <p className="text-muted-foreground border-border mt-4 border-t pt-3 text-[11px]">
            Generated {format(parseISO(report.generatedAt), "PPp")}
            {report.model ? ` · ${report.model}` : ""}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
