import { describe, expect, it } from "vitest";

import { isStateChangingAction, parseMizanAiAction } from "./mizan-ai-actions";

describe("Mizan AI actions", () => {
  it("validates conversational asset creation", () => {
    const action = parseMizanAiAction({
      action: "ADD_ASSET",
      asset_class: "precious_metals",
      source: "conversational_ingest",
      confidence: 0.92,
      requires_review: true,
      payload: { amount: 15.5, unit: "oz", metal: "gold" },
    });
    expect(action.action).toBe("ADD_ASSET");
    expect(isStateChangingAction(action)).toBe(true);
  });

  it("forces review for low confidence state-changing drafts", () => {
    const action = parseMizanAiAction({
      action: "ADD_ASSET",
      asset_class: "property",
      source: "conversational_ingest",
      confidence: 0.42,
      requires_review: false,
      payload: {},
    });
    expect(action.requires_review).toBe(true);
  });

  it("rejects unsupported asset classes", () => {
    expect(() =>
      parseMizanAiAction({
        action: "ADD_ASSET",
        asset_class: "guesswork",
        source: "conversational_ingest",
        confidence: 0.9,
        requires_review: true,
        payload: {},
      }),
    ).toThrow();
  });
});
