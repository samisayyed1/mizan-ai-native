import { useQuery } from "@tanstack/react-query";

import { getTickerQuotes } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { TickerQuote } from "@/lib/types";

export function useTickerQuotes() {
  return useQuery<TickerQuote[], Error>({
    queryKey: [QueryKeys.TICKER_QUOTES],
    queryFn: getTickerQuotes,
    refetchInterval: 60_000,
    staleTime: 30_000,
  });
}
