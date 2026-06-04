/**
 * Collectibles panel — Track B PR-B15 / Goal v3 §V Phase 5.
 *
 * Composition mirrors PR-B11 Real Estate (the other "per-item"
 * panel where holdings don't aggregate):
 *   - Header: total collectibles + item count
 *   - Donut by category (Watches / Art / Jewelry / Wine /
 *     Memorabilia / Other)
 *   - Per-item bar (one bar per holding)
 *   - Item list (tap-row → holding detail) with category chip
 *
 * AI estimation pipeline (Chrono24 / WatchCharts / Hagerty /
 * StockX) + `'ai-estimated'` badge wiring ships separately as
 * PR-B15.b.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { Bar, Donut } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isCollectibleHolding,
  rollupByCategory,
  rollupByItem,
  totalCollectiblesExposure,
} from "./rollup";

export default function CollectiblesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const collectibleHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCollectibleHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCollectiblesExposure(allHoldings ?? []),
    [allHoldings],
  );
  const categoryRows = useMemo(
    () => rollupByCategory(allHoldings ?? []),
    [allHoldings],
  );
  const itemRows = useMemo(() => rollupByItem(allHoldings ?? []), [allHoldings]);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Sparkles className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Collectibles</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {collectibleHoldings.length === 0 && !isLoading
            ? "No collectibles yet."
            : `Total collectibles ${formatAmount(totalExposure, baseCurrency)} across ${collectibleHoldings.length} ${collectibleHoldings.length === 1 ? "item" : "items"}.`}
        </div>
      </header>

      {collectibleHoldings.length > 0 && (
        <>
          <section
            aria-label="Collectibles by category"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By category</div>
            {categoryRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No category data — add a collectible type (Watch /
                Art / Jewelry / Wine / Memorabilia) in the item detail.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={categoryRows.map((r) => ({ label: r.category, value: r.value }))}
                  ariaLabel="Collectibles allocation by category"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Collectibles per item"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">Per item</div>
            <div className="h-64">
              <Bar
                data={itemRows.map((r) => ({ label: r.label, value: r.value }))}
                ariaLabel="Collectibles market value per item"
                palette="categorical"
              />
            </div>
          </section>

          <section
            aria-label="Collectibles list"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Items</h2>
            </header>
            <ul role="list" className="divide-y">
              {itemRows.map((p) => (
                <li
                  key={p.holdingId}
                  className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                >
                  <button
                    type="button"
                    onClick={() => navigate(`/holdings/${p.holdingId}`)}
                    className="text-foreground flex-1 text-left text-sm font-medium"
                  >
                    <div>{p.label}</div>
                    <div className="text-muted-foreground text-xs">{p.category}</div>
                  </button>
                  <div className="text-foreground text-sm font-semibold tabular-nums">
                    {formatAmount(p.value, baseCurrency)}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        </>
      )}

      <div className="pt-2">
        <Button variant="outline" size="sm" onClick={() => navigate("/")}>
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          Back to dashboard
        </Button>
      </div>
    </div>
  );
}
