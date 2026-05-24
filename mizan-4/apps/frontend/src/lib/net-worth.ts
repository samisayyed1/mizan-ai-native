/**
 * Net-worth presentation helpers (Feroz §14 + §24).
 *
 * The backend computes net worth as assets − liabilities and INCLUDES
 * vehicle holdings in the assets total. Feroz's direction (#14) is that
 * vehicles are depreciating and must be excluded from net worth. PR #86
 * did this client-side, but only inside the net-worth *widget* — the
 * dedicated net-worth page still showed vehicles, so the two surfaces
 * disagreed for anyone holding a legacy vehicle.
 *
 * This module is the single source of truth for that exclusion so every
 * net-worth surface agrees. It transforms a raw `NetWorthResponse` into
 * an equivalent one with vehicle assets removed from the breakdown and
 * subtracted from both `assets.total` and `netWorth`.
 *
 * Pure + unit-tested. No UI imports.
 */

import type { NetWorthResponse } from "@/lib/types";

// The backend emits the asset category as "vehicles". Match on the
// `category` key (not the localized display name) and anchor to the
// start so it can never collide with another category (cash,
// investments, properties, collectibles, preciousMetals, otherAssets,
// liabilities — none start with "vehicle").
const VEHICLE_CATEGORY = /^vehicle/i;

/**
 * Return a NetWorthResponse with vehicle assets excluded.
 *
 * - Strict no-op (returns the same object) when there are no vehicle
 *   breakdown items, so the common case — and the entire Sunday demo,
 *   which has no vehicles — incurs zero risk and zero allocation.
 * - When vehicles exist: drops them from `assets.breakdown`, and
 *   subtracts their summed value from `assets.total` and `netWorth`.
 *   Liabilities are untouched.
 * - Values in the DTO are decimal strings; we parse, adjust, and
 *   re-stringify. Non-finite/garbage values are treated as 0 so a
 *   single malformed row can never poison the totals.
 */
export function excludeVehiclesFromNetWorth(data: NetWorthResponse): NetWorthResponse {
  const breakdown = data.assets.breakdown ?? [];
  const hasVehicle = breakdown.some((item) => VEHICLE_CATEGORY.test(item.category));
  if (!hasVehicle) return data;

  const vehicleSum = breakdown
    .filter((item) => VEHICLE_CATEGORY.test(item.category))
    .reduce((acc, item) => {
      const v = parseFloat(item.value);
      return acc + (Number.isFinite(v) ? v : 0);
    }, 0);

  const assetsTotal = (parseFloat(data.assets.total) || 0) - vehicleSum;
  const netWorth = (parseFloat(data.netWorth) || 0) - vehicleSum;

  return {
    ...data,
    netWorth: netWorth.toString(),
    assets: {
      ...data.assets,
      total: assetsTotal.toString(),
      breakdown: breakdown.filter((item) => !VEHICLE_CATEGORY.test(item.category)),
    },
  };
}
