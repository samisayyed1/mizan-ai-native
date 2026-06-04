/**
 * Brokerage Accounts panel rollup — Track B PR-B8 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Brokerage Accounts": donut by broker + per-account list
 * surfacing NAV + position count + last-sync state. Distinct from the
 * Equities panel (PR-B2) which lines up the individual positions — this
 * panel lines up the *accounts* themselves so the user sees where their
 * brokerage exposure sits.
 *
 * Dedupe contract: every position lives in exactly one Account; the
 * Account.accountType determines whether the position is brokerage
 * (`'SECURITIES'`) or elsewhere (`'CASH'` → Bank & Cash panel,
 * `'CRYPTOCURRENCY'` → Crypto panel). A position counted in the
 * Equities panel by its assetType is counted here by its parent
 * account — no double-spending of NAV, two different lenses on the
 * same dollars.
 *
 * §23 reference user has a Wahed Halal IRA + Schwab brokerage +
 * Saxo SG + a SnapTrade-linked retail account — this panel groups
 * those four accounts under a donut keyed by broker.
 */
import type { Account, Holding } from "@/lib/types";
import { AccountType } from "@/lib/constants";

/** Is this Account a brokerage (securities) account? */
export function isBrokerageAccount(a: Account): boolean {
  return a.accountType === AccountType.SECURITIES;
}

/**
 * Resolve a broker label for an Account.
 *
 * Order:
 *   1. `account.platformId` (when the platform registry has been threaded
 *      through — surfaced as `account.meta.platformLabel` JSON if present).
 *   2. `account.provider` (PLAID, SNAPTRADE, MANUAL, etc.) normalised.
 *   3. `account.name` as last resort so we never render empty.
 */
export function brokerLabelFor(a: Account): string {
  // Try meta.platformLabel JSON if threaded
  if (a.meta) {
    try {
      const parsed = JSON.parse(a.meta) as { platformLabel?: string };
      if (parsed.platformLabel && typeof parsed.platformLabel === "string") {
        return parsed.platformLabel;
      }
    } catch {
      // ignore malformed JSON — fall through
    }
  }
  // Map known providers to friendly labels (extend as providers ship)
  if (a.provider) {
    const normalised = a.provider.toUpperCase();
    if (normalised === "SNAPTRADE") return "SnapTrade";
    if (normalised === "PLAID") return "Plaid";
    if (normalised === "MANUAL") return "Manual";
    if (normalised === "SETU") return "Setu";
    if (normalised === "SGFINDEX") return "SGFinDex";
    if (normalised === "TINK") return "Tink";
    if (normalised === "BASIQ") return "Basiq";
    if (normalised === "LEAN") return "Lean";
    if (normalised === "CCXT") return "CCXT";
    return a.provider;
  }
  return a.name;
}

/** Per-account row for the brokerage list. */
export interface BrokerageAccountRow {
  /** Stable account id for tap-row navigation. */
  accountId: string;
  /** Display name for the account itself (Schwab IRA, Wahed, etc.). */
  accountName: string;
  /** Resolved broker label (donut slice key). */
  broker: string;
  /** Sum of holdings.marketValue.base in this account, base currency. */
  totalValueBase: number;
  /** Number of distinct positions in this account (excludes zero/neg). */
  positionCount: number;
  /** Account-local currency, for sub-label rendering. */
  currency: string;
}

/**
 * Roll holdings into per-account rows, descending by total value.
 * Only `accountType === 'SECURITIES'` accounts are included.
 * Accounts with zero positions are excluded — they belong on the
 * Connections settings page, not the asset-class dashboard.
 */
export function rollupByAccount(
  holdings: readonly Holding[],
  accounts: readonly Account[],
): BrokerageAccountRow[] {
  const brokerageAccountIds = new Set(
    accounts.filter(isBrokerageAccount).map((a) => a.id),
  );
  const accountById = new Map(accounts.map((a) => [a.id, a]));

  // Aggregate per-account sums + position counts
  const totals = new Map<string, { sum: number; count: number }>();
  for (const h of holdings) {
    if (!brokerageAccountIds.has(h.accountId)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const entry = totals.get(h.accountId) ?? { sum: 0, count: 0 };
    entry.sum += value;
    entry.count += 1;
    totals.set(h.accountId, entry);
  }

  const rows: BrokerageAccountRow[] = [];
  for (const [accountId, { sum, count }] of totals.entries()) {
    const account = accountById.get(accountId);
    if (!account) continue;
    rows.push({
      accountId,
      accountName: account.name,
      broker: brokerLabelFor(account),
      totalValueBase: sum,
      positionCount: count,
      currency: account.currency,
    });
  }
  rows.sort((a, b) => b.totalValueBase - a.totalValueBase);
  return rows;
}

/** Per-broker donut slice (aggregates across accounts sharing a broker). */
export interface BrokerageBrokerRow {
  broker: string;
  totalValueBase: number;
  accountCount: number;
  positionCount: number;
}

/**
 * Roll holdings into per-broker donut rows, descending by value.
 * Aggregates accounts sharing a broker — e.g., two Schwab accounts
 * (Roth IRA + Taxable Brokerage) collapse into one "Charles Schwab"
 * slice with accountCount=2 + positionCount summed.
 */
export function rollupByBroker(
  holdings: readonly Holding[],
  accounts: readonly Account[],
): BrokerageBrokerRow[] {
  const perAccount = rollupByAccount(holdings, accounts);
  const map = new Map<string, BrokerageBrokerRow>();
  for (const r of perAccount) {
    const existing = map.get(r.broker);
    if (existing) {
      existing.totalValueBase += r.totalValueBase;
      existing.accountCount += 1;
      existing.positionCount += r.positionCount;
    } else {
      map.set(r.broker, {
        broker: r.broker,
        totalValueBase: r.totalValueBase,
        accountCount: 1,
        positionCount: r.positionCount,
      });
    }
  }
  const rows = Array.from(map.values());
  rows.sort((a, b) => b.totalValueBase - a.totalValueBase);
  return rows;
}

/** Sum of all brokerage NAV in base currency. */
export function totalBrokerageNav(
  holdings: readonly Holding[],
  accounts: readonly Account[],
): number {
  return rollupByAccount(holdings, accounts).reduce(
    (acc, row) => acc + row.totalValueBase,
    0,
  );
}
