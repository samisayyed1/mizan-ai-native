/**
 * Private Equity panel — Track B PR-B7 / Goal v3 §V Phase 5.
 *
 * Composed from the shared world-class panel primitives.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import {
  GoldBarsCard,
  HoldingsCard,
  PanelHero,
  PanelShell,
} from "@/components/panels/panel-shared";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";

import {
  isPrivateEquityHolding,
  rollupByVintage,
  totalPrivateEquityExposure,
} from "./rollup";

export default function PrivateEquityPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const peHoldings = useMemo(
    () => (allHoldings ?? []).filter(isPrivateEquityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalPrivateEquityExposure(allHoldings ?? []),
    [allHoldings],
  );
  const vintageRows = useMemo(
    () =>
      rollupByVintage(allHoldings ?? []).map((r) => ({
        label: String(r.year),
        value: r.exposureBase,
      })),
    [allHoldings],
  );

  const empty = peHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Building}
        label="Private Equity"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No PE positions yet."
            : `${peHoldings.length} ${peHoldings.length === 1 ? "position" : "positions"}${vintageRows.length > 0 ? ` · ${vintageRows.length} ${vintageRows.length === 1 ? "vintage" : "vintages"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <GoldBarsCard
            title="By vintage year"
            rows={vintageRows}
            baseCurrency={baseCurrency}
            total={totalExposure}
            emptyMessage="No vintage data on these positions yet."
          />
          <HoldingsCard
            holdings={peHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="PE"
            title="Positions"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
