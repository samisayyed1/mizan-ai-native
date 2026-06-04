/**
 * Forex panel — Track B PR-B16 / Goal v3 §V Phase 5.
 *
 * Composition mirrors PR-B1/B2/B3/B9/B10/B11:
 *   - Header: total FX exposure + position count
 *   - Bar chart: per-pair exposure (one bar per holding)
 *   - Donut: per long-currency exposure (the "from" leg of each pair)
 *   - Holdings list (tap → asset detail)
 *
 * Per-pair histogram (price-history overlay) lands separately as
 * PR-B16.a alongside the FX history hydration shipped in
 * PR-C5.d.4 (#114).
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
  isFxHolding,
  rollupByLongLeg,
  rollupByPair,
  totalFxExposure,
} from "./rollup";

export default function ForexPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const fxHoldings = useMemo(
    () => (allHoldings ?? []).filter(isFxHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalFxExposure(allHoldings ?? []),
    [allHoldings],
  );
  const pairRows = useMemo(() => rollupByPair(allHoldings ?? []), [allHoldings]);
  const longLegRows = useMemo(
    () => rollupByLongLeg(allHoldings ?? []),
    [allHoldings],
  );

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.RefreshCw className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Forex</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {fxHoldings.length === 0 && !isLoading
            ? "No FX positions yet."
            : `Total FX exposure ${formatAmount(totalExposure, baseCurrency)} across ${fxHoldings.length} ${fxHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {fxHoldings.length > 0 && (
        <>
          <section
            aria-label="FX exposure per pair"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">Per pair</div>
            <div className="h-64">
              <Bar
                data={pairRows.map((r) => ({ label: r.label, value: r.exposureBase }))}
                ariaLabel="FX exposure per pair"
                palette="categorical"
              />
            </div>
          </section>

          <section
            aria-label="FX exposure by long currency"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By long currency</div>
            {longLegRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No long-leg data — instrument symbols don&apos;t match a recognised pair format.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={longLegRows.map((r) => ({
                    label: r.currency,
                    value: r.exposureBase,
                  }))}
                  ariaLabel="FX exposure by long currency"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="FX positions"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Positions</h2>
            </header>
            <ul role="list" className="divide-y">
              {pairRows.map((p) => (
                <li
                  key={p.holdingId}
                  className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                >
                  <button
                    type="button"
                    onClick={() => navigate(`/holdings/${p.holdingId}`)}
                    className="text-foreground flex-1 text-left text-sm font-medium"
                  >
                    {p.label}
                  </button>
                  <div className="text-foreground text-sm font-semibold tabular-nums">
                    {formatAmount(p.exposureBase, baseCurrency)}
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
