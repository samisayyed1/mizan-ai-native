import { keepPreviousData, useQueries } from "@tanstack/react-query";
import { calculatePerformanceHistory } from "@/adapters";
import { format } from "date-fns";
import { DateRange } from "react-day-picker";
import { QueryKeys } from "@/lib/query-keys";
import { TrackedItem } from "@/lib/types";

/**
 * Hook to calculate cumulative returns for a list of comparison items.
 * Uses the user-selected date range directly for all queries.
 *
 * @param selectedItems List of comparison items to calculate cumulative returns for.
 * @param dateRange The date range for the calculation period.
 * @param trackingMode Optional tracking mode for accounts ("HOLDINGS" or "TRANSACTIONS").
 *                     Used for SOTA performance calculations in HOLDINGS mode.
 *
 * @returns An object containing the calculated cumulative returns data,
 *          a boolean indicating whether the data is loading,
 *          a boolean indicating whether there are any errors,
 *          an array of error messages,
 *          and a formatted display date range string.
 */
export function useCalculatePerformanceHistory({
  selectedItems,
  dateRange,
  trackingMode,
}: {
  selectedItems: TrackedItem[];
  dateRange: DateRange | undefined;
  trackingMode?: "HOLDINGS" | "TRANSACTIONS";
}) {
  // Filter out invalid items (defensive: handles stale localStorage data)
  const validItems = selectedItems.filter(
    (item) =>
      item && typeof item.id === "string" && item.id && typeof item.type === "string" && item.type,
  );

  // Get the formatted date range for API calls, keep as undefined if not present
  const startDate = dateRange?.from ? format(dateRange.from, "yyyy-MM-dd") : undefined;
  const endDate = dateRange?.to ? format(dateRange.to, "yyyy-MM-dd") : undefined;

  const performanceQueries = useQueries({
    queries: validItems.map((item) => ({
      queryKey: [
        QueryKeys.PERFORMANCE_HISTORY,
        item.type,
        item.id,
        startDate,
        endDate,
        trackingMode,
      ],
      queryFn: () =>
        calculatePerformanceHistory(
          item.type,
          item.id,
          startDate,
          endDate,
          // Only pass trackingMode for accounts, not for symbols
          item.type === "account" ? trackingMode : undefined,
        ),
      // Enable query only if dates are present (item validation done above).
      enabled: !!startDate && !!endDate,
      staleTime: 30 * 1000,
      retry: false,
      placeholderData: keepPreviousData,
    })),
  });

  const isLoading = performanceQueries.some((query) => query.isLoading);
  const hasErrors = performanceQueries.some((query) => query.isError);
  const errorMessages = performanceQueries
    .filter((query) => query.isError)
    .map((query) => query.error)
    .filter(Boolean)
    .map((error) => (error instanceof Error ? error.message : String(error)));

  // Format chart data directly from query results
  const chartData = performanceQueries
    .map((query, index) => {
      if (query.isError || !query.data) return null;

      const item = validItems[index];
      return {
        ...query.data,
        id: item.id,
        type: item.type,
        name: item.type === "symbol" ? `${item.name} (${item.id})` : item.name,
      };
    })
    .filter(Boolean);

  const displayStartDate = dateRange?.from ? format(dateRange.from, "MMM d, yyyy") : "";

  const displayEndDate = dateRange?.to ? format(dateRange.to, "MMM d, yyyy") : "";

  // Compute the actual data extent across all selected series. When the
  // user requests a window wider than they have data for (typical on a
  // freshly synced account with limited broker history), the chart will
  // be effectively clamped — surface that to the caller so the header
  // can be honest about what's actually shown.
  const actualDataStart = (() => {
    let earliest: string | undefined;
    for (const series of chartData) {
      const first = series?.returns?.[0]?.date;
      if (!first) continue;
      if (!earliest || first < earliest) earliest = first;
    }
    return earliest;
  })();

  const displayDateRange = (() => {
    if (!displayStartDate || !displayEndDate) {
      return "Compare account performance over time";
    }
    // If the actual data starts noticeably later than the requested window,
    // tell the user the truth.
    if (actualDataStart && startDate && actualDataStart > startDate) {
      const formattedActual = format(new Date(actualDataStart), "MMM d, yyyy");
      return `${formattedActual} – ${displayEndDate} · all available data (requested ${displayStartDate})`;
    }
    return `${displayStartDate} - ${displayEndDate}`;
  })();

  return {
    data: chartData,
    isLoading,
    hasErrors,
    errorMessages,
    queries: performanceQueries,
    formattedStartDate: startDate,
    formattedEndDate: endDate,
    displayDateRange,
    actualDataStart,
  };
}
