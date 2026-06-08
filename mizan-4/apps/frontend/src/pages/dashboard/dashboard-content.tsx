import { useHoldings } from "@/hooks/use-holdings";
import { useValuationHistory } from "@/hooks/use-valuation-history";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { useQuery } from "@tanstack/react-query";
import { HoldingsHeatmap } from "./holdings-heatmap";
import { NewsHomeWidget } from "./news-home-widget";
import { PortfolioHealthCard } from "./portfolio-health-card";
import { ZakatCard } from "./zakat-card";
import SavingGoals from "./goals";
import { AssetClassPanelGrid } from "@/components/asset-class-panels";
import { NetWorthStrip } from "@/components/dashboard/net-worth-strip";
import { AiCommandBar } from "@/components/dashboard/ai-command-bar";
import { TodaysSignalCard } from "@/components/dashboard/todays-signal-card";
import { listNotifications } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

/**
 * Dashboard composition per ADR 0018 (Dashboard Information Architecture).
 *
 * Top-to-bottom order on the main column:
 *   (a) AI command bar — pinned, full-width, persistent input
 *   (b) Net Worth strip — single big number + toggleable deltas + sparkline
 *   (c) Heatmap — every holding as a tile sized by USD value, colored by perf
 *   (d) Asset class panel grid — 12 panels in fixed order from taxonomy.ts
 *   (e) Today's Signal card — highest-signal AI insight for today
 *
 * Right sidebar (lg+ only): Goals, Portfolio Health, Zakat, News.
 *
 * What this composition deliberately removes vs the prior shipped layout:
 * - TickerConveyor (was at the very top — too noisy, distracts from totals)
 * - Inline Balance + gain block (Net Worth strip is the canonical surface)
 * - HistoryChart + IntervalSelector + "estimate full history" toggle
 *   (those belong on the Net Worth detail page, not the dashboard)
 * - AccountsSummary (was rendering connected accounts as a top-level surface;
 *   accounts now live INSIDE the Brokerage / Bank Cash panels per ADR 0018's
 *   "user thinks in asset classes" model)
 * - PortfolioUpdateTrigger HoverCard (manual recalc affordance moves to
 *   the Net Worth detail page; the dashboard auto-refreshes)
 */
export function DashboardContent() {
  const { holdings: allHoldings, isLoading: isHoldingsLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);

  // Today's Signal feed — same source as the bell, capped at 25 newest.
  // Selection algorithm in todays-signal-card picks today's highest-
  // severity unread; an empty feed renders the "quiet" empty state.
  const { data: notificationsPage, isLoading: isNotificationsLoading } = useQuery({
    queryKey: QueryKeys.notifications(25),
    queryFn: () => listNotifications(25),
    staleTime: 60_000,
  });

  // All-time valuation history powers the Net Worth strip's sparkline +
  // delta windows (24h / 7d / 30d / YTD / All). Passing `undefined` gets
  // the full extent; the strip computes window deltas client-side.
  const { valuationHistory, isLoading: isHistoryLoading } = useValuationHistory(undefined);

  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  return (
    // PR-POLISH-4 — depth-page background ladder. Dark mode shifts to
    // a pure-grayscale tier (#0A) so cards above visually rise off
    // the page. Light mode stays on the Flexoki paper tone.
    <div
      className="flex min-h-screen flex-col"
      style={{ background: "var(--depth-page)" }}
    >
      <div className="grow px-4 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] pt-6 md:px-6 md:pb-6 md:pt-8 lg:px-10 lg:pb-8 lg:pt-10">
        <div className="grid grid-cols-1 gap-8 lg:grid-cols-3 lg:gap-12">
          <div className="space-y-6 lg:col-span-2">
            {/* ADR 0018 (a) — AI Command Bar.
                Pinned full-width input. Submit → /assistant with the
                prompt pre-seeded. Voice button routes to dictation. */}
            <AiCommandBar />

            {/* ADR 0018 (b) — Net Worth strip.
                Single large number in base currency, toggleable deltas
                (24h / 7d / 30d / YTD / All) as chips, sparkline below.
                Tap → /net-worth detail page. */}
            <NetWorthStrip
              history={valuationHistory}
              baseCurrency={baseCurrency}
              isLoading={isHistoryLoading}
              defaultWindow="30d"
            />

            {/* ADR 0018 (c) — Heatmap.
                Every holding as a treemap tile, sized by USD value,
                colored by 24h performance. Tap → asset detail. */}
            <HoldingsHeatmap
              holdings={allHoldings ?? []}
              isLoading={isHoldingsLoading}
              baseCurrency={baseCurrency}
            />

            {/* ADR 0018 (d) — Asset class panel grid.
                Twelve panels in fixed order from
                `@/components/asset-class-panels/taxonomy.ts`.
                Each tile: name, total value, 24h/30d delta, sparkline,
                chevron. Tap → /panels/{panelId} detail view. */}
            <AssetClassPanelGrid
              holdings={allHoldings ?? []}
              baseCurrency={baseCurrency}
            />

            {/* ADR 0018 (e) — Today's Signal card.
                One card, highest-severity unread from today's
                deterministic insights output. Severity-themed chrome;
                deep-link tap → route, no-link tap → expand reasoning. */}
            <TodaysSignalCard
              notifications={notificationsPage?.items}
              isLoading={isNotificationsLoading}
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
  );
}
