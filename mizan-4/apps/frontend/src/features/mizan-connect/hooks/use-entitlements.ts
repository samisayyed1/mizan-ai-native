import { useQuery } from "@tanstack/react-query";

import { getEntitlements } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

import { type Entitlements, UNLIMITED, withinLimit } from "../types";

/** Silver-capability defaults, used while loading or signed out. */
const SILVER_ENTITLEMENTS: Entitlements = {
  plan: "silver",
  maxPortfolios: 25,
  maxHoldings: UNLIMITED,
  maxAssetClasses: UNLIMITED,
  brokerSync: false,
  maxBrokerConnections: 0,
  deviceSync: false,
  cloudBackup: false,
  managedAi: true,
  aiCreditsMonthly: 300,
  newsDailyLimit: 3,
  marketRefreshDailyLimit: 0,
  csvImportsMonthly: UNLIMITED,
  advancedReports: false,
  zakatEngine: false,
  advisorMode: false,
};

/**
 * The current user's subscription entitlements. Resolves to Silver defaults
 * while loading or signed out, so callers can read fields unconditionally
 * (`entitlements.brokerSync`, etc.) without null checks.
 *
 * The query is long-lived; invalidate `QueryKeys.ENTITLEMENTS` after a
 * subscription change (e.g. returning from Stripe Checkout) to refresh.
 */
export function useEntitlements() {
  const query = useQuery({
    queryKey: [QueryKeys.ENTITLEMENTS],
    queryFn: getEntitlements,
    staleTime: 5 * 60 * 1000,
  });

  const entitlements = query.data ?? SILVER_ENTITLEMENTS;

  return {
    entitlements,
    isLoading: query.isLoading,
    isPaid: true,
    /** Whether adding one more of `current` stays within `limit`. */
    canAddWithin: (current: number, limit: number) => withinLimit(current, limit),
  };
}
