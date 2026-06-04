/**
 * Provident Funds panel — Track B PR-B9 / Goal v3 §V Phase 5.
 *
 * Composition:
 *   - Header: total PF + position count
 *   - Donut by scheme (CPF / EPF / 401k / NPS / Super / Other)
 *   - CPF sub-donut (OA / SA / MA / RA) — only renders when CPF is present
 *   - Positions list with scheme + sub-account chip
 *
 * §23 reference user has CPF OA + SA + MA + RA balances plus SRS
 * US-equity positions. The CPF sub-donut surfaces the four CPF
 * accounts; cross-asset linkage via `metadata.providentFund.sourceAccountId`
 * lets the user jump from a SGS bond row here to the bonds panel
 * (and back).
 *
 * Out of scope (track separately as PR-B9.a):
 *   - CPF interest accrual line (1Y backtest from monthly statements)
 *   - SRS top-up reminder insight
 *   - EPF withdrawal eligibility window banner
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
  isProvidentFundHolding,
  rollupByCpfSubAccount,
  rollupByPosition,
  rollupByScheme,
  totalProvidentFundExposure,
} from "./rollup";

export default function ProvidentFundsPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const pfHoldings = useMemo(
    () => (allHoldings ?? []).filter(isProvidentFundHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalProvidentFundExposure(allHoldings ?? []),
    [allHoldings],
  );
  const schemeRows = useMemo(
    () => rollupByScheme(allHoldings ?? []),
    [allHoldings],
  );
  const cpfRows = useMemo(
    () => rollupByCpfSubAccount(allHoldings ?? []),
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
          <Icons.PiggyBank className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Provident Funds</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {pfHoldings.length === 0 && !isLoading
            ? "No provident-fund positions yet."
            : `Total ${formatAmount(totalExposure, baseCurrency)} across ${pfHoldings.length} ${pfHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {pfHoldings.length > 0 && (
        <>
          <section
            aria-label="Provident funds by scheme"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By scheme</div>
            <div className="h-64">
              <Donut
                data={schemeRows.map((r) => ({ label: r.label, value: r.totalValueBase }))}
                ariaLabel="Provident fund exposure by scheme"
                palette="categorical"
              />
            </div>
          </section>

          {cpfRows.length > 0 && (
            <section
              aria-label="CPF sub-accounts"
              className="bg-card rounded-2xl border p-4"
            >
              <div className="mb-3 text-sm font-medium">CPF sub-accounts</div>
              <div className="h-64">
                <Donut
                  data={cpfRows.map((r) => ({
                    label: r.subAccount,
                    value: r.totalValueBase,
                  }))}
                  ariaLabel="CPF balances by sub-account (OA / SA / MA / RA)"
                  palette="categorical"
                />
              </div>
            </section>
          )}

          <section
            aria-label="Provident fund positions"
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
                      {p.schemeLabel}
                      {p.subAccount ? ` · ${p.subAccount}` : ""}
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
