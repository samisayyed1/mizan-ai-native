/**
 * Provident Funds panel — Track B PR-B9 / Goal v3 §V Phase 5.
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
  isProvidentFundHolding,
  rollupByCpfSubAccount,
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
  const schemeCategories = useMemo(
    () =>
      rowsToCategories(
        schemeRows.map((r) => ({ label: r.label, value: r.totalValueBase })),
      ),
    [schemeRows],
  );
  const cpfRows = useMemo(
    () =>
      rollupByCpfSubAccount(allHoldings ?? []).map((r) => ({
        label: r.subAccount,
        value: r.totalValueBase,
        sub: `${r.positionCount} ${r.positionCount === 1 ? "position" : "positions"}`,
      })),
    [allHoldings],
  );
  const hasCpf = cpfRows.some((r) => r.value > 0);

  const empty = pfHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.PiggyBank}
        label="Provident Funds"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No provident-fund holdings yet."
            : `${pfHoldings.length} ${pfHoldings.length === 1 ? "position" : "positions"}${schemeRows.length > 0 ? ` · ${schemeRows.length} ${schemeRows.length === 1 ? "scheme" : "schemes"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          {hasCpf ? (
            <PanelGlanceRow>
              <AllocationDonutCard
                title="By scheme"
                categories={schemeCategories}
                baseCurrency={baseCurrency}
                emptyMessage="No scheme classification yet."
              />
              <GoldBarsCard
                title="CPF sub-accounts"
                rows={cpfRows}
                baseCurrency={baseCurrency}
                total={totalExposure}
                emptyMessage="No CPF sub-accounts."
              />
            </PanelGlanceRow>
          ) : (
            <AllocationDonutCard
              title="By scheme"
              categories={schemeCategories}
              baseCurrency={baseCurrency}
              emptyMessage="No scheme classification yet."
            />
          )}
          <HoldingsCard
            holdings={pfHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="provident funds"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
