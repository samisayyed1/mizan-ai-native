/**
 * Commodities panel — Track B PR-B10 / Goal v3 §V Phase 5.
 *
 * Composition mirrors PR-B1/B2/B3/B9/B11:
 *   - Header: total commodities + position count
 *   - Donut by metal (Gold / Silver / Platinum / Palladium / Other)
 *   - Holdings list (tap → asset detail)
 *
 * The physical vs paper Mizan Badge modifier (per ADR 0023) lands
 * separately once the badge stack threading is verified.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { Donut } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isCommodityHolding,
  rollupByMetal,
  totalCommoditiesExposure,
} from "./rollup";

export default function CommoditiesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const commodityHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCommodityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCommoditiesExposure(allHoldings ?? []),
    [allHoldings],
  );
  const metalRows = useMemo(() => rollupByMetal(allHoldings ?? []), [allHoldings]);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Gem className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Commodities</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {commodityHoldings.length === 0 && !isLoading
            ? "No commodities holdings yet."
            : `Total commodities ${formatAmount(totalExposure, baseCurrency)} across ${commodityHoldings.length} ${commodityHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {commodityHoldings.length > 0 && (
        <>
          <section
            aria-label="Commodities by metal"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By metal</div>
            {metalRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No allocation data available.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={metalRows.map((r) => ({
                    label: r.metal,
                    value: r.exposureBase,
                  }))}
                  ariaLabel="Commodities allocation by metal"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Commodities holdings"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Holdings</h2>
            </header>
            <ul role="list" className="divide-y">
              {commodityHoldings.map((h) => {
                const label = h.instrument?.name ?? h.instrument?.symbol ?? h.id;
                return (
                  <li
                    key={h.id}
                    className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                  >
                    <button
                      type="button"
                      onClick={() => navigate(`/holdings/${h.instrument?.id ?? h.id}`)}
                      className="text-foreground flex-1 text-left text-sm font-medium"
                    >
                      {label}
                    </button>
                    <div className="text-foreground text-sm font-semibold tabular-nums">
                      {formatAmount(h.marketValue?.base ?? 0, baseCurrency)}
                    </div>
                  </li>
                );
              })}
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
