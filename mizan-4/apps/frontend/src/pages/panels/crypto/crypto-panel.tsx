/**
 * Crypto panel — Track B PR-B9 / Goal v3 §V Phase 5.
 *
 * Composition mirrors PR-B1/B2/B3/B11:
 *   - Header: total crypto exposure + position count
 *   - Donut by chain (Bitcoin / Ethereum / Solana / BNB Chain /
 *     Polygon / Stablecoins / Other)
 *   - Holdings list (tap → asset detail)
 *
 * The Sharia-toggleable flag (per ADR 0036) lands separately with
 * PR-F9. This PR ships the panel surface.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { Donut } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

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

  const cryptoHoldings = useMemo(
    () => (allHoldings ?? []).filter(isCryptoHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalCryptoExposure(allHoldings ?? []),
    [allHoldings],
  );
  const chainRows = useMemo(() => rollupByChain(allHoldings ?? []), [allHoldings]);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.Bitcoin className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Crypto</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {cryptoHoldings.length === 0 && !isLoading
            ? "No crypto holdings yet."
            : `Total crypto ${formatAmount(totalExposure, baseCurrency)} across ${cryptoHoldings.length} ${cryptoHoldings.length === 1 ? "position" : "positions"}.`}
        </div>
      </header>

      {cryptoHoldings.length > 0 && (
        <>
          <section
            aria-label="Crypto allocation by chain"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By chain</div>
            {chainRows.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                No allocation data available.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={chainRows.map((r) => ({
                    label: r.chain,
                    value: r.exposureBase,
                  }))}
                  ariaLabel="Crypto allocation by chain"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Crypto holdings"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Holdings</h2>
            </header>
            <ul role="list" className="divide-y">
              {cryptoHoldings.map((h) => {
                const label = h.instrument?.name ?? h.instrument?.symbol ?? h.id;
                return (
                  <li
                    key={h.id}
                    className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                  >
                    <button
                      type="button"
                      onClick={() => navigate(`/holdings/${h.instrument?.id ?? h.id}`)}
                      className="text-foreground flex-1 text-left text-sm font-medium"
                    >
                      {label}
                    </button>
                    <div className="text-foreground text-sm font-semibold tabular-nums">
                      {formatAmount(h.marketValue?.base ?? 0, baseCurrency)}
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        </>
      )}

      <div className="pt-2">
        <Button variant="outline" size="sm" onClick={() => navigate("/")}>
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          Back to dashboard
        </Button>
      </div>
    </div>
  );
}
