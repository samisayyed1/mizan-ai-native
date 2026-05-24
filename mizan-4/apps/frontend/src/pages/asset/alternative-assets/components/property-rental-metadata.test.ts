import { describe, expect, it } from "vitest";
import { AlternativeAssetKind } from "@/lib/types";
import {
  formValuesToMetadata,
  getDefaultDetailsFormValues,
  type PropertyDetailsFormValues,
} from "./asset-details-sheet-schema";

/**
 * Round-trip coverage for the property rental fields (Feroz step D §16).
 * The risk here is the metadata <-> form mapping silently dropping or
 * mistyping a field, so we assert both directions explicitly.
 */
describe("property rental metadata", () => {
  describe("formValuesToMetadata", () => {
    it("writes is_rented=true and all rental fields when rented", () => {
      const values: PropertyDetailsFormValues = {
        kind: AlternativeAssetKind.PROPERTY,
        name: "Beach House",
        purchasePrice: null,
        purchaseDate: null,
        notes: null,
        address: null,
        propertyType: "rental",
        isRented: true,
        rentalAmount: 2500,
        rentalFrequency: "monthly",
        rentalStartDate: new Date(2021, 0, 15),
        rentalEndDate: new Date(2026, 11, 31),
      };
      const meta = formValuesToMetadata(values);
      expect(meta.is_rented).toBe("true");
      expect(meta.rental_amount).toBe("2500");
      expect(meta.rental_frequency).toBe("monthly");
      expect(meta.rental_start_date).toBe("2021-01-15");
      expect(meta.rental_end_date).toBe("2026-12-31");
    });

    it("omits the end date when the tenancy is ongoing (perpetual)", () => {
      const values: PropertyDetailsFormValues = {
        kind: AlternativeAssetKind.PROPERTY,
        name: "City Apartment",
        purchasePrice: null,
        purchaseDate: null,
        notes: null,
        address: null,
        propertyType: null,
        isRented: true,
        rentalAmount: 1800,
        rentalFrequency: "annual",
        rentalStartDate: new Date(2024, 5, 1),
        rentalEndDate: null,
      };
      const meta = formValuesToMetadata(values);
      expect(meta.is_rented).toBe("true");
      expect(meta.rental_frequency).toBe("annual");
      expect("rental_end_date" in meta).toBe(false);
    });

    it("writes is_rented=false and no rental detail fields when not rented", () => {
      const values: PropertyDetailsFormValues = {
        kind: AlternativeAssetKind.PROPERTY,
        name: "Primary Home",
        purchasePrice: null,
        purchaseDate: null,
        notes: null,
        address: null,
        propertyType: "residence",
        isRented: false,
        rentalAmount: 9999,
        rentalFrequency: "monthly",
        rentalStartDate: new Date(2020, 0, 1),
        rentalEndDate: null,
      };
      const meta = formValuesToMetadata(values);
      // Explicit false so toggling off reliably clears the rented state.
      expect(meta.is_rented).toBe("false");
      expect("rental_amount" in meta).toBe(false);
      expect("rental_frequency" in meta).toBe(false);
      expect("rental_start_date" in meta).toBe(false);
    });
  });

  describe("getDefaultDetailsFormValues", () => {
    it("reads rental fields back out of metadata", () => {
      const values = getDefaultDetailsFormValues(AlternativeAssetKind.PROPERTY, "Beach House", {
        is_rented: "true",
        rental_amount: "2500",
        rental_frequency: "monthly",
        rental_start_date: "2021-01-15",
        rental_end_date: "2026-12-31",
      }) as PropertyDetailsFormValues;

      expect(values.isRented).toBe(true);
      expect(values.rentalAmount).toBe(2500);
      expect(values.rentalFrequency).toBe("monthly");
      expect(values.rentalStartDate?.getFullYear()).toBe(2021);
      expect(values.rentalEndDate?.getFullYear()).toBe(2026);
    });

    it("defaults isRented to false and rental fields to null when metadata is empty", () => {
      const values = getDefaultDetailsFormValues(
        AlternativeAssetKind.PROPERTY,
        "Empty",
        {},
      ) as PropertyDetailsFormValues;

      expect(values.isRented).toBe(false);
      expect(values.rentalAmount).toBeNull();
      expect(values.rentalFrequency).toBeNull();
      expect(values.rentalStartDate).toBeNull();
      expect(values.rentalEndDate).toBeNull();
    });

    it("treats a missing end date as ongoing (null, not a crash)", () => {
      const values = getDefaultDetailsFormValues(AlternativeAssetKind.PROPERTY, "Ongoing", {
        is_rented: "true",
        rental_amount: "1000",
        rental_frequency: "monthly",
        rental_start_date: "2024-06-01",
      }) as PropertyDetailsFormValues;

      expect(values.isRented).toBe(true);
      expect(values.rentalEndDate).toBeNull();
    });
  });

  it("round-trips metadata → form → metadata without loss (rented, perpetual)", () => {
    const original: Record<string, string> = {
      is_rented: "true",
      rental_amount: "3200",
      rental_frequency: "monthly",
      rental_start_date: "2022-03-01",
      sub_type: "rental",
    };
    const form = getDefaultDetailsFormValues(
      AlternativeAssetKind.PROPERTY,
      "Round Trip",
      original,
    ) as PropertyDetailsFormValues;
    const back = formValuesToMetadata(form);

    expect(back.is_rented).toBe("true");
    expect(back.rental_amount).toBe("3200");
    expect(back.rental_frequency).toBe("monthly");
    expect(back.rental_start_date).toBe("2022-03-01");
    expect("rental_end_date" in back).toBe(false);
  });
});
