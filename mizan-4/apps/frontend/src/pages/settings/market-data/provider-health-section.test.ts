import { describe, expect, it } from "vitest";

import { circuitStateVisual } from "./provider-health-helpers";

describe("circuitStateVisual", () => {
  it("maps Closed to an operational success badge", () => {
    expect(circuitStateVisual("Closed")).toEqual({
      variant: "success",
      label: "Operational",
    });
  });

  it("maps HalfOpen to a recovering warning badge", () => {
    expect(circuitStateVisual("HalfOpen")).toEqual({
      variant: "warning",
      label: "Recovering",
    });
  });

  it("maps Open to a failing destructive badge", () => {
    expect(circuitStateVisual("Open")).toEqual({
      variant: "destructive",
      label: "Failing",
    });
  });
});
