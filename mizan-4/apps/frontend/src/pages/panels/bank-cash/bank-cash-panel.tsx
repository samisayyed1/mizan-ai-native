/**
 * Bank & Cash panel — Track B PR-B3 / Goal v3 §V Phase 5 step B3.
 *
 * Composed from the shared world-class panel primitives.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import {
  AllocationDonutCard,
  GoldBarsCard,
  HoldingsCard,
  PanelGlanceRow,
  PanelHero,
  PanelShell,
  rowsToCategories,
} from "@/components/panels/panel-shared";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";

import {
  isCashHolding,
  rollupByCountry,
  rollupByCurrency,
  totalCashExposure,
} from "./rollup";

export default function BankCashPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const cashHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCashHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCashExposure(allHoldings ?? []),
    [allHoldings],
  );
  const currencyCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByCurrency(allHoldings ?? []).map((r) => ({
          label: r.currency,
          value: r.exposureBase,
        })),
      ),
    [allHoldings],
  );
  const countryRows = useMemo(
    () =>
      rollupByCountry(allHoldings ?? []).map((r) => ({
        label: r.country,
        value: r.exposureBase,
      })),
    [allHoldings],
  );

  const empty = cashHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Wallet}
        label="Bank & Cash"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No cash holdings yet."
            : `${cashHoldings.length} ${cashHoldings.length === 1 ? "account" : "accounts"}${currencyCategories.length > 0 ? ` · ${currencyCategories.length} ${currencyCategories.length === 1 ? "currency" : "currencies"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <PanelGlanceRow>
            <AllocationDonutCard
              title="By currency"
              categories={currencyCategories}
              baseCurrency={baseCurrency}
              emptyMessage="No currency data."
            />
            <GoldBarsCard
              title="By country"
              rows={countryRows}
              baseCurrency={baseCurrency}
              total={totalExposure}
              emptyMessage="No country data — classify accounts in their detail page."
            />
          </PanelGlanceRow>
          <HoldingsCard
            holdings={cashHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="cash"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
