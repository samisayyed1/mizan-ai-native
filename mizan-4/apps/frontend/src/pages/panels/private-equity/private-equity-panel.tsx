/**
 * Private Equity panel — Track B PR-B7 / Goal v3 §V Phase 5.
 *
 * §23 reference user has Hasan VC + HAPL + Wholesum + Stake PE
 * positions; the vintage bar surfaces year-of-acquisition
 * laddering. The J-curve projection overlay lands separately as
 * PR-B7.a once `crates/synthesis` exposes its API to the frontend.
 *
 * Composition mirrors PR-B11 / PR-B15 (the other "per-item" panels):
 *   - Header: total PE + position count
 *   - Bar chart by vintage year (ascending)
 *   - Bar chart per position (descending by value)
 *   - Positions list with vintage chip
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { Bar } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isPrivateEquityHolding,
  rollupByPosition,
  rollupByVintage,
  totalPrivateEquityExposure,
} from "./rollup";

export default function PrivateEquityPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const peHoldings = useMemo(
    () => (allHoldings ?? []).filter(isPrivateEquityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalPrivateEquityExposure(allHoldings ?? []),
    [allHoldings],
  );
  const vintageRows = useMemo(
    () => rollupByVintage(allHoldings ?? []),
    [allHoldings],
  );
  const positionRows = useMemo(
    () => rollupByPosition(allHoldings ?? []),
    [allHoldings],
  );

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Building className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Private Equity</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {peHoldings.length === 0 && !isLoading
            ? "No private-equity positions yet."
            : `Total PE ${formatAmount(totalExposure, baseCurrency)} across ${peHoldings.length} ${peHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {peHoldings.length > 0 && (
        <>
          <section
            aria-label="Private equity by vintage"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By vintage year</div>
            {vintageRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No vintage data — add a purchase date in the position detail.
              </div>
            ) : (
              <div className="h-64">
                <Bar
                  data={vintageRows.map((r) => ({
                    label: String(r.year),
                    value: r.exposureBase,
                  }))}
                  ariaLabel="Private equity exposure by vintage year"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Private equity per position"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">Per position</div>
            <div className="h-64">
              <Bar
                data={positionRows.map((r) => ({ label: r.label, value: r.value }))}
                ariaLabel="Private equity market value per position"
                palette="categorical"
              />
            </div>
          </section>

          <section
            aria-label="Private equity positions"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Positions</h2>
            </header>
            <ul role="list" className="divide-y">
              {positionRows.map((p) => (
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
                    <div className="text-muted-foreground text-xs">
                      {p.vintage !== null ? `Vintage ${p.vintage}` : "Vintage —"}
                    </div>
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
