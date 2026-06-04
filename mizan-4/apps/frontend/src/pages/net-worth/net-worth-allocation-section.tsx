/**
 * Net Worth allocation Sankey section — Track NW PR-NW2 / Goal v3 §V Phase 7.
 *
 * Renders a Sankey diagram flowing from "Net Worth" → each non-empty
 * asset-class panel. Sits between the net-worth value chart and the
 * holdings list on the Net Worth page so users immediately see how
 * their wealth is decomposed.
 *
 * Self-contained — owns its own `useHoldings` call so the parent
 * `net-worth-content.tsx` stays untouched-shape. The Sankey re-fetches
 * whenever holdings change.
 *
 * Out of scope (deferred):
 *   - Income → spending → savings cash-flow Sankey (PR-NW2.b)
 *   - Liabilities → categories Sankey (PR-NW3)
 *   - Tap-on-node → asset-class panel navigation (PR-NW2.c)
 */
import { useMemo } from "react";

import { Sankey } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  buildAssetAllocationFlow,
  totalNetWorthBase,
} from "./asset-allocation-flow";

export default function NetWorthAllocationSection() {
  const { holdings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const dataset = useMemo(
    () => buildAssetAllocationFlow(holdings ?? []),
    [holdings],
  );
  const total = useMemo(
    () => totalNetWorthBase(holdings ?? []),
    [holdings],
  );

  if (isLoading) {
    return (
      <section
        aria-label="Net worth allocation flow"
        className="bg-card rounded-2xl border p-4"
      >
        <div className="text-muted-foreground text-sm">Loading allocation…</div>
      </section>
    );
  }

  if (!dataset || dataset.links.length === 0) {
    return (
      <section
        aria-label="Net worth allocation flow"
        className="bg-card rounded-2xl border p-4"
      >
        <div className="mb-2 text-sm font-medium">Allocation flow</div>
        <div className="text-muted-foreground py-8 text-center text-sm">
          Add a holding to see your Net Worth flow.
        </div>
      </section>
    );
  }

  return (
    <section
      aria-label="Net worth allocation flow"
      className="bg-card rounded-2xl border p-4"
    >
      <div className="mb-1 flex items-center justify-between">
        <div className="text-sm font-medium">Allocation flow</div>
        <div className="text-muted-foreground text-xs">
          {formatAmount(total, baseCurrency)} across {dataset.links.length}{" "}
          {dataset.links.length === 1 ? "class" : "classes"}
        </div>
      </div>
      <Sankey
        dataset={dataset}
        ariaLabel={`Net worth flows from total ${formatAmount(total, baseCurrency)} to ${dataset.links.length} asset class${dataset.links.length === 1 ? "" : "es"}`}
        palette="categorical"
        height={Math.max(240, dataset.links.length * 40)}
      />
    </section>
  );
}
