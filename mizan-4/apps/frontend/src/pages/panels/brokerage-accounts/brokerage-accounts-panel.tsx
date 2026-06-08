/**
 * Brokerage Accounts panel — Track B PR-B8 / Goal v3 §V Phase 5.
 *
 * Composition mirrors PR-B3 (Bank & Cash) — both panels line up
 * *accounts* rather than individual positions:
 *   - Header: total brokerage NAV + accounts count
 *   - Donut by broker (Schwab / Wahed / Saxo / SnapTrade-linked)
 *   - Accounts list (tap → account detail) showing NAV + position
 *     count + broker label + account-local currency chip
 *
 * Distinct from PR-B2 Equities which slices the *positions*. Both
 * read from the same holdings table — no double-spending of NAV.
 *
 * The SnapTrade sync-status badge ships separately as PR-B8.a once
 * the `BrokerSyncState` is threaded into the accounts query.
 *
 * Track UI PR-UI-4 — uses the shared AssetClassPanelLayout so the
 * page chrome matches every other panel detail view per Spec §6.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { AssetClassPanelLayout } from "@/components/panels/asset-class-panel-layout";
import { Donut } from "@/components/charts";
import { useAccounts } from "@/hooks/use-accounts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  rollupByAccount,
  rollupByBroker,
  totalBrokerageNav,
} from "./rollup";

export default function BrokerageAccountsPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading: holdingsLoading } = useHoldings(
    PORTFOLIO_ACCOUNT_ID,
  );
  const { accounts, isLoading: accountsLoading } = useAccounts();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const isLoading = holdingsLoading || accountsLoading;

  const accountRows = useMemo(
    () => rollupByAccount(allHoldings ?? [], accounts ?? []),
    [allHoldings, accounts],
  );
  const brokerRows = useMemo(
    () => rollupByBroker(allHoldings ?? [], accounts ?? []),
    [allHoldings, accounts],
  );
  const totalNav = useMemo(
    () => totalBrokerageNav(allHoldings ?? [], accounts ?? []),
    [allHoldings, accounts],
  );

  const summary =
    accountRows.length === 0 && !isLoading
      ? "No brokerage accounts yet."
      : `Total brokerage ${formatAmount(totalNav, baseCurrency)} across ${accountRows.length} ${accountRows.length === 1 ? "account" : "accounts"}.`;

  const chart =
    accountRows.length > 0 ? (
      <section
        aria-label="Brokerage NAV by broker"
        className="bg-card rounded-2xl border p-4"
      >
        <div className="mb-3 text-sm font-medium">By broker</div>
        <div className="h-64">
          <Donut
            data={brokerRows.map((r) => ({
              label: r.broker,
              value: r.totalValueBase,
            }))}
            ariaLabel="Brokerage NAV by broker"
            palette="categorical"
          />
        </div>
      </section>
    ) : null;

  const list =
    accountRows.length > 0 ? (
      <section
        aria-label="Brokerage accounts list"
        className="bg-card rounded-2xl border"
      >
        <header className="border-b px-4 py-3">
          <h2 className="text-sm font-medium">Accounts</h2>
        </header>
        <ul role="list" className="divide-y">
          {accountRows.map((row) => (
            <li
              key={row.accountId}
              className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
            >
              <button
                type="button"
                onClick={() => navigate(`/accounts/${row.accountId}`)}
                className="text-foreground flex-1 text-left text-sm font-medium"
              >
                <div>{row.accountName}</div>
                <div className="text-muted-foreground text-xs">
                  {row.broker} · {row.positionCount}{" "}
                  {row.positionCount === 1 ? "position" : "positions"} ·{" "}
                  {row.currency}
                </div>
              </button>
              <div className="text-foreground text-sm font-semibold tabular-nums">
                {formatAmount(row.totalValueBase, baseCurrency)}
              </div>
            </li>
          ))}
        </ul>
      </section>
    ) : null;

  return (
    <AssetClassPanelLayout
      title="Brokerage Accounts"
      IconComponent={Icons.Briefcase}
      summary={summary}
      chart={chart}
      list={list}
    />
  );
}
