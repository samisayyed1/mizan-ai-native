import { describe, expect, it } from "vitest";

import {
  assertCapability,
  canUseCapability,
  getAccountCapabilities,
  normalizeAccountTier,
} from "./capabilities";

describe("account capabilities", () => {
  it("keeps Silver private and complete without Plaid", () => {
    const silver = getAccountCapabilities("silver");
    expect(silver.aiAssistant).toBe(true);
    expect(silver.csvIngestion).toBe(true);
    expect(silver.zakatEngine).toBe(true);
    expect(silver.plaidSync).toBe(false);
    expect(silver.weeklyAiReports).toBe(false);
  });

  it("unlocks Gold monitoring and Plaid sync", () => {
    const gold = getAccountCapabilities("gold");
    expect(gold.plaidSync).toBe(true);
    expect(gold.liveLiabilityTracking).toBe(true);
    expect(gold.backgroundMonitoring).toBe(true);
    expect(gold.proactiveAlerts).toBe(true);
  });

  it("maps legacy slugs to the two-tier product", () => {
    expect(normalizeAccountTier("free")).toBe("silver");
    expect(normalizeAccountTier("basic")).toBe("silver");
    expect(normalizeAccountTier("silver")).toBe("silver");
    expect(normalizeAccountTier("pro")).toBe("gold");
    expect(normalizeAccountTier("enterprise")).toBe("gold");
  });

  it("gates capabilities centrally", () => {
    expect(canUseCapability("silver", "plaidSync")).toBe(false);
    expect(canUseCapability("gold", "plaidSync")).toBe(true);
    expect(() => assertCapability("silver", "plaidSync")).toThrow(/plaidSync/);
  });
});
