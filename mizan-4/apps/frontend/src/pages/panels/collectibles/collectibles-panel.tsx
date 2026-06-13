/**
 * Collectibles panel — Track B PR-B15 / Goal v3 §V Phase 5.
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
import { usePanelAdd } from "@/components/panels/use-panel-add";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";

import {
  isCollectibleHolding,
  rollupByCategory,
  totalCollectiblesExposure,
} from "./rollup";

export default function CollectiblesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";
  const onAdd = usePanelAdd("collectibles");

  const collectibleHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCollectibleHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCollectiblesExposure(allHoldings ?? []),
    [allHoldings],
  );
  const categoryCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByCategory(allHoldings ?? []).map((r) => ({
          label: r.category,
          value: r.value,
        })),
      ),
    [allHoldings],
  );

  const empty = collectibleHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Sparkles}
        label="Collectibles"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No collectibles yet."
            : `${collectibleHoldings.length} ${collectibleHoldings.length === 1 ? "item" : "items"}`
        }
        onAdd={onAdd}
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By category"
            categories={categoryCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No category classification yet."
          />
          <HoldingsCard
            holdings={collectibleHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="collectibles"
            title="Items"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
