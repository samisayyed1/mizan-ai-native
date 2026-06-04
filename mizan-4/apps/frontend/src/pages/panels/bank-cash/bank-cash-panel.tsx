/**
 * Bank & Cash panel — Track B PR-B3 / Goal v3 §V Phase 5 step B3.
 *
 * Composition mirrors PR-B1 (Sukuks) / PR-B2 (Equities):
 *   - Header: panel title + total cash + count
 *   - Donut with toggle between "By country" (default) and "By currency"
 *   - Holdings list (tap → asset detail)
 *
 * The §23 reference user has 12 cash accounts across SG / IN / KSA /
 * UAE. The country donut surfaces geographic concentration; the
 * currency donut surfaces FX exposure (complements the FX
 * insights rule from PR-C5.a #108).
 */
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { Donut } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isCashHolding,
  rollupByCountry,
  rollupByCurrency,
  totalCashExposure,
} from "./rollup";

type DonutMode = "country" | "currency";

export default function BankCashPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [mode, setMode] = useState<DonutMode>("country");

  const cashHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCashHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCashExposure(allHoldings ?? []),
    [allHoldings],
  );
  const countryRows = useMemo(
    () => rollupByCountry(allHoldings ?? []),
    [allHoldings],
  );
  const currencyRows = useMemo(
    () => rollupByCurrency(allHoldings ?? []),
    [allHoldings],
  );

  const donutData = useMemo(() => {
    if (mode === "country") {
      return countryRows.map((r) => ({ label: r.country, value: r.exposureBase }));
    }
    return currencyRows.map((r) => ({ label: r.currency, value: r.exposureBase }));
  }, [mode, countryRows, currencyRows]);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Wallet className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Bank &amp; Cash</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {cashHoldings.length === 0 && !isLoading
            ? "No cash holdings yet."
            : `Total cash ${formatAmount(totalExposure, baseCurrency)} across ${cashHoldings.length} ${cashHoldings.length === 1 ? "account" : "accounts"}.`}
        </div>
      </header>

      {cashHoldings.length > 0 && (
        <>
          <section
            aria-label="Cash allocation"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 flex items-center justify-between">
              <div className="text-sm font-medium">
                {mode === "country" ? "By country" : "By currency"}
              </div>
              <div
                role="tablist"
                aria-label="Donut chart view"
                className="bg-muted/40 inline-flex rounded-full p-1"
              >
                <ToggleButton
                  active={mode === "country"}
                  onClick={() => setMode("country")}
                  label="Country"
                />
                <ToggleButton
                  active={mode === "currency"}
                  onClick={() => setMode("currency")}
                  label="Currency"
                />
              </div>
            </div>
            {donutData.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                {mode === "country"
                  ? "No country data on these cash holdings — classify them in the holding detail."
                  : "No currency data — set the local currency on the holding instrument."}
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={donutData}
                  ariaLabel={`Cash allocation by ${mode}`}
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Cash holdings"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Accounts</h2>
            </header>
            <ul role="list" className="divide-y">
              {cashHoldings.map((h) => {
                const label = h.instrument?.name ?? h.instrument?.symbol ?? h.id;
                return (
                  <li
                    key={h.id}
                    className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                  >
                    <button
                      type="button"
                      onClick={() => navigate(`/accounts/${h.accountId}`)}
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
