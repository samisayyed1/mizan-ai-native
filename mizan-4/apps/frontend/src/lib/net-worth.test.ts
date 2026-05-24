import { describe, expect, it } from "vitest";
import { excludeVehiclesFromNetWorth } from "./net-worth";
import type { BreakdownItem, NetWorthResponse } from "./types";

function item(category: string, name: string, value: string): BreakdownItem {
  return { category, name, value };
}

function makeNetWorth(over: Partial<NetWorthResponse> = {}): NetWorthResponse {
  return {
    date: over.date ?? "2026-05-20",
    assets: over.assets ?? {
      total: "1000000",
      breakdown: [
        item("investments", "Brokerage", "600000"),
        item("cash", "Cash", "150000"),
        item("vehicles", "Rolls-Royce", "250000"),
      ],
    },
    liabilities: over.liabilities ?? {
      total: "200000",
      breakdown: [item("liabilities", "Mortgage", "200000")],
    },
    netWorth: over.netWorth ?? "800000",
    // The DTO carries more fields (staleAssets etc.) but the helper only
    // touches assets + netWorth; cast keeps the fixture minimal.
  } as NetWorthResponse;
}

describe("excludeVehiclesFromNetWorth", () => {
  it("subtracts the vehicle subtotal from assets.total and netWorth", () => {
    const out = excludeVehiclesFromNetWorth(makeNetWorth());
    expect(parseFloat(out.assets.total)).toBe(750000);
    expect(parseFloat(out.netWorth)).toBe(550000);
  });

  it("removes vehicle rows from the assets breakdown", () => {
    const out = excludeVehiclesFromNetWorth(makeNetWorth());
    expect(out.assets.breakdown.some((b) => b.category === "vehicles")).toBe(false);
    expect(out.assets.breakdown).toHaveLength(2);
  });

  it("leaves liabilities untouched", () => {
    const out = excludeVehiclesFromNetWorth(makeNetWorth());
    expect(out.liabilities.total).toBe("200000");
    expect(out.liabilities.breakdown).toHaveLength(1);
  });

  it("is a strict no-op (same object reference) when there are no vehicles", () => {
    const input = makeNetWorth({
      assets: {
        total: "750000",
        breakdown: [item("investments", "Brokerage", "600000"), item("cash", "Cash", "150000")],
      },
      netWorth: "550000",
    });
    expect(excludeVehiclesFromNetWorth(input)).toBe(input);
  });

  it("sums multiple vehicle rows", () => {
    const out = excludeVehiclesFromNetWorth(
      makeNetWorth({
        assets: {
          total: "1000000",
          breakdown: [
            item("investments", "Brokerage", "600000"),
            item("vehicles", "Car", "150000"),
            item("vehicles", "Boat", "250000"),
          ],
        },
        netWorth: "800000",
      }),
    );
    expect(parseFloat(out.assets.total)).toBe(600000);
    // 800000 netWorth − (150000 + 250000) vehicles = 400000
    expect(parseFloat(out.netWorth)).toBe(400000);
    expect(out.assets.breakdown).toHaveLength(1);
  });

  it("treats a non-finite vehicle value as zero (defensive)", () => {
    const out = excludeVehiclesFromNetWorth(
      makeNetWorth({
        assets: {
          total: "600000",
          breakdown: [
            item("investments", "Brokerage", "600000"),
            item("vehicles", "Glitched", "not-a-number"),
          ],
        },
        netWorth: "600000",
      }),
    );
    // The garbage row is removed but contributes 0 to the subtraction.
    expect(parseFloat(out.assets.total)).toBe(600000);
    expect(parseFloat(out.netWorth)).toBe(600000);
    expect(out.assets.breakdown.some((b) => b.category === "vehicles")).toBe(false);
  });

  it("matches 'vehicle' singular as well as 'vehicles'", () => {
    const out = excludeVehiclesFromNetWorth(
      makeNetWorth({
        assets: {
          total: "500000",
          breakdown: [
            item("investments", "Brokerage", "400000"),
            item("vehicle", "Motorcycle", "100000"),
          ],
        },
        netWorth: "500000",
      }),
    );
    expect(parseFloat(out.assets.total)).toBe(400000);
  });

  it("does not false-positive on other categories", () => {
    const input = makeNetWorth({
      assets: {
        total: "300000",
        breakdown: [
          item("properties", "House", "200000"),
          item("collectibles", "Watch", "50000"),
          item("preciousMetals", "Gold", "50000"),
        ],
      },
      netWorth: "300000",
    });
    // No "vehicle*" category → strict no-op.
    expect(excludeVehiclesFromNetWorth(input)).toBe(input);
  });
});
