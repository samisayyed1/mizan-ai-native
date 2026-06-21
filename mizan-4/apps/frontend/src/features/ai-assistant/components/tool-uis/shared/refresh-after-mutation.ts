import type { QueryClient } from "@tanstack/react-query";

import { recalculatePortfolio } from "@/adapters";

/**
 * After an AI write tool runs to completion, the new row(s) live in
 * SQLite but the *derived* surfaces — TOTAL `daily_account_valuation`,
 * the holdings snapshot, the allocation rollup, the heatmap — still
 * reflect the pre-mutation state. The dashboard reads from those
 * derived tables, so without a recompute the user sees "the AI said
 * it added my $1M property but Net Worth is unchanged" for ~60s
 * until the next scheduled recalculation. That feels broken even
 * though the data IS in the DB.
 *
 * This helper makes the loop feel "live like water":
 *
 *   1. Kick the Rust `recalculate_portfolio` command so the TOTAL
 *      snapshot + all per-account valuations regenerate in the
 *      background (~1–2 s for a 30-position portfolio).
 *   2. Invalidate EVERY react-query cache. Any view the user
 *      navigates to immediately refetches; the dashboard's
 *      `useValuationHistory`, `useHoldings`, etc. each see fresh
 *      numbers as soon as the recompute lands.
 *
 * The recompute is fire-and-forget — we don't await it. The intent
 * is to push it into the background so the AI tool UI's success
 * card flips immediately without a spinner on the click path. The
 * invalidations queue refetches that will land as soon as the
 * Rust side commits.
 *
 * Idempotent + safe to call from every write-tool success handler.
 */
export function refreshAfterMutation(queryClient: QueryClient): void {
  void recalculatePortfolio().catch(() => {
    // Swallow — the manual "Recalculate" button on the dashboard is
    // the user's fallback if this background trigger ever fails.
    // The mutation itself already succeeded; failure to schedule
    // recompute is a freshness degradation, not a data-loss bug.
  });
  // `invalidateQueries()` with no key invalidates everything. That's
  // intentional — AI write tools can touch holdings + activities +
  // goals + liabilities + net-worth + allocation + news cards
  // simultaneously (e.g. "delete my retirement goal AND the
  // associated mortgage"), and enumerating every affected key per
  // tool is brittle. Refetches only fire for actively-mounted
  // queries; everything else just marks-stale.
  queryClient.invalidateQueries();
}
