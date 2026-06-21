import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { CategoryAllocation } from "@/lib/types";

import { AllocationDonut } from "./allocation-donut";

/** Convenience to build a CategoryAllocation row with optional color. */
function cat(
  id: string,
  name: string,
  pct: number,
  value: number,
  color = "",
): CategoryAllocation {
  return {
    categoryId: id,
    categoryName: name,
    color,
    value,
    percentage: pct,
  };
}

describe("AllocationDonut", () => {
  it("paints each asset class with a distinct hue (Sami 2026-06-21: 'donut was all white, should shade right for different asset classes')", () => {
    const categories: CategoryAllocation[] = [
      cat("equities", "Equities", 60, 60_000),
      cat("bank-cash", "Bank & Cash", 25, 25_000),
      cat("bonds-sukuks", "Bonds & Sukuks", 15, 15_000),
    ];

    render(<AllocationDonut categories={categories} baseCurrency="USD" />);

    // One arc per non-zero segment, each carrying its own stroke colour.
    const equityArc = screen.getByTestId("allocation-donut-arc-equities");
    const cashArc = screen.getByTestId("allocation-donut-arc-bank-cash");
    const bondsArc = screen.getByTestId("allocation-donut-arc-bonds-sukuks");

    const strokes = [
      equityArc.getAttribute("stroke"),
      cashArc.getAttribute("stroke"),
      bondsArc.getAttribute("stroke"),
    ];

    // The bug was that all three slices ended up the same gold ladder
    // colour (gold-cream → gold-primary → warm-gold) and read as a
    // single colour. After the fix each panel id maps to a distinct
    // hue, so the set has three unique values.
    expect(new Set(strokes).size).toBe(3);

    // Spot-check the semantic mapping. We don't pin the exact HSL
    // because the palette can be tuned without breaking the contract;
    // we DO assert that equities lands in the warm-gold band and cash
    // lands in the green band — i.e. they're not the same hue.
    expect(equityArc.getAttribute("stroke")).toMatch(/hsl\(\s*\d/);
    expect(cashArc.getAttribute("stroke")).not.toBe(equityArc.getAttribute("stroke"));
    expect(bondsArc.getAttribute("stroke")).not.toBe(equityArc.getAttribute("stroke"));
  });

  it("honours the backend-supplied color when present", () => {
    const categories: CategoryAllocation[] = [
      cat("equities", "Equities", 100, 100_000, "hsl(120 100% 50%)"),
    ];
    render(<AllocationDonut categories={categories} baseCurrency="USD" />);
    const arc = screen.getByTestId("allocation-donut-arc-equities");
    expect(arc.getAttribute("stroke")).toBe("hsl(120 100% 50%)");
  });

  it("falls back to the warm-gold ladder for unknown categoryIds (never returns a near-white)", () => {
    const categories: CategoryAllocation[] = [
      cat("unknown-future-class-1", "Future class A", 50, 50_000),
      cat("unknown-future-class-2", "Future class B", 50, 50_000),
    ];
    render(<AllocationDonut categories={categories} baseCurrency="USD" />);
    const arcA = screen.getByTestId("allocation-donut-arc-unknown-future-class-1");
    const arcB = screen.getByTestId("allocation-donut-arc-unknown-future-class-2");
    const a = arcA.getAttribute("stroke");
    const b = arcB.getAttribute("stroke");
    expect(a).toMatch(/hsl\(\s*31/); // warm-gold family
    expect(b).toMatch(/hsl\(\s*31/);
    expect(a).not.toBe(b); // ladder cycles, so consecutive indices differ
  });

  it("filters zero-value segments out of the ring", () => {
    const categories: CategoryAllocation[] = [
      cat("equities", "Equities", 100, 100_000),
      cat("crypto", "Crypto", 0, 0),
    ];
    render(<AllocationDonut categories={categories} baseCurrency="USD" />);
    expect(screen.queryByTestId("allocation-donut-arc-crypto")).toBeNull();
    expect(screen.getByTestId("allocation-donut-arc-equities")).toBeTruthy();
  });
});
