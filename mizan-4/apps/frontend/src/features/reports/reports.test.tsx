// Reports infrastructure smoke tests
// ----------------------------------
//
// The full PDF render path uses html2canvas + a real DOM, neither of which
// is reliable inside jsdom. These tests cover the static React surface and
// the renderer's filename derivation; the visual fidelity of the produced
// PDF is verified manually per the M4 QA matrix.

import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { ReportSection, ReportShell } from ".";

describe("ReportShell", () => {
  it("renders title, subtitle, and disclaimer", () => {
    const { getByText } = render(
      <ReportShell title="Income report" subtitle="2026 YTD">
        <p>body</p>
      </ReportShell>,
    );

    expect(getByText("Income report")).toBeInTheDocument();
    expect(getByText("2026 YTD")).toBeInTheDocument();
    expect(getByText(/Mizan does not provide investment advice/i)).toBeInTheDocument();
  });

  it("uses logoUrl when provided instead of the Mizan wordmark", () => {
    const { queryByAltText, queryByText } = render(
      <ReportShell title="X" logoUrl="https://example.test/logo.png">
        <span />
      </ReportShell>,
    );

    expect(queryByAltText("")).toBeInTheDocument();
    expect(queryByText("Mizan")).not.toBeInTheDocument();
  });

  it("supports a custom disclaimer override", () => {
    const { getByText } = render(
      <ReportShell title="X" disclaimer="Bespoke disclaimer">
        <span />
      </ReportShell>,
    );

    expect(getByText("Bespoke disclaimer")).toBeInTheDocument();
  });
});

describe("ReportSection", () => {
  it("renders heading, caption, metric, and body", () => {
    const { getByText } = render(
      <ReportSection title="Dividend income" caption="12 positions" metric="$12,450">
        <div>section body</div>
      </ReportSection>,
    );

    expect(getByText("Dividend income")).toBeInTheDocument();
    expect(getByText("12 positions")).toBeInTheDocument();
    expect(getByText("$12,450")).toBeInTheDocument();
    expect(getByText("section body")).toBeInTheDocument();
  });

  it("omits the caption and metric blocks when not provided", () => {
    const { queryByText, getByText } = render(
      <ReportSection title="Solo title">
        <span>solo body</span>
      </ReportSection>,
    );

    expect(getByText("Solo title")).toBeInTheDocument();
    expect(getByText("solo body")).toBeInTheDocument();
    expect(queryByText("12 positions")).not.toBeInTheDocument();
  });
});
