/**
 * Equities panel — Track B PR-B2 / Goal v3 §V Phase 5 step B2.
 *
 * Per Goal v3 §V Phase 5: "Donut by sub-class (Stocks / ETFs /
 * Mutual Funds). Bar by geographic exposure."
 *
 * Composition mirrors the Sukuks panel (PR-B1 #117):
 *   - Header: panel title + total equity exposure + count
 *   - Donut chart of allocation by sub-class
 *   - Bar chart of exposure by region
 *   - Holdings list (tap → asset detail)
 *
 * Reads from `useHoldings(PORTFOLIO_ACCOUNT_ID)` and the existing
 * `<Donut />` / `<Bar />` primitives shipped in PR-A3 (#70).
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
  isEquityHolding,
  rollupByRegion,
  rollupBySubclass,
  totalEquityExposure,
} from "./rollup";

export default function EquitiesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const equityHoldings = useMemo(
    () => (allHoldings ?? []).filter(isEquityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalEquityExposure(allHoldings ?? []),
    [allHoldings],
  );
  const subclassRows = useMemo(
    () => rollupBySubclass(allHoldings ?? []),
    [allHoldings],
  );
  const regionRows = useMemo(() => rollupByRegion(allHoldings ?? []), [allHoldings]);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.TrendingUp className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Equities</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {equityHoldings.length === 0 && !isLoading
            ? "No equity holdings yet."
            : `Total exposure ${formatAmount(totalExposure, baseCurrency)} across ${equityHoldings.length} ${equityHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {equityHoldings.length > 0 && (
        <>
          <section
            aria-label="Equity sub-class allocation"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By sub-class</div>
            {subclassRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No allocation data available.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={subclassRows}
                  ariaLabel="Equity allocation by sub-class"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Equity geographic exposure"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By region</div>
            {regionRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No region data on these holdings — classify them in the holding detail.
              </div>
            ) : (
              <div className="h-64">
                <Bar
                  data={regionRows.map((r) => ({
                    label: r.region,
                    value: r.exposureBase,
                  }))}
                  ariaLabel="Equity exposure by region"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Equity holdings"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Holdings</h2>
            </header>
            <ul role="list" className="divide-y">
              {equityHoldings.map((h) => {
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
