/**
 * Crypto panel — Track B PR-B9 / Goal v3 §V Phase 5.
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
  isCryptoHolding,
  rollupByChain,
  totalCryptoExposure,
} from "./rollup";

export default function CryptoPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";
  const onAdd = usePanelAdd("crypto");

  const cryptoHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCryptoHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCryptoExposure(allHoldings ?? []),
    [allHoldings],
  );
  const chainCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByChain(allHoldings ?? []).map((r) => ({
          label: r.chain,
          value: r.exposureBase,
        })),
      ),
    [allHoldings],
  );

  const empty = cryptoHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Bitcoin}
        label="Crypto"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No crypto holdings yet."
            : `${cryptoHoldings.length} ${cryptoHoldings.length === 1 ? "position" : "positions"}${chainCategories.length > 0 ? ` · ${chainCategories.length} ${chainCategories.length === 1 ? "chain" : "chains"}` : ""}`
        }
        onAdd={onAdd}
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By chain"
            categories={chainCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No chain data."
          />
          <HoldingsCard
            holdings={cryptoHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="crypto"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
