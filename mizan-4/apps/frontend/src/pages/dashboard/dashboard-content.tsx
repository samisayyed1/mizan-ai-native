import { useDocumentTitle } from "@/hooks/use-document-title";
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
 * Dashboard composition per ADR 0018 / 0018b·v3 (Dashboard IA).
 *
 * Top-to-bottom order on the main column (data-first, no AI input on the
 * hero — the number is what the eye should land on, the AI sits with the
 * other today-context surfaces in the sidebar):
 *   (a) Net Worth strip — single big number + toggleable deltas + sparkline
 *   (b) Heatmap — every holding as a tile sized by USD value, colored by perf
 *   (c) Asset class panel grid — 12 panels in fixed order from taxonomy.ts
 *
 * Right sidebar (lg+ only), top to bottom (time-sensitivity descends):
 *   1. News
 *   2. Today's Signal / Heads Up
 *   3. Ask Mizan (AI command bar)
 *   4. Zakat
 *   5. Portfolio Health
 *   6. Goals
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
  // Brand presence in OS chrome — tab title reads "Mizan — Dashboard"
  // so the dock / taskbar / overflow tab list keeps Mizan recognisable.
  useDocumentTitle("Dashboard");

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
      {/* PR-DENSITY-2 / PR-DENSITY-7 — strict 8-pt rhythm:
       *   page edge padding: 16 mobile / 24 desktop
       *   main↔sidebar gap: 24 (was 32-48 — sidebar shouldn't feel like
       *       a separate page, it's a companion column)
       *   between-card gap on main column: 16 (was 24)
       *
       * The tightened rhythm + the 72px tiles means the hero strip
       * (AI bar + Net Worth + Heatmap + first row of tiles) fits in
       * a ~720px viewport without scroll — the user sees the
       * Singapore-millionaire dashboard in one glance. */}
      <div className="grow px-4 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] pt-4 md:px-6 md:pb-6 md:pt-6 lg:px-8 lg:pb-8 lg:pt-8">
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-3 lg:gap-6">
          <div className="space-y-4 lg:col-span-2">
            {/* ADR 0018b·v3 (a) — Net Worth strip.
                Single large number in base currency, toggleable deltas
                (24h / 7d / 30d / YTD / All) as chips, sparkline below.
                Now the hero of the main column — the AI command bar
                moved to the sidebar so the dashboard opens on data,
                not an input field. Tap → /net-worth detail page. */}
            <NetWorthStrip
              history={valuationHistory}
              baseCurrency={baseCurrency}
              isLoading={isHistoryLoading}
              defaultWindow="30d"
            />

            {/* ADR 0018b·v2 (c) — Heatmap.
                Every holding as a treemap tile, sized by USD value,
                colored by 24h performance. Sits ABOVE the asset class
                grid: the heatmap is the "big picture" beat (where the
                money lives + how it moved), the tiles below are the
                taxonomy / navigation layer the user drills into. */}
            <HoldingsHeatmap
              holdings={allHoldings ?? []}
              isLoading={isHoldingsLoading}
              baseCurrency={baseCurrency}
            />

            {/* ADR 0018b·v2 (d) — Asset class panel grid.
                Twelve panels in fixed order from
                `@/components/asset-class-panels/taxonomy.ts`. Tiles
                serve as the navigation layer; tap any → /panels/{id}. */}
            <AssetClassPanelGrid
              holdings={allHoldings ?? []}
              baseCurrency={baseCurrency}
            />

            {/* NOTE: Today's Signal card (Heads Up) moved out of the
                main column into the right sidebar (between News and
                Zakat) per founder spec — actionable signals live
                alongside the other time-sensitive surfaces, not as a
                trailing main-column afterthought. */}
          </div>

          {/* Right sidebar (lg+) — ADR 0018b·v3 composition.
              Order is descending time-sensitivity / action-ability:
                1. News        — daily-fresh, pulls the eye to the column
                2. Heads Up    — today's highest-severity actionable signal
                3. Ask Mizan   — AI command bar (sits with the other today
                                 surfaces — query is a today action)
                4. Zakat       — Mizan's unique value, daily reminder
                5. Health      — running portfolio insight
                6. Goals       — monthly/yearly checkpoints, fine at bottom

              No section header — the sidebar IS today by implication.
              On narrow viewports (<lg) it reflows inline below the
              panel grid. */}
          <aside
            aria-label="Dashboard sidebar"
            className="space-y-3 lg:col-span-1 lg:sticky lg:top-4 lg:self-start"
          >
            <NewsHomeWidget />
            <TodaysSignalCard
              notifications={notificationsPage?.items}
              isLoading={isNotificationsLoading}
            />
            {/* AI command bar — non-pinned (sidebar is already sticky),
                shorter placeholder for the narrower column. */}
            <AiCommandBar pinned={false} placeholder="Ask Mizan…" />
            <ZakatCard />
            <PortfolioHealthCard />
            <SavingGoals />
          </aside>
        </div>
      </div>
    </div>
  );
}
