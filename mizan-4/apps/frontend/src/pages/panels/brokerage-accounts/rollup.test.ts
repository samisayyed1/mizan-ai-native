/**
 * Tests for Track B PR-B8 Brokerage Accounts rollup helpers.
 */
import { describe, expect, it } from "vitest";

import { AccountType } from "@/lib/constants";
import type { Account, Holding, TrackingMode } from "@/lib/types";

import {
  brokerLabelFor,
  isBrokerageAccount,
  rollupByAccount,
  rollupByBroker,
  totalBrokerageNav,
} from "./rollup";

function makeAccount(opts: Partial<Account> & { id: string }): Account {
  return {
    id: opts.id,
    name: opts.name ?? opts.id,
    accountType: opts.accountType ?? AccountType.SECURITIES,
    balance: opts.balance ?? 0,
    currency: opts.currency ?? "USD",
    isDefault: opts.isDefault ?? false,
    isActive: opts.isActive ?? true,
    isArchived: opts.isArchived ?? false,
    trackingMode: opts.trackingMode ?? ("HOLDINGS" as TrackingMode),
    createdAt: opts.createdAt ?? new Date("2026-01-01"),
    updatedAt: opts.updatedAt ?? new Date("2026-01-01"),
    platformId: opts.platformId,
    accountNumber: opts.accountNumber,
    meta: opts.meta,
    provider: opts.provider,
    providerAccountId: opts.providerAccountId,
    group: opts.group,
  };
}

function makeHolding(opts: {
  id: string;
  accountId: string;
  baseValue: number;
}): Holding {
  return {
    id: opts.id,
    holdingType: "SECURITY" as Holding["holdingType"],
    accountId: opts.accountId,
    quantity: 1,
    localCurrency: "USD",
    baseCurrency: "USD",
    marketValue: { local: opts.baseValue, base: opts.baseValue },
    weight: 1,
    asOfDate: "2026-06-04",
    assetKind: "INVESTMENT",
    instrument: {
      id: `i-${opts.id}`,
      symbol: opts.id,
      name: opts.id,
      currency: "USD",
      quoteMode: "MANUAL",
      classifications: null,
    },
  };
}

describe("isBrokerageAccount", () => {
  it("returns true for SECURITIES accountType", () => {
    expect(isBrokerageAccount(makeAccount({ id: "a1" }))).toBe(true);
  });

  it("returns false for CASH / CRYPTOCURRENCY", () => {
    expect(
      isBrokerageAccount(
        makeAccount({ id: "a2", accountType: AccountType.CASH }),
      ),
    ).toBe(false);
    expect(
      isBrokerageAccount(
        makeAccount({ id: "a3", accountType: AccountType.CRYPTOCURRENCY }),
      ),
    ).toBe(false);
  });
});

describe("brokerLabelFor", () => {
  it("prefers meta.platformLabel JSON when present", () => {
    expect(
      brokerLabelFor(
        makeAccount({
          id: "a",
          name: "Roth IRA",
          provider: "SNAPTRADE",
          meta: JSON.stringify({ platformLabel: "Charles Schwab" }),
        }),
      ),
    ).toBe("Charles Schwab");
  });

  it("falls back to mapped provider label", () => {
    expect(
      brokerLabelFor(makeAccount({ id: "a", provider: "SNAPTRADE" })),
    ).toBe("SnapTrade");
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "PLAID" }))).toBe(
      "Plaid",
    );
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "MANUAL" }))).toBe(
      "Manual",
    );
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "SETU" }))).toBe(
      "Setu",
    );
    expect(
      brokerLabelFor(makeAccount({ id: "a", provider: "SGFINDEX" })),
    ).toBe("SGFinDex");
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "TINK" }))).toBe(
      "Tink",
    );
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "BASIQ" }))).toBe(
      "Basiq",
    );
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "LEAN" }))).toBe(
      "Lean",
    );
    expect(brokerLabelFor(makeAccount({ id: "a", provider: "CCXT" }))).toBe(
      "CCXT",
    );
  });

  it("normalises provider case before mapping", () => {
    expect(
      brokerLabelFor(makeAccount({ id: "a", provider: "snaptrade" })),
    ).toBe("SnapTrade");
  });

  it("returns the raw provider when unmapped", () => {
    expect(
      brokerLabelFor(makeAccount({ id: "a", provider: "INTERACTIVE_BROKERS" })),
    ).toBe("INTERACTIVE_BROKERS");
  });

  it("falls back to account name when no provider", () => {
    expect(brokerLabelFor(makeAccount({ id: "a", name: "Wahed Halal" }))).toBe(
      "Wahed Halal",
    );
  });

  it("ignores malformed meta JSON and falls back gracefully", () => {
    expect(
      brokerLabelFor(
        makeAccount({
          id: "a",
          name: "Schwab IRA",
          meta: "this-is-not-json",
        }),
      ),
    ).toBe("Schwab IRA");
  });

  it("ignores meta JSON without platformLabel key", () => {
    expect(
      brokerLabelFor(
        makeAccount({
          id: "a",
          name: "Saxo",
          meta: JSON.stringify({ something: "else" }),
        }),
      ),
    ).toBe("Saxo");
  });
});

describe("rollupByAccount", () => {
  it("§23 fixture: four brokerage accounts grouped by NAV desc", () => {
    const accounts = [
      makeAccount({
        id: "acc-wahed",
        name: "Wahed Halal IRA",
        provider: "SNAPTRADE",
      }),
      makeAccount({
        id: "acc-schwab",
        name: "Schwab Brokerage",
        provider: "SNAPTRADE",
      }),
      makeAccount({
        id: "acc-saxo",
        name: "Saxo SG",
        provider: "MANUAL",
      }),
      makeAccount({
        id: "acc-cash",
        name: "DBS Savings",
        accountType: AccountType.CASH,
      }),
    ];
    const holdings = [
      makeHolding({ id: "h1", accountId: "acc-schwab", baseValue: 500_000 }),
      makeHolding({ id: "h2", accountId: "acc-schwab", baseValue: 250_000 }),
      makeHolding({ id: "h3", accountId: "acc-wahed", baseValue: 300_000 }),
      makeHolding({ id: "h4", accountId: "acc-saxo", baseValue: 150_000 }),
      makeHolding({ id: "h5", accountId: "acc-cash", baseValue: 100_000 }),
    ];
    const rows = rollupByAccount(holdings, accounts);
    expect(rows).toEqual([
      {
        accountId: "acc-schwab",
        accountName: "Schwab Brokerage",
        broker: "SnapTrade",
        totalValueBase: 750_000,
        positionCount: 2,
        currency: "USD",
      },
      {
        accountId: "acc-wahed",
        accountName: "Wahed Halal IRA",
        broker: "SnapTrade",
        totalValueBase: 300_000,
        positionCount: 1,
        currency: "USD",
      },
      {
        accountId: "acc-saxo",
        accountName: "Saxo SG",
        broker: "Manual",
        totalValueBase: 150_000,
        positionCount: 1,
        currency: "USD",
      },
    ]);
  });

  it("excludes cash-account holdings — no double-spending of NAV across panels", () => {
    const accounts = [
      makeAccount({ id: "acc-cash", accountType: AccountType.CASH }),
    ];
    const holdings = [
      makeHolding({ id: "h", accountId: "acc-cash", baseValue: 100_000 }),
    ];
    expect(rollupByAccount(holdings, accounts)).toEqual([]);
  });

  it("excludes crypto-account holdings — that's the Crypto panel's turf", () => {
    const accounts = [
      makeAccount({
        id: "acc-crypto",
        accountType: AccountType.CRYPTOCURRENCY,
      }),
    ];
    const holdings = [
      makeHolding({ id: "h", accountId: "acc-crypto", baseValue: 50_000 }),
    ];
    expect(rollupByAccount(holdings, accounts)).toEqual([]);
  });

  it("excludes accounts with zero positions (belong on Connections page, not dashboard)", () => {
    const accounts = [
      makeAccount({ id: "acc-empty", name: "Empty Schwab" }),
      makeAccount({ id: "acc-funded", name: "Funded" }),
    ];
    const holdings = [
      makeHolding({ id: "h", accountId: "acc-funded", baseValue: 1000 }),
    ];
    const rows = rollupByAccount(holdings, accounts);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.accountId).toBe("acc-funded");
  });

  it("skips zero/negative position values", () => {
    const accounts = [makeAccount({ id: "acc" })];
    const holdings = [
      makeHolding({ id: "h1", accountId: "acc", baseValue: 0 }),
      makeHolding({ id: "h2", accountId: "acc", baseValue: -10 }),
      makeHolding({ id: "h3", accountId: "acc", baseValue: 100 }),
    ];
    expect(rollupByAccount(holdings, accounts)).toEqual([
      {
        accountId: "acc",
        accountName: "acc",
        broker: "acc",
        totalValueBase: 100,
        positionCount: 1,
        currency: "USD",
      },
    ]);
  });

  it("preserves account-local currency for sub-label rendering", () => {
    const accounts = [
      makeAccount({ id: "acc-sg", currency: "SGD", name: "OCBC Securities" }),
    ];
    const holdings = [
      makeHolding({ id: "h", accountId: "acc-sg", baseValue: 100 }),
    ];
    expect(rollupByAccount(holdings, accounts)[0]?.currency).toBe("SGD");
  });

  it("ignores holdings pointing at non-existent accounts", () => {
    const accounts = [makeAccount({ id: "acc" })];
    const holdings = [
      makeHolding({ id: "h", accountId: "ghost-account", baseValue: 100 }),
    ];
    expect(rollupByAccount(holdings, accounts)).toEqual([]);
  });
});

describe("rollupByBroker", () => {
  it("aggregates accounts sharing a broker label", () => {
    const accounts = [
      makeAccount({
        id: "schwab-roth",
        name: "Schwab Roth IRA",
        meta: JSON.stringify({ platformLabel: "Charles Schwab" }),
      }),
      makeAccount({
        id: "schwab-tax",
        name: "Schwab Taxable",
        meta: JSON.stringify({ platformLabel: "Charles Schwab" }),
      }),
      makeAccount({
        id: "wahed",
        name: "Wahed IRA",
        meta: JSON.stringify({ platformLabel: "Wahed" }),
      }),
    ];
    const holdings = [
      makeHolding({ id: "h1", accountId: "schwab-roth", baseValue: 200_000 }),
      makeHolding({ id: "h2", accountId: "schwab-roth", baseValue: 100_000 }),
      makeHolding({ id: "h3", accountId: "schwab-tax", baseValue: 500_000 }),
      makeHolding({ id: "h4", accountId: "wahed", baseValue: 300_000 }),
    ];
    expect(rollupByBroker(holdings, accounts)).toEqual([
      {
        broker: "Charles Schwab",
        totalValueBase: 800_000,
        accountCount: 2,
        positionCount: 3,
      },
      {
        broker: "Wahed",
        totalValueBase: 300_000,
        accountCount: 1,
        positionCount: 1,
      },
    ]);
  });

  it("returns empty array on no brokerage holdings", () => {
    expect(rollupByBroker([], [])).toEqual([]);
  });
});

describe("totalBrokerageNav", () => {
  it("sums all brokerage positions in base ccy", () => {
    const accounts = [
      makeAccount({ id: "b1" }),
      makeAccount({ id: "b2" }),
      makeAccount({ id: "cash", accountType: AccountType.CASH }),
    ];
    const holdings = [
      makeHolding({ id: "h1", accountId: "b1", baseValue: 100 }),
      makeHolding({ id: "h2", accountId: "b2", baseValue: 250 }),
      makeHolding({ id: "h3", accountId: "cash", baseValue: 9_999_999 }),
    ];
    expect(totalBrokerageNav(holdings, accounts)).toBe(350);
  });

  it("empty inputs return 0", () => {
    expect(totalBrokerageNav([], [])).toBe(0);
  });
});
