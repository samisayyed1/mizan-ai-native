import { useQuery } from "@tanstack/react-query";

import { fetchFinancialNews } from "@/adapters";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { QueryKeys } from "@/lib/query-keys";
import type { Holding, NewsArticle, NewsScope } from "@/lib/types";

import { useHoldings } from "./use-holdings";

/**
 * Derive the symbols to request "For You" news for, from the user's holdings.
 * Uses the bare ticker (the frontend Holding doesn't carry the exchange MIC);
 * the backend matches against TradingView's symbol filter best-effort.
 */
export function buildNewsSymbols(holdings: Holding[]): string[] {
  const symbols = holdings
    .map((h) => h.instrument?.symbol?.trim().toUpperCase())
    .filter((s): s is string => !!s);
  return Array.from(new Set(symbols)).slice(0, 12);
}

export function useFinancialNews(scope: NewsScope) {
  const { holdings } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const symbols = scope === "forYou" ? buildNewsSymbols(holdings) : [];

  const query = useQuery<NewsArticle[], Error>({
    queryKey: [QueryKeys.FINANCIAL_NEWS, scope, symbols],
    queryFn: () => fetchFinancialNews(scope === "forYou" ? symbols : undefined),
    // Markets is always enabled; For You only when the user holds something.
    enabled: scope === "markets" || symbols.length > 0,
    refetchInterval: 5 * 60 * 1000,
    staleTime: 60 * 1000,
  });

  return {
    data: query.data,
    isLoading: query.isLoading,
    isError: query.isError,
    error: query.error,
    hasSymbols: symbols.length > 0,
  };
}
