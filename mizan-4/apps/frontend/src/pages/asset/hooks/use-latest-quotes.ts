import { useQuery } from "@tanstack/react-query";

import { getLatestQuotes } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { LatestQuoteSnapshot } from "@/lib/types";

export function useLatestQuotes(symbols: string[]) {
  return useQuery<Record<string, LatestQuoteSnapshot>>({
    // eslint-disable-next-line @tanstack/query/exhaustive-deps -- sorted symbols string is canonical identity
    queryKey: [QueryKeys.ASSETS, QueryKeys.LATEST_QUOTES, [...symbols].sort().join(",")],
    queryFn: () => getLatestQuotes(symbols),
    enabled: symbols.length > 0,
  });
}
