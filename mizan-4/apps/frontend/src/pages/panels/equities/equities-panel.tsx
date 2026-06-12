/**
 * Equities panel — Track B PR-B2 / Goal v3 §V Phase 5 step B2.
 *
 * Composed from the shared world-class panel primitives in
 * `@/components/panels/panel-shared` — the same visual language
 * (gold ladder, premium holdings list, eyebrow heading) is now
 * shared across every asset-class panel.
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
  isEquityHolding,
  rollupByRegion,
  rollupBySubclass,
  totalEquityExposure,
} from "./rollup";

export default function EquitiesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const equityHoldings = useMemo(
    () => (allHoldings ?? []).filter(isEquityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalEquityExposure(allHoldings ?? []),
    [allHoldings],
  );
  const subclassCategories = useMemo(
    () =>
      rowsToCategories(
        rollupBySubclass(allHoldings ?? []).map((r) => ({
          label: r.label,
          value: r.value,
        })),
      ),
    [allHoldings],
  );
  const regionRows = useMemo(() => rollupByRegion(allHoldings ?? []), [allHoldings]);

  const empty = equityHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.TrendingUp}
        label="Equities"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No equity holdings yet."
            : `${equityHoldings.length} ${equityHoldings.length === 1 ? "position" : "positions"}${regionRows.length > 0 ? ` · ${regionRows.length} ${regionRows.length === 1 ? "region" : "regions"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <PanelGlanceRow>
            <AllocationDonutCard
              title="By sub-class"
              categories={subclassCategories}
              baseCurrency={baseCurrency}
              emptyMessage="No allocation data."
            />
            <GoldBarsCard
              title="By region"
              rows={regionRows.map((r) => ({ label: r.region, value: r.exposureBase }))}
              baseCurrency={baseCurrency}
              total={totalExposure}
              emptyMessage="No region data — classify holdings in their detail page."
            />
          </PanelGlanceRow>
          <HoldingsCard
            holdings={equityHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="equities"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
