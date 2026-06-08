import { PageErrorBoundary } from "@/components/page-error-boundary";
import { useDocumentTitle } from "@/hooks/use-document-title";
import { useAddAsset } from "@/features/add-asset";
import { useBalancePrivacy } from "@/hooks/use-balance-privacy";
import { useSettingsContext } from "@/lib/settings-provider";
import type { Goal } from "@/lib/types";
import {
  Button,
  Page,
  PageContent,
  PageHeader,
  Skeleton,
  formatCompactAmount,
  formatPercent,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useState } from "react";
import { Link } from "react-router-dom";
import { GoalCard } from "../components/goal-card";
import { useGoals } from "../hooks/use-goals";

function StatBlock({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-2.5">
      <span className="text-muted-foreground text-[10px] tracking-[0.15em]">{label}</span>
      <span className="text-foreground font-serif text-[15px] font-semibold tabular-nums">
        {value}
      </span>
    </div>
  );
}

function SummaryStats({ goals }: { goals: Goal[] }) {
  const { isBalanceHidden } = useBalancePrivacy();
  const { settings } = useSettingsContext();
  // Defensive null guards: goals[0] may be undefined if the array is
  // empty; settings can be loading; currency falls back to USD so
  // formatCompactAmount never receives undefined.
  const currency = settings?.baseCurrency ?? goals[0]?.currency ?? "USD";

  // Coerce each summary field through Number() with a NaN-safe fallback.
  // A malformed goal row in the database (string in a number column,
  // missing field, null where 0 expected) would otherwise propagate
  // NaN through the reduce and render "$NaN" / "NaN%" — both of which
  // are worse than 0.
  const toFinite = (v: unknown): number => {
    const n = typeof v === "number" ? v : Number(v);
    return Number.isFinite(n) ? n : 0;
  };

  const saved = goals.reduce((s, g) => s + toFinite(g.summaryCurrentValue), 0);
  const target = goals.reduce(
    (s, g) => s + toFinite(g.summaryTargetAmount ?? g.targetAmount),
    0,
  );
  // Guard against 0/0 = NaN; render 0% instead.
  const overall = target > 0 ? saved / target : 0;
  const onTrackCount = goals.filter(
    (g) => g.statusHealth === "on_track" || g.statusLifecycle === "achieved",
  ).length;

  const savedDisplay = isBalanceHidden ? "••••" : formatCompactAmount(saved, currency);
  const targetDisplay = isBalanceHidden ? "••••" : formatCompactAmount(target, currency);

  return (
    <div className="mb-6 flex flex-wrap items-baseline gap-x-8 gap-y-2">
      <StatBlock label="SAVED" value={savedDisplay} />
      <StatBlock label="TARGET" value={targetDisplay} />
      <StatBlock label="OVERALL" value={formatPercent(overall)} />
      <StatBlock label="ON TRACK" value={`${onTrackCount}/${goals.length}`} />
    </div>
  );
}

function GoalGrid({ goals }: { goals: Goal[] }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {goals.map((goal) => (
        <GoalCard key={goal.id} goal={goal} />
      ))}
    </div>
  );
}

export default function GoalsDashboardPage() {
  return (
    <PageErrorBoundary pageName="Goals">
      <GoalsDashboardContent />
    </PageErrorBoundary>
  );
}

function GoalsDashboardContent() {
  useDocumentTitle("Goals");
  const { active, atRisk, achieved, archived, isLoading } = useGoals();
  const [archivedOpen, setArchivedOpen] = useState(false);
  const addAsset = useAddAsset();

  if (isLoading) {
    return (
      <Page>
        <PageHeader heading="Goals" text="Track and plan your financial goals" />
        <PageContent>
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {[1, 2, 3].map((i) => (
              <Skeleton key={i} className="h-64 w-full rounded-xl" />
            ))}
          </div>
        </PageContent>
      </Page>
    );
  }

  // Merge all non-archived goals into one flat list, sorted by target amount DESC.
  const allActive = [...atRisk, ...active, ...achieved].sort((a, b) => {
    const aTarget = a.summaryTargetAmount ?? a.targetAmount ?? 0;
    const bTarget = b.summaryTargetAmount ?? b.targetAmount ?? 0;
    if (aTarget !== bTarget) return bTarget - aTarget;
    return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
  });
  const hasGoals = allActive.length > 0 || archived.length > 0;

  return (
    <Page>
      <PageHeader
        heading="Goals"
        text="Track and plan your financial goals"
        actions={
          <Link to="/goals/new">
            <Button size="sm">
              <Icons.Plus className="mr-1 h-4 w-4" />
              New Goal
            </Button>
          </Link>
        }
      />
      <PageContent>
        {!hasGoals ? (
          // Empty state mirrors the Dashboard + Holdings heroes — lead
          // with the AI agent (GoalPlanner recipe) and demote the
          // manual form to a secondary link. Most users describing a
          // goal ("retire at 55 with $2M") want the agent to compute
          // the savings rate + glidepath for them, not to fill in a
          // multi-field form by hand.
          <div className="border-border/60 from-background to-muted/30 relative mx-auto my-12 max-w-2xl overflow-hidden rounded-2xl border bg-gradient-to-b p-8 text-center md:p-12">
            <div className="pointer-events-none absolute inset-0 -z-10 flex items-center justify-center opacity-[0.04]">
              <Icons.Target className="size-64" />
            </div>
            <div className="mx-auto inline-flex h-12 w-12 items-center justify-center rounded-full bg-amber-500/15 text-amber-600 dark:text-amber-300">
              <Icons.Sparkles className="size-6" />
            </div>
            <h3 className="text-foreground mt-4 text-lg font-semibold">
              Plan your first goal
            </h3>
            <p className="text-muted-foreground mx-auto mt-2 max-w-md text-sm leading-relaxed">
              Tell Mizan what you're saving toward — retirement, a home, a
              wedding, kids' education — and the agent computes the monthly
              contribution needed plus a realistic glidepath against your
              current portfolio.
            </p>
            <Button
              size="lg"
              className="mt-6"
              onClick={() =>
                addAsset.open({
                  source: "portfolio",
                  prompt: "Help me plan a financial goal.",
                })
              }
            >
              <Icons.Sparkles className="mr-2 h-4 w-4" />
              Plan with Mizan
            </Button>
            <div className="mt-3">
              <Link
                to="/goals/new"
                className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
              >
                or create one manually
              </Link>
            </div>
          </div>
        ) : (
          <div className="space-y-8">
            {allActive.length > 0 && (
              <div>
                <SummaryStats goals={allActive} />
                <GoalGrid goals={allActive} />
              </div>
            )}

            {archived.length > 0 && (
              <section>
                <button
                  type="button"
                  onClick={() => setArchivedOpen((o) => !o)}
                  className="text-muted-foreground hover:text-foreground flex items-center gap-2 text-xs font-medium transition-colors"
                >
                  <Icons.ChevronRight
                    className={`h-3.5 w-3.5 transition-transform ${archivedOpen ? "rotate-90" : ""}`}
                  />
                  Archived ({archived.length})
                </button>
                {archivedOpen && (
                  <div className="mt-3">
                    <GoalGrid goals={archived} />
                  </div>
                )}
              </section>
            )}
          </div>
        )}
      </PageContent>
    </Page>
  );
}
