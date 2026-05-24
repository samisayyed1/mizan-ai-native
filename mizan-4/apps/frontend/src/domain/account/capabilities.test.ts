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
    expect(silver.zakatEngine).toBe(false);
    expect(silver.plaidSync).toBe(false);
    expect(silver.weeklyAiReports).toBe(false);
  });

  it("unlocks Gold monitoring, Plaid sync, and the Zakat engine", () => {
    const gold = getAccountCapabilities("gold");
    expect(gold.zakatEngine).toBe(true);
    expect(gold.plaidSync).toBe(true);
    expect(gold.liveLiabilityTracking).toBe(true);
    expect(gold.backgroundMonitoring).toBe(true);
    expect(gold.proactiveAlerts).toBe(true);
  });

  it("gates the Zakat engine on Gold-only", () => {
    expect(canUseCapability("silver", "zakatEngine")).toBe(false);
    expect(canUseCapability("gold", "zakatEngine")).toBe(true);
    expect(() => assertCapability("silver", "zakatEngine")).toThrow(/zakatEngine/);
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
