/**
 * Forex panel — Track B PR-B16 / Goal v3 §V Phase 5.
 *
 * Composed from the shared world-class panel primitives.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import {
  AllocationDonutCard,
  HoldingsCard,
  PanelHero,
  PanelShell,
  rowsToCategories,
} from "@/components/panels/panel-shared";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";

import {
  isFxHolding,
  rollupByLongLeg,
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
  const longLegCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByLongLeg(allHoldings ?? []).map((r) => ({
          label: r.currency,
          value: r.exposureBase,
        })),
      ),
    [allHoldings],
  );

  const empty = fxHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.RefreshCw}
        label="Forex"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No FX positions yet."
            : `${fxHoldings.length} ${fxHoldings.length === 1 ? "position" : "positions"}${longLegCategories.length > 0 ? ` · ${longLegCategories.length} long ${longLegCategories.length === 1 ? "currency" : "currencies"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By long currency"
            categories={longLegCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No long-leg currency data."
          />
          <HoldingsCard
            holdings={fxHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="FX"
            title="Pairs"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
