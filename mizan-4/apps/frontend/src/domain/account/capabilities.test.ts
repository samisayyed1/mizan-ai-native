import { describe, expect, it } from "vitest";

import {
  assertCapability,
  canUseCapability,
  getAccountCapabilities,
  normalizeAccountTier,
} from "./capabilities";

describe("account capabilities", () => {
  it("keeps Free signed-out-friendly with BYO AI + live quotes + news", () => {
    const free = getAccountCapabilities("free");
    expect(free.aiAssistant).toBe(true);
    expect(free.byoAi).toBe(true);
    expect(free.liveQuotes).toBe(true);
    expect(free.news).toBe(true);
    // Locked: paid-only things.
    expect(free.managedAi).toBe(false);
    expect(free.csvIngestion).toBe(false);
    expect(free.plaidSync).toBe(false);
    expect(free.zakatEngine).toBe(false);
    expect(free.weeklyAiReports).toBe(false);
  });

  it("keeps Silver private + AI + manual entry; no Plaid, no Zakat engine", () => {
    const silver = getAccountCapabilities("silver");
    expect(silver.aiAssistant).toBe(true);
    expect(silver.managedAi).toBe(true);
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

  it("maps slugs across three tiers", () => {
    expect(normalizeAccountTier()).toBe("free");
    expect(normalizeAccountTier(null)).toBe("free");
    expect(normalizeAccountTier("")).toBe("free");
    expect(normalizeAccountTier("free")).toBe("free");
    expect(normalizeAccountTier("basic")).toBe("silver");
    expect(normalizeAccountTier("silver")).toBe("silver");
    expect(normalizeAccountTier("pro")).toBe("gold");
    expect(normalizeAccountTier("gold")).toBe("gold");
    expect(normalizeAccountTier("enterprise")).toBe("gold");
  });

  it("gates capabilities centrally", () => {
    expect(canUseCapability("free", "plaidSync")).toBe(false);
    expect(canUseCapability("free", "managedAi")).toBe(false);
    expect(canUseCapability("silver", "plaidSync")).toBe(false);
    expect(canUseCapability("silver", "managedAi")).toBe(true);
    expect(canUseCapability("gold", "plaidSync")).toBe(true);
    expect(() => assertCapability("free", "managedAi")).toThrow(/managedAi/);
    expect(() => assertCapability("silver", "plaidSync")).toThrow(/plaidSync/);
  });

  it("gates the Zakat engine on Gold-only", () => {
    expect(canUseCapability("free", "zakatEngine")).toBe(false);
    expect(canUseCapability("silver", "zakatEngine")).toBe(false);
    expect(canUseCapability("gold", "zakatEngine")).toBe(true);
    expect(() => assertCapability("silver", "zakatEngine")).toThrow(/zakatEngine/);
  });
});
