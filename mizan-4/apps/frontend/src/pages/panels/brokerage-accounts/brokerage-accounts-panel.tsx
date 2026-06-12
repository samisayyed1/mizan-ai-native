/**
 * Brokerage Accounts panel — Track B PR-B8 / Goal v3 §V Phase 5.
 *
 * Composed from the shared world-class panel primitives. The "holdings
 * list" surface is replaced with an Accounts list because the panel's
 * unit of navigation is the account, not an individual holding.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import {
  AllocationDonutCard,
  PanelHero,
  PanelShell,
  rowsToCategories,
} from "@/components/panels/panel-shared";
import { usePanelAdd } from "@/components/panels/use-panel-add";
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
  type BrokerageAccountRow,
} from "./rollup";

export default function BrokerageAccountsPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading: holdingsLoading } = useHoldings(
    PORTFOLIO_ACCOUNT_ID,
  );
  const { accounts, isLoading: accountsLoading } = useAccounts();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";
  const onAdd = usePanelAdd("brokerage");

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
  const brokerCategories = useMemo(
    () =>
      rowsToCategories(
        brokerRows.map((r) => ({ label: r.broker, value: r.totalValueBase })),
      ),
    [brokerRows],
  );

  const empty = accountRows.length === 0 && !isLoading;

  return (
    <PanelShell>
      <PanelHero
        icon={Icons.Briefcase}
        label="Brokerage"
        value={totalNav}
        baseCurrency={baseCurrency}
        meta={
          empty
            ? "No brokerage accounts connected yet."
            : `${accountRows.length} ${accountRows.length === 1 ? "account" : "accounts"}${brokerRows.length > 0 ? ` · ${brokerRows.length} ${brokerRows.length === 1 ? "broker" : "brokers"}` : ""}`
        }
        onAdd={onAdd}
        empty={empty}
      />

      {empty ? null : (
        <>
          <AllocationDonutCard
            title="By broker"
            categories={brokerCategories}
            baseCurrency={baseCurrency}
            emptyMessage="No broker classification yet."
          />
          <section
            aria-label="Brokerage accounts"
            className="bg-card overflow-hidden rounded-2xl border"
          >
            <header className="flex items-center justify-between border-b px-5 py-4">
              <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
                Accounts
              </h2>
              <span className="text-muted-foreground text-xs tabular-nums">
                {accountRows.length} {accountRows.length === 1 ? "row" : "rows"}
              </span>
            </header>
            <ul role="list" className="divide-border divide-y">
              {accountRows.map((row) => (
                <AccountRow
                  key={row.accountId}
                  row={row}
                  baseCurrency={baseCurrency}
                  totalNav={totalNav}
                  onSelect={() => navigate(`/accounts/${row.accountId}`)}
                />
              ))}
            </ul>
          </section>
        </>
      )}
    </PanelShell>
  );
}

function AccountRow({
  row,
  baseCurrency,
  totalNav,
  onSelect,
}: {
  row: BrokerageAccountRow;
  baseCurrency: string;
  totalNav: number;
  onSelect: () => void;
}) {
  const weight = totalNav > 0 ? (row.totalValueBase / totalNav) * 100 : 0;
  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className="hover:bg-muted/40 focus-visible:bg-muted/40 group flex w-full items-center gap-3 px-5 py-3.5 text-left transition-colors focus:outline-none"
      >
        <span className="bg-muted/60 text-foreground/80 grid h-9 w-12 shrink-0 place-items-center rounded-md text-[10px] font-semibold tracking-tight">
          {row.broker.slice(0, 5).toUpperCase()}
        </span>
        <span className="min-w-0 flex-1">
          <span className="text-foreground block truncate text-[13px] font-medium">
            {row.accountName}
          </span>
          <span className="text-muted-foreground text-[11px] tabular-nums">
            {row.broker} · {row.positionCount}{" "}
            {row.positionCount === 1 ? "position" : "positions"} · {weight.toFixed(1)}% of brokerage
          </span>
        </span>
        <span className="text-foreground shrink-0 text-right text-[14px] font-semibold tabular-nums">
          {formatAmount(row.totalValueBase, baseCurrency)}
        </span>
        <Icons.ChevronRight
          className="text-muted-foreground/40 group-hover:text-muted-foreground h-4 w-4 shrink-0 transition-colors"
          aria-hidden="true"
        />
      </button>
    </li>
  );
}
