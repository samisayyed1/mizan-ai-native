/**
 * Sukuks panel — Track B PR-B1 / Goal v3 §V Phase 5 / §23 anchor surface.
 *
 * The §23 reference scenario:
 *   > "He taps the Sukuks panel, sees a bar chart by issuer
 *   > (concentration view: Emaar X%, Dar al Arkan Y%, Sobha Z%)
 *   > toggleable to maturity year (laddering view)."
 *
 * Composition:
 *   - Header strip: panel title + total bond/sukuk exposure
 *   - Bar chart with toggle between "By issuer" (default) and
 *     "By maturity year"
 *   - Holdings list below the chart (one row per bond/sukuk)
 *
 * The bar chart consumes the `<Bar />` primitive shipped in PR-A3
 * (#70) which already meets the §A19 <200ms cached-paint budget.
 *
 * Hooked from the dashboard's twelve-panel grid (PR-A4 #94) via the
 * `/panels/sukuks` route; the grid's `holdingsHref` for the
 * `bonds-sukuks` descriptor is repointed to this page in this PR.
 */
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { AssetClassPanelLayout } from "@/components/panels/asset-class-panel-layout";
import { Bar } from "@/components/charts";
import type { BarDatum } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isBondHolding,
  rollupByIssuer,
  rollupByMaturityYear,
  totalBondExposure,
} from "./rollup";

type ChartMode = "issuer" | "maturity";

export default function SukuksPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [chartMode, setChartMode] = useState<ChartMode>("issuer");

  const bondHoldings = useMemo(
    () => (allHoldings ?? []).filter(isBondHolding),
    [allHoldings],
  );

  const totalExposure = useMemo(
    () => totalBondExposure(allHoldings ?? []),
    [allHoldings],
  );

  const byIssuer = useMemo(() => rollupByIssuer(allHoldings ?? []), [allHoldings]);
  const byMaturity = useMemo(
    () => rollupByMaturityYear(allHoldings ?? []),
    [allHoldings],
  );

  // Compute the active bar dataset based on the toggle. Both shapes
  // map to `BarDatum` ({ label, value }) — same primitive renders both.
  const barData: BarDatum[] = useMemo(() => {
    if (chartMode === "issuer") {
      return byIssuer.map((r) => ({ label: r.issuer, value: r.exposureBase }));
    }
    return byMaturity.map((r) => ({
      label: String(r.year),
      value: r.exposureBase,
    }));
  }, [chartMode, byIssuer, byMaturity]);

  const summary =
    bondHoldings.length === 0 && !isLoading
      ? "No bond or sukuk holdings yet."
      : `Total exposure ${formatAmount(totalExposure, baseCurrency)} across ${bondHoldings.length} ${bondHoldings.length === 1 ? "position" : "positions"}.`;

  const chart =
    bondHoldings.length > 0 ? (
      <section
        aria-label="Bond and sukuk allocation"
        className="bg-card rounded-2xl border p-4"
      >
        <div className="mb-3 flex items-center justify-between">
          <div className="text-sm font-medium">
            {chartMode === "issuer" ? "By issuer" : "By maturity year"}
          </div>
          <div
            role="tablist"
            aria-label="Bar chart view"
            className="bg-muted/40 inline-flex rounded-full p-1"
          >
            <ToggleButton
              active={chartMode === "issuer"}
              onClick={() => setChartMode("issuer")}
              label="Issuer"
            />
            <ToggleButton
              active={chartMode === "maturity"}
              onClick={() => setChartMode("maturity")}
              label="Maturity"
            />
          </div>
        </div>
        {barData.length === 0 ? (
          <div className="text-muted-foreground py-12 text-center text-sm">
            {chartMode === "maturity"
              ? "No bond maturities recorded yet — add maturity dates in the holding detail."
              : "No allocation data available."}
          </div>
        ) : (
          <div className="h-64">
            <Bar
              data={barData}
              ariaLabel={`Bond and sukuk allocation ${chartMode === "issuer" ? "by issuer" : "by maturity year"}`}
              palette="categorical"
            />
          </div>
        )}
      </section>
    ) : null;

  const list =
    bondHoldings.length > 0 ? (
      <section
        aria-label="Bond and sukuk holdings"
        className="bg-card rounded-2xl border"
      >
        <header className="border-b px-4 py-3">
          <h2 className="text-sm font-medium">Holdings</h2>
        </header>
        <ul role="list" className="divide-y">
          {bondHoldings.map((h) => {
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
    ) : null;

  return (
    <AssetClassPanelLayout
      title="Bonds & Sukuks"
      IconComponent={Icons.Coins}
      summary={summary}
      chart={chart}
      list={list}
    />
  );
}

function ToggleButton({
  active,
  onClick,
  label,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`rounded-full px-3 py-1 text-xs font-medium transition-colors ${
        active
          ? "bg-background text-foreground shadow-sm"
          : "text-muted-foreground hover:text-foreground"
      }`}
    >
      {label}
    </button>
  );
}
