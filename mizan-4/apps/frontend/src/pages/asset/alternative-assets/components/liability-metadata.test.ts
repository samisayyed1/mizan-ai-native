import { describe, expect, it } from "vitest";
import { AlternativeAssetKind } from "@/lib/types";
import {
  formValuesToMetadata,
  getDefaultDetailsFormValues,
  type LiabilityDetailsFormValues,
} from "./asset-details-sheet-schema";

/**
 * Round-trip coverage for the liability fields added in Feroz step E
 * (§21 balance date + loan duration, §23 EMI). The risk is the
 * metadata <-> form mapping silently dropping or mistyping a field.
 */
describe("liability metadata", () => {
  describe("formValuesToMetadata", () => {
    it("writes all liability fields including balance date, duration and EMI", () => {
      const values: LiabilityDetailsFormValues = {
        kind: AlternativeAssetKind.LIABILITY,
        name: "Beach House Mortgage",
        purchasePrice: null,
        purchaseDate: null,
        notes: null,
        liabilityType: "mortgage",
        originalAmount: 500000,
        originationDate: new Date(2021, 0, 1),
        interestRate: 4.25,
        balanceDate: new Date(2026, 4, 17),
        loanDurationYears: 25,
        monthlyPayment: 2700,
        linkedAssetId: null,
      };
      const meta = formValuesToMetadata(values);
      expect(meta.sub_type).toBe("mortgage");
      expect(meta.original_amount).toBe("500000");
      expect(meta.origination_date).toBe("2021-01-01");
      expect(meta.interest_rate).toBe("4.25");
      expect(meta.balance_date).toBe("2026-05-17");
      expect(meta.loan_duration_years).toBe("25");
      expect(meta.monthly_payment).toBe("2700");
    });

    it("omits the new optional fields when they are absent", () => {
      const values: LiabilityDetailsFormValues = {
        kind: AlternativeAssetKind.LIABILITY,
        name: "Credit Card",
        purchasePrice: null,
        purchaseDate: null,
        notes: null,
        liabilityType: "credit_card",
        originalAmount: null,
        originationDate: null,
        interestRate: null,
        balanceDate: null,
        loanDurationYears: null,
        monthlyPayment: null,
        linkedAssetId: null,
      };
      const meta = formValuesToMetadata(values);
      expect(meta.sub_type).toBe("credit_card");
      expect("balance_date" in meta).toBe(false);
      expect("loan_duration_years" in meta).toBe(false);
      expect("monthly_payment" in meta).toBe(false);
    });
  });

  describe("getDefaultDetailsFormValues", () => {
    it("reads the new liability fields back from metadata", () => {
      const values = getDefaultDetailsFormValues(AlternativeAssetKind.LIABILITY, "Mortgage", {
        sub_type: "mortgage",
        original_amount: "500000",
        origination_date: "2021-01-01",
        interest_rate: "4.25",
        balance_date: "2026-05-17",
        loan_duration_years: "25",
        monthly_payment: "2700",
      }) as LiabilityDetailsFormValues;

      expect(values.liabilityType).toBe("mortgage");
      expect(values.balanceDate?.getFullYear()).toBe(2026);
      expect(values.loanDurationYears).toBe(25);
      expect(values.monthlyPayment).toBe(2700);
    });

    it("defaults the new fields to null when metadata is empty", () => {
      const values = getDefaultDetailsFormValues(AlternativeAssetKind.LIABILITY, "Loan", {
        sub_type: "auto_loan",
      }) as LiabilityDetailsFormValues;
      expect(values.balanceDate).toBeNull();
      expect(values.loanDurationYears).toBeNull();
      expect(values.monthlyPayment).toBeNull();
    });
  });

  it("round-trips metadata → form → metadata without loss", () => {
    const original: Record<string, string> = {
      sub_type: "auto_loan",
      original_amount: "30000",
      origination_date: "2023-06-15",
      interest_rate: "7.5",
      balance_date: "2026-05-01",
      loan_duration_years: "5",
      monthly_payment: "600",
    };
    const form = getDefaultDetailsFormValues(
      AlternativeAssetKind.LIABILITY,
      "Car Loan",
      original,
    ) as LiabilityDetailsFormValues;
    const back = formValuesToMetadata(form);

    expect(back.sub_type).toBe("auto_loan");
    expect(back.original_amount).toBe("30000");
    expect(back.origination_date).toBe("2023-06-15");
    expect(back.interest_rate).toBe("7.5");
    expect(back.balance_date).toBe("2026-05-01");
    expect(back.loan_duration_years).toBe("5");
    expect(back.monthly_payment).toBe("600");
  });
});
