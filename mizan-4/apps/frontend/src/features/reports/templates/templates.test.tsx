// Pro report template smoke tests
// --------------------------------
//
// Each template is a pure render of caller-supplied data into the
// `<ReportShell>` harness. Tests assert that the headline number, the
// section headings, and the key data rows make it onto the DOM. PDF
// fidelity is verified manually per the M4 QA matrix.

import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import {
  HealthReport,
  IncomeReport,
  PayoffReport,
  RentalReport,
  type HealthReportData,
  type IncomeReportData,
  type PayoffReportData,
  type RentalReportData,
} from "..";

describe("IncomeReport", () => {
  const data: IncomeReportData = {
    periodLabel: "YTD 2026",
    currency: "USD",
    totalIncome: 12345,
    monthlyAverage: 1029,
    yoyGrowth: 0.085,
    byType: { DIVIDEND: 8000, INTEREST: 2345, RENTAL: 2000 },
    topAssets: [
      { symbol: "AAPL", name: "Apple Inc.", total: 4500, type: "DIVIDEND" },
      { symbol: "MSFT", total: 3500, type: "DIVIDEND" },
    ],
    byMonth: { "2026-01": 1100, "2026-02": 1200, "2026-03": 1050 },
  };

  it("renders title, total, and top assets", () => {
    const { getByText, getAllByText } = render(<IncomeReport data={data} />);
    expect(getByText("Income report")).toBeInTheDocument();
    expect(getByText("YTD 2026", { selector: "span" })).toBeInTheDocument();
    expect(getAllByText(/AAPL/).length).toBeGreaterThan(0);
    expect(getByText("Apple Inc.", { exact: false })).toBeInTheDocument();
    expect(getAllByText("DIVIDEND").length).toBeGreaterThan(0);
  });

  it("shows YoY growth with a sign", () => {
    const { getByText } = render(<IncomeReport data={data} />);
    expect(getByText(/\+8\.5%/)).toBeInTheDocument();
  });
});

describe("RentalReport", () => {
  it("renders one section per property and a totals header", () => {
    const data: RentalReportData = {
      currency: "USD",
      periodLabel: "2026",
      properties: [
        {
          label: "47 Maple St",
          tenantName: "Alice",
          monthlyRent: 2200,
          rentalStartDate: "2025-01-01",
        },
        { label: "Loft 3B", monthlyRent: 1800 },
      ],
    };
    const { getByText } = render(<RentalReport data={data} />);
    expect(getByText("Rental income")).toBeInTheDocument();
    expect(getByText("47 Maple St")).toBeInTheDocument();
    expect(getByText("Loft 3B")).toBeInTheDocument();
    expect(getByText(/Tenant: Alice/)).toBeInTheDocument();
  });

  it("renders an empty-state when no rentals exist", () => {
    const { getByText } = render(
      <RentalReport data={{ currency: "USD", periodLabel: "2026", properties: [] }} />,
    );
    expect(getByText("No rented properties")).toBeInTheDocument();
  });
});

describe("PayoffReport", () => {
  it("renders summary metrics and first-12-month schedule", () => {
    const schedule = Array.from({ length: 24 }, (_, i) => ({
      period: i + 1,
      dueDate: `2026-${String((i % 12) + 1).padStart(2, "0")}-01`,
      payment: 1000,
      interest: 100 - i * 4,
      principal: 900 + i * 4,
      balanceAfter: Math.max(0, 24000 - (i + 1) * 1000),
    }));
    const data: PayoffReportData = {
      liabilityName: "Wells Fargo mortgage",
      currency: "USD",
      monthlyPayment: 1000,
      totalPaid: 24000,
      totalInterest: 1200,
      payoffDate: "2027-12-01",
      numberOfPayments: 24,
      schedule,
      notes: ["Schedule assumes fixed rate."],
    };
    const { getByText } = render(<PayoffReport data={data} />);
    expect(getByText("Wells Fargo mortgage")).toBeInTheDocument();
    expect(getByText("Payoff summary")).toBeInTheDocument();
    expect(getByText("First 12 months")).toBeInTheDocument();
    expect(getByText("Final stretch")).toBeInTheDocument();
    expect(getByText(/Schedule assumes fixed rate\./)).toBeInTheDocument();
  });
});

describe("HealthReport", () => {
  it("renders composite score and per-driver breakdown", () => {
    const data: HealthReportData = {
      currency: "USD",
      score: 72,
      drivers: [
        {
          id: "concentration",
          label: "Concentration",
          score: 80,
          metric: 0.2,
          note: "Top position is 20% of portfolio.",
        },
        {
          id: "fxExposure",
          label: "FX exposure",
          score: 65,
          metric: 0.35,
          note: "35% in foreign currency.",
        },
      ],
      worstDriver: {
        id: "fxExposure",
        label: "FX exposure",
        score: 65,
        note: "35% in foreign currency.",
      },
      notes: ["Heuristic only."],
    };
    const { getByText, getAllByText } = render(<HealthReport data={data} />);
    expect(getByText("Portfolio health")).toBeInTheDocument();
    expect(getByText("72")).toBeInTheDocument();
    expect(getAllByText(/FX exposure/).length).toBeGreaterThan(0);
    expect(getAllByText(/Concentration/).length).toBeGreaterThan(0);
    expect(getByText(/Heuristic only/)).toBeInTheDocument();
  });

  it("renders empty-state when score is null", () => {
    const { getByText } = render(
      <HealthReport
        data={{
          currency: "USD",
          score: null,
          drivers: [],
          worstDriver: null,
          notes: [],
        }}
      />,
    );
    expect(getByText(/no positions yet/i)).toBeInTheDocument();
  });
});
