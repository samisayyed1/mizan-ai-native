/**
 * Insurance panel — Track B PR-B10 / Goal v3 §V Phase 5.
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
  isInsuranceHolding,
  rollupByCategory,
  totalInsuranceExposure,
} from "./rollup";

export default function InsurancePanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const insuranceHoldings = useMemo(
    () => (allHoldings ?? []).filter(isInsuranceHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalInsuranceExposure(allHoldings ?? []),
    [allHoldings],
  );
  const categoryRows = useMemo(
    () => rollupByCategory(allHoldings ?? []),
    [allHoldings],
  );
  const categoryCategories = useMemo(
    () =>
      rowsToCategories(
        categoryRows.map((r) => ({ label: r.label, value: r.totalValueBase })),
      ),
    [categoryRows],
  );

  const empty = insuranceHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.ShieldCheck}
        label="Insurance"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No insurance policies yet."
            : `${insuranceHoldings.length} ${insuranceHoldings.length === 1 ? "policy" : "policies"}${categoryRows.length > 0 ? ` · ${categoryRows.length} ${categoryRows.length === 1 ? "category" : "categories"}` : ""}`
        }
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
            holdings={insuranceHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="insurance"
            title="Policies"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
