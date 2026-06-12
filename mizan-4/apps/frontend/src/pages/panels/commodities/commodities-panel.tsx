/**
 * Commodities panel — Track B PR-B10 / Goal v3 §V Phase 5.
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
  isCommodityHolding,
  rollupByMetal,
  totalCommoditiesExposure,
} from "./rollup";

export default function CommoditiesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const commodityHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCommodityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCommoditiesExposure(allHoldings ?? []),
    [allHoldings],
  );
  const metalCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByMetal(allHoldings ?? []).map((r) => ({
          label: r.metal,
          value: r.exposureBase,
        })),
      ),
    [allHoldings],
  );

  const empty = commodityHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Gem}
        label="Commodities"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No commodities yet."
            : `${commodityHoldings.length} ${commodityHoldings.length === 1 ? "position" : "positions"}${metalCategories.length > 0 ? ` · ${metalCategories.length} ${metalCategories.length === 1 ? "metal" : "metals"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By metal"
            categories={metalCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No metal classification yet."
          />
          <HoldingsCard
            holdings={commodityHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="commodities"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
