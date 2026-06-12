/**
 * Sukuks panel — Track B PR-B1 / Goal v3 §V Phase 5 / §23 anchor surface.
 *
 * Composed from the shared world-class panel primitives in
 * `@/components/panels/panel-shared` — same gold-ladder donut + bars
 * + holdings list pattern as every other asset-class panel.
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
  isBondHolding,
  rollupByIssuer,
  rollupByMaturityYear,
  totalBondExposure,
} from "./rollup";

export default function SukuksPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const bondHoldings = useMemo(
    () => (allHoldings ?? []).filter(isBondHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalBondExposure(allHoldings ?? []),
    [allHoldings],
  );
  const issuerCategories = useMemo(
    () =>
      rowsToCategories(
        rollupByIssuer(allHoldings ?? []).map((r) => ({
          label: r.issuer,
          value: r.exposureBase,
        })),
      ),
    [allHoldings],
  );
  const maturityRows = useMemo(
    () =>
      rollupByMaturityYear(allHoldings ?? []).map((r) => ({
        label: String(r.year),
        value: r.exposureBase,
      })),
    [allHoldings],
  );

  const empty = bondHoldings.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Coins}
        label="Sukuks"
        value={totalExposure}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No bond or sukuk holdings yet."
            : `${bondHoldings.length} ${bondHoldings.length === 1 ? "position" : "positions"}${issuerCategories.length > 0 ? ` · ${issuerCategories.length} ${issuerCategories.length === 1 ? "issuer" : "issuers"}` : ""}`
        }
        empty={empty}
      />

      {empty ? null : (
        <>
          <PanelGlanceRow>
            <AllocationDonutCard
              title="By issuer"
              categories={issuerCategories}
              baseCurrency={baseCurrency}
              emptyMessage="No issuer data — classify holdings in their detail page."
            />
            <GoldBarsCard
              title="Maturity ladder"
              rows={maturityRows}
              baseCurrency={baseCurrency}
              total={totalExposure}
              emptyMessage="No maturity data on these holdings yet."
            />
          </PanelGlanceRow>
          <HoldingsCard
            holdings={bondHoldings}
            baseCurrency={baseCurrency}
            totalExposure={totalExposure}
            classNoun="sukuks"
            onSelect={(h) =>
              navigate(`/holdings/${h.instrument?.id ?? h.id}`)
            }
          />
        </>
      )}
    </PanelShell>
  );
}
