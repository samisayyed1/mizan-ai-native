import { useEffect, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";

import { useHoldings } from "@/hooks/use-holdings";
import { useTickerQuotes } from "@/hooks/use-ticker-quotes";
import { HoldingType, PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { QueryKeys } from "@/lib/query-keys";

import { TickerItem, type TickerDatum } from "./ticker-item";

const MAX_ITEMS = 40;

/**
 * Dashboard ticker conveyor: an infinite marquee streaming the user's holdings
 * (with day-change colour) followed by curated market indices/commodities.
 * Renders nothing until there is something to show, so it never leaves an empty
 * strip. The row is duplicated twice and translated -50% for a seamless loop;
 * the global reduced-motion rule pauses it.
 */
export function TickerConveyor() {
  const { holdings } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { data: indices } = useTickerQuotes();
  const queryClient = useQueryClient();

  // Refetch the curated indices the moment the Tauri startup quote sync
  // completes. Without this listener, the ticker waits up to ~6 hours
  // (the periodic sync cadence) before showing live prices on a cold
  // launch. The backend emits this event from
  // `apps/tauri/src/scheduler.rs::run_startup_quote_sync`.
  useEffect(() => {
    const unlisten = listen("quotes:startup-sync-complete", () => {
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.TICKER_QUOTES] });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  const items = useMemo<TickerDatum[]>(() => {
    const fromHoldings: TickerDatum[] = holdings
      .filter((h) => h.holdingType === HoldingType.SECURITY && h.instrument?.symbol)
      .map((h) => ({
        key: `h-${h.instrument!.symbol}`,
        label: h.instrument!.symbol,
        price: h.price ?? null,
        changePct: h.dayChangePct ?? null,
        currency: h.localCurrency,
      }));
    const fromIndices: TickerDatum[] = (indices ?? []).map((q) => ({
      key: `i-${q.symbol}`,
      label: q.label,
      price: q.price ?? null,
      changePct: q.changePct ?? null,
      currency: q.currency ?? undefined,
    }));
    return [...fromHoldings, ...fromIndices].slice(0, MAX_ITEMS);
  }, [holdings, indices]);

  if (items.length === 0) return null;

  // Steady scroll speed: longer lists take proportionally longer per loop.
  const durationSeconds = Math.max(20, items.length * 4);

  return (
    <div className="bg-card/40 overflow-hidden border-b" aria-label="Live market ticker">
      <div
        className="animate-ticker flex w-max gap-6 py-2 hover:[animation-play-state:paused]"
        style={{ "--ticker-duration": `${durationSeconds}s` } as React.CSSProperties}
      >
        {[0, 1].map((copy) => (
          <div key={copy} aria-hidden={copy === 1} className="flex shrink-0 gap-6 pl-6">
            {items.map((item) => (
              <TickerItem key={`${copy}-${item.key}`} datum={item} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
