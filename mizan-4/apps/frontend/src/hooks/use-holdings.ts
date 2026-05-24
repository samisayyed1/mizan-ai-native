import { useQuery } from "@tanstack/react-query";
import { Holding } from "@/lib/types";
import { getHoldings } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

export function useHoldings(accountId: string) {
  const {
    data: holdings = [],
    isLoading,
    isError,
    error,
  } = useQuery<Holding[], Error>({
    queryKey: [QueryKeys.HOLDINGS, accountId],
    queryFn: () => getHoldings(accountId),
    enabled: !!accountId,
    // Holdings only mutate via portfolio recalcs, which already invalidate this
    // query key. Without staleTime every navigation back to a page that mounts
    // useHoldings re-runs getHoldings against the Tauri bridge — wasteful, and
    // it makes account/dashboard switches feel laggy.
    staleTime: 30_000,
  });

  return { holdings, isLoading, isError, error };
}
