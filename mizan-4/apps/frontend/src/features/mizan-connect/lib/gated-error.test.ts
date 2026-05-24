import { describe, expect, it } from "vitest";

import { parseGatedError } from "./gated-error";
import { upgradeCopyFor } from "./upgrade-copy";

const gatedJson = JSON.stringify({
  __gated: true,
  feature: "broker_sync",
  requiredTier: "pro",
  currentPlan: "free",
  message: "Connect your broker…",
});

describe("parseGatedError", () => {
  it("parses a raw JSON string (Tauri error channel)", () => {
    const gated = parseGatedError(gatedJson);
    expect(gated?.feature).toBe("broker_sync");
    expect(gated?.requiredTier).toBe("pro");
  });

  it("parses JSON embedded in an Error message (web adapter)", () => {
    const err = new Error(`Command failed: ${gatedJson}`);
    expect(parseGatedError(err)?.feature).toBe("broker_sync");
  });

  it("parses a plain object", () => {
    const obj = {
      __gated: true,
      feature: "max_portfolios",
      requiredTier: "basic",
      currentPlan: "free",
      message: "x",
    };
    expect(parseGatedError(obj)?.feature).toBe("max_portfolios");
  });

  it("returns null for non-gated errors", () => {
    expect(parseGatedError("network timeout")).toBeNull();
    expect(parseGatedError(new Error("boom"))).toBeNull();
    expect(parseGatedError(JSON.stringify({ error: "nope" }))).toBeNull();
    expect(parseGatedError(undefined)).toBeNull();
  });

  it("every gated feature maps to contextual upgrade copy", () => {
    for (const feature of [
      "broker_sync",
      "device_sync",
      "managed_ai",
      "max_portfolios",
      "max_asset_classes",
      "max_holdings",
      "csv_imports",
      "advanced_reports",
    ]) {
      const copy = upgradeCopyFor(feature);
      expect(copy.title.length).toBeGreaterThan(0);
      expect(copy.body.length).toBeGreaterThan(0);
    }
  });

  it("unknown feature falls back to generic copy", () => {
    const copy = upgradeCopyFor("something_new");
    expect(copy.title).toBe("Upgrade Mizan");
  });
});
