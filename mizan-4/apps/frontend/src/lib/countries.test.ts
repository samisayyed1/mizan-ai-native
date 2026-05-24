import { describe, expect, it } from "vitest";
import { COUNTRIES, COUNTRY_SELECT_OPTIONS, countryFlag } from "./countries";

describe("countryFlag", () => {
  it("derives the correct regional-indicator flag for a valid code", () => {
    // 🇺🇸 = U+1F1FA U+1F1F8
    expect(countryFlag("US")).toBe("\u{1F1FA}\u{1F1F8}");
    // 🇸🇬
    expect(countryFlag("SG")).toBe("\u{1F1F8}\u{1F1EC}");
  });

  it("is case-insensitive", () => {
    expect(countryFlag("us")).toBe(countryFlag("US"));
  });

  it("returns empty string for malformed codes (so the UI just shows the name)", () => {
    expect(countryFlag("")).toBe("");
    expect(countryFlag("U")).toBe("");
    expect(countryFlag("USA")).toBe("");
    expect(countryFlag("1!")).toBe("");
  });
});

describe("COUNTRIES list", () => {
  it("includes every country Feroz named on the May-17 call", () => {
    const names = new Set(COUNTRIES.map((c) => c.name));
    for (const expected of [
      "Saudi Arabia",
      "United Arab Emirates",
      "Singapore",
      "India",
      "Pakistan",
      "United States",
    ]) {
      expect(names.has(expected)).toBe(true);
    }
  });

  it("has unique ISO codes and unique names", () => {
    const codes = COUNTRIES.map((c) => c.code);
    const names = COUNTRIES.map((c) => c.name);
    expect(new Set(codes).size).toBe(codes.length);
    expect(new Set(names).size).toBe(names.length);
  });

  it("uses well-formed alpha-2 codes throughout", () => {
    expect(COUNTRIES.every((c) => /^[A-Z]{2}$/.test(c.code))).toBe(true);
  });
});

describe("COUNTRY_SELECT_OPTIONS", () => {
  it("maps every country to a { value: name, label: flag+name } option", () => {
    expect(COUNTRY_SELECT_OPTIONS).toHaveLength(COUNTRIES.length);
    const india = COUNTRY_SELECT_OPTIONS.find((o) => o.value === "India")!;
    expect(india.value).toBe("India");
    expect(india.label).toContain("India");
    expect(india.label).toContain(countryFlag("IN"));
  });
});
