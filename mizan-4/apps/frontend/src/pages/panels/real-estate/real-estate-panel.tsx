/**
 * Real Estate panel — Track B PR-B11 / Goal v3 §V Phase 5.
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
  isRealEstateHolding,
  rollupByIntent,
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
  const intentCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByIntent(allHoldings ?? []).map((r) => ({
          label: r.intent,
          value: r.value,
        })),
      ),
    [allHoldings],
  );

  const empty = propertyHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Home}
        label="Real Estate"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No real-estate holdings yet."
            : `${propertyHoldings.length} ${propertyHoldings.length === 1 ? "property" : "properties"}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By intent"
            categories={intentCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No intent classification yet."
          />
          <HoldingsCard
            holdings={propertyHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="real-estate"
            title="Properties"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
