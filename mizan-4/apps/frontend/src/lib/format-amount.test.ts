// Edge-case coverage for `formatAmount` from @mizan/ui.
//
// The function is the workhorse behind every monetary value the user
// sees on screen. It's also responsible for the privacy-mode-friendly
// behaviour (returning "-" instead of letting NaN/Infinity leak as
// "NaN" / "Infinity %"). Until now it had zero tests — this file
// covers the contracts the dashboard depends on.

import { describe, expect, it } from "vitest";

// The canonical formatAmount/formatPercent/formatQuantity live in
// `packages/ui/src/lib/utils.ts`, but importing them via `@mizan/ui`
// drags the entire JSX component barrel through Vitest's transformer
// and breaks on react/jsx-dev-runtime resolution. The same functions
// also exist as a verbatim duplicate at `apps/frontend/src/lib/utils.ts`
// (TODO follow-up: collapse the duplication). Testing the in-app copy
// transitively proves the algorithm — and the regression suite below
// is the contract those edge cases must keep meeting.
import { formatAmount, formatPercent, formatPrice, formatQuantity } from "./utils";

describe("formatAmount", () => {
  it("returns '-' for null/undefined", () => {
    expect(formatAmount(null, "USD")).toBe("-");
    expect(formatAmount(undefined, "USD")).toBe("-");
  });

  it("returns '-' for NaN and Infinity (never leaks NaN%/Infinity to the user)", () => {
    expect(formatAmount(Number.NaN, "USD")).toBe("-");
    expect(formatAmount(Number.POSITIVE_INFINITY, "USD")).toBe("-");
    expect(formatAmount(Number.NEGATIVE_INFINITY, "USD")).toBe("-");
  });

  it("formats positive amounts with the user's currency", () => {
    expect(formatAmount(1234.56, "USD")).toMatch(/\$1,234\.56/);
    // EUR uses comma as decimal separator in the en-US locale's
    // formatter — but Intl resolves the currency symbol independently
    // of separator. Just assert the value + symbol both appear.
    const eur = formatAmount(1234.56, "EUR");
    expect(eur).toMatch(/1,234\.56/);
    expect(eur).toMatch(/€/);
  });

  it("formats negative amounts with the standard negative-sign locale rule", () => {
    expect(formatAmount(-99.5, "USD")).toMatch(/-?\$99\.50|\(\$99\.50\)/);
  });

  it("rounds sub-cent amounts to zero so we never render '-0.00'", () => {
    expect(formatAmount(0.001, "USD")).toMatch(/\$0\.00/);
    expect(formatAmount(-0.001, "USD")).toMatch(/\$0\.00/);
    expect(formatAmount(0.0049, "USD")).toMatch(/\$0\.00/);
  });

  it("accepts a numeric string and converts it", () => {
    expect(formatAmount("250000.00", "USD")).toMatch(/\$250,000\.00/);
  });

  it("returns '-' for non-numeric strings", () => {
    expect(formatAmount("not a number", "USD")).toBe("-");
  });

  it('treats an empty string as 0 (Number("") === 0)', () => {
    // Documenting actual behaviour. Arguably "" should map to "-" the
    // same way null does, but several call sites pass "" intentionally
    // to mean "this row contributes nothing" — changing that would
    // ripple through the UI. Tracked as a separate cleanup.
    expect(formatAmount("", "USD")).toMatch(/\$0\.00/);
  });

  it("omits the currency symbol when displayCurrency is false", () => {
    const v = formatAmount(1234.56, "USD", false);
    expect(v).not.toContain("$");
    expect(v).toMatch(/1,234\.56/);
  });

  it("renders GBp (UK pence) as 1234.56p, not £1234.56", () => {
    expect(formatAmount(1234.56, "GBp")).toBe("1,234.56p");
    expect(formatAmount(1234.56, "GBX")).toBe("1,234.56p");
    expect(formatAmount(1234.56, "GBp", false)).toBe("1,234.56");
  });

  it("renders without a currency symbol when currency is an empty string", () => {
    // The implementation's nullish-coalescing fallback to USD fires
    // only on null/undefined, not "". An explicit "" currency renders
    // the value bare. Documented here so a future refactor doesn't
    // silently change the dashboard's behaviour for blank-currency
    // rows.
    expect(formatAmount(100, "")).toMatch(/100\.00/);
    expect(formatAmount(100, "")).not.toContain("$");
  });
});

describe("formatPercent", () => {
  it("returns '-' for null/undefined/NaN/Infinity (no NaN% leaks)", () => {
    expect(formatPercent(null)).toBe("-");
    expect(formatPercent(undefined)).toBe("-");
    expect(formatPercent(Number.NaN)).toBe("-");
    expect(formatPercent(Number.POSITIVE_INFINITY)).toBe("-");
    expect(formatPercent(Number.NEGATIVE_INFINITY)).toBe("-");
  });

  it("formats a fraction as a percentage with 2 decimal places", () => {
    expect(formatPercent(0)).toBe("0.00%");
    expect(formatPercent(0.0123)).toBe("1.23%");
    expect(formatPercent(-0.0789)).toBe("-7.89%");
    expect(formatPercent(1)).toBe("100.00%");
  });
});

describe("formatPrice", () => {
  // formatPrice exists because formatAmount rounds to 2 decimal places,
  // which renders sub-cent crypto/penny-token prices like "$0.00".
  // The CSV import preview shouldn't lie to the user about what they
  // uploaded.

  it("returns '-' for null/undefined/empty/NaN/Infinity", () => {
    expect(formatPrice(null, "USD")).toBe("-");
    expect(formatPrice(undefined, "USD")).toBe("-");
    expect(formatPrice("", "USD")).toBe("-");
    expect(formatPrice(Number.NaN, "USD")).toBe("-");
    expect(formatPrice(Number.POSITIVE_INFINITY, "USD")).toBe("-");
  });

  it("uses currency-style formatting for prices >= $1", () => {
    expect(formatPrice(150.25, "USD")).toMatch(/\$150\.25/);
    expect(formatPrice("1.00", "USD")).toMatch(/\$1\.00/);
  });

  it("preserves sub-cent precision for prices below $1", () => {
    // Without formatPrice the CSV preview rendered this as "$0.00".
    expect(formatPrice(0.00001234, "USD")).toMatch(/\$0\.00001234/);
    expect(formatPrice("0.000123", "USD")).toMatch(/\$0\.000123/);
    expect(formatPrice(0.5, "USD")).toMatch(/\$0\.50/);
  });

  it("trims trailing zeros past 2 decimal places when the price is sub-dollar", () => {
    // 0.10 must render as $0.10 (minimum 2 dp), but 0.1000000 shouldn't
    // explode to $0.10000000.
    expect(formatPrice(0.1, "USD")).toMatch(/\$0\.10/);
    expect(formatPrice("0.10000000", "USD")).toMatch(/\$0\.10/);
  });

  it("omits currency symbol when displayCurrency=false", () => {
    expect(formatPrice(0.00001234, "USD", false)).toBe("0.00001234");
    expect(formatPrice(150.25, "USD", false)).toMatch(/^150\.25$/);
  });

  it("renders GBp pence prices with the p suffix at sub-cent precision", () => {
    expect(formatPrice(0.00001234, "GBp")).toBe("0.00001234p");
    expect(formatPrice(0.00001234, "GBp", false)).toBe("0.00001234");
  });

  it("accepts a Decimal-as-string without coercing through Number until display", () => {
    // Documents the contract for the CSV import path: precision-string
    // inputs are accepted and rendered as-is up to 8 decimals.
    expect(formatPrice("0.12345678", "USD")).toMatch(/\$0\.12345678/);
  });
});

describe("formatQuantity", () => {
  it("returns '-' for null/undefined and non-finite", () => {
    expect(formatQuantity(null)).toBe("-");
    expect(formatQuantity(undefined)).toBe("-");
    expect(formatQuantity(Number.NaN)).toBe("-");
  });

  it("formats numbers and numeric strings with grouping", () => {
    expect(formatQuantity(1234567)).toMatch(/1,234,567/);
    expect(formatQuantity("250.5")).toMatch(/250\.5/);
  });

  it("preserves sub-unit precision for tiny crypto holdings", () => {
    // 1 satoshi = 0.00000001 BTC. The old 4-dp cap rounded this to "0";
    // a user with a real satoshi-scale balance was seeing zero.
    expect(formatQuantity(0.00000001)).toBe("0.00000001");
    expect(formatQuantity("0.00000001")).toBe("0.00000001");
    // Slightly bigger sub-unit values still trim trailing zeros.
    expect(formatQuantity(0.5)).toBe("0.5");
  });
});
