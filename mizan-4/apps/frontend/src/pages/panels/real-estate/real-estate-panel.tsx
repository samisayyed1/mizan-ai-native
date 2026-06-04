/**
 * Real Estate panel — Track B PR-B11 / Goal v3 §V Phase 5.
 *
 * §23 reference user's real estate composition is the most
 * Zakat-narrative-rich panel: Bukit Batok primary residence (Not
 * Zakatable), 3 Hyderabad rentals (Zakatable on net rental income
 * through Hawl), 1 Hyderabad held-for-sale (Zakatable on market
 * value, AI-estimated against Dubai Land Department comparables).
 *
 * Composition mirrors PR-B1 / PR-B2 / PR-B3:
 *   - Header: total real estate + property count
 *   - Donut by intent (Residence / Rental / Land / Commercial)
 *   - Bar per property (one bar per holding)
 *   - Property list (tap-row → holding detail)
 *
 * Stacked equity-over-mortgage bar lands in PR-B11.a once the
 * mortgage-link wire ships.
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
  isRealEstateHolding,
  rollupByIntent,
  rollupByProperty,
  totalRealEstateExposure,
} from "./rollup";

export default function RealEstatePanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const propertyHoldings = useMemo(
    () => (allHoldings ?? []).filter(isRealEstateHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalRealEstateExposure(allHoldings ?? []),
    [allHoldings],
  );
  const intentRows = useMemo(
    () => rollupByIntent(allHoldings ?? []),
    [allHoldings],
  );
  const propertyRows = useMemo(
    () => rollupByProperty(allHoldings ?? []),
    [allHoldings],
  );

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Home className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Real Estate</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {propertyHoldings.length === 0 && !isLoading
            ? "No real-estate holdings yet."
            : `Total real estate ${formatAmount(totalExposure, baseCurrency)} across ${propertyHoldings.length} ${propertyHoldings.length === 1 ? "property" : "properties"}.`}
        </div>
      </header>

      {propertyHoldings.length > 0 && (
        <>
          <section
            aria-label="Real estate by intent"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By intent</div>
            {intentRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No intent classification on these properties — add a
                property type (Residence / Rental / Land / Commercial)
                in the property detail.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={intentRows.map((r) => ({ label: r.intent, value: r.value }))}
                  ariaLabel="Real estate allocation by intent"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Real estate per property"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">Per property</div>
            <div className="h-64">
              <Bar
                data={propertyRows.map((r) => ({ label: r.label, value: r.value }))}
                ariaLabel="Real estate market value per property"
                palette="categorical"
              />
            </div>
          </section>

          <section
            aria-label="Real estate holdings"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Properties</h2>
            </header>
            <ul role="list" className="divide-y">
              {propertyRows.map((p) => (
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
                    <div className="text-muted-foreground text-xs">{p.intent}</div>
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
