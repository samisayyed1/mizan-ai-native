import { useQuery } from "@tanstack/react-query";
import { parseISO } from "date-fns";
import { getHistoricalValuations } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

/**
 * Returns the earliest valuation date we have data for in `accountId`,
 * suitable for gating IntervalSelector to ranges we can actually plot.
 *
 * Cached aggressively (stale 5 min): the data extent doesn't change
 * minute-to-minute, and re-fetching for every chart interaction would be
 * wasteful.
 */
export function useValuationExtent(accountId: string): Date | null {
  const { data } = useQuery<Date | null, Error>({
    queryKey: [...QueryKeys.valuationHistory(accountId), "__extent__"],
    queryFn: async () => {
      // Unbounded fetch — backend returns rows sorted by date; first row
      // is the earliest. This is one-shot per session per account.
      const rows = await getHistoricalValuations(accountId, undefined, undefined);
      if (!rows?.length) return null;
      // Defensive sort in case the adapter doesn't guarantee order.
      const earliest = rows.reduce<string | null>((acc, row) => {
        if (!row.valuationDate) return acc;
        if (!acc || row.valuationDate < acc) return row.valuationDate;
        return acc;
      }, null);
      return earliest ? parseISO(earliest) : null;
    },
    staleTime: 5 * 60 * 1000,
  });
  return data ?? null;
}
