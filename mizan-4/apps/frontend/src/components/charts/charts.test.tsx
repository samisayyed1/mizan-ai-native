/**
 * Vitest unit suite for the chart primitives — Track A PR-A3.
 *
 * Focus: contract verification per ADR 0019 §"Shared component API".
 *   - Empty-state rendering for every primitive (ADR requires honest
 *     empty state instead of blank canvas)
 *   - Aria label propagation (every chart MUST be screen-reader labeled)
 *   - Datum filtering (negative donut slices, NaN sparkline points, etc.)
 *   - Tap callback wiring
 *
 * Performance contract (<200ms cached paint per §A19) is verified via
 * Playwright in `apps/frontend/tests/e2e/charts.spec.ts` once that suite
 * lands in PR-A4 with the dashboard composition test.
 *
 * Recharts ResponsiveContainer needs an explicit width when rendered
 * inside jsdom — we pass `width` directly to every chart in the test
 * fixtures so Recharts doesn't bail on `0×0` container measurement.
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { Bar, Donut, Heatmap, Sparkline } from "./index";
import type { BarDatum, DonutDatum, HeatmapDatum, SparklineDatum } from "./types";

const ariaLabel = "test-chart";
const W = 320;
const H = 160;

describe("Donut", () => {
  it("renders an empty state for [] data", () => {
    render(<Donut data={[]} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-donut-empty")).toBeTruthy();
    expect(screen.getByRole("img", { name: ariaLabel })).toBeTruthy();
  });

  it("renders an empty state when every slice sums to zero", () => {
    const data: DonutDatum[] = [
      { label: "A", value: 0 },
      { label: "B", value: 0 },
    ];
    render(<Donut data={data} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-donut-empty")).toBeTruthy();
  });

  it("filters out negative + non-finite slices but renders the rest", () => {
    const data: DonutDatum[] = [
      { label: "Good", value: 30 },
      { label: "Bad-negative", value: -10 },
      { label: "Bad-NaN", value: Number.NaN },
      { label: "OK", value: 20 },
    ];
    render(<Donut data={data} ariaLabel={ariaLabel} width={W} height={H} />);
    // The active chart container renders (not the empty-state).
    expect(screen.getByTestId("chart-donut")).toBeTruthy();
    expect(screen.queryByTestId("chart-donut-empty")).toBeNull();
  });

  it("renders the centerLabel slot when provided", () => {
    const data: DonutDatum[] = [
      { label: "A", value: 60 },
      { label: "B", value: 40 },
    ];
    render(
      <Donut data={data} ariaLabel={ariaLabel} width={W} height={H} centerLabel={<span>$100K</span>} />,
    );
    expect(screen.getByTestId("chart-donut-center-label")).toBeTruthy();
    expect(screen.getByText("$100K")).toBeTruthy();
  });

  it("requires an ariaLabel (TypeScript) and surfaces it on the role=img", () => {
    const data: DonutDatum[] = [{ label: "A", value: 100 }];
    render(<Donut data={data} ariaLabel="allocation by class" width={W} height={H} />);
    expect(screen.getByRole("img", { name: "allocation by class" })).toBeTruthy();
  });
});

describe("Bar", () => {
  it("renders an empty state for [] data", () => {
    render(<Bar data={[]} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-bar-empty")).toBeTruthy();
  });

  it("renders bars when data is provided", () => {
    const data: BarDatum[] = [
      { label: "AAPL", value: 100 },
      { label: "MSFT", value: 80 },
      { label: "GOOG", value: 60 },
    ];
    render(<Bar data={data} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-bar")).toBeTruthy();
  });

  it("accepts horizontal orientation without crashing", () => {
    const data: BarDatum[] = [{ label: "long-category-name", value: 10 }];
    render(
      <Bar data={data} ariaLabel={ariaLabel} width={W} height={H} orientation="horizontal" />,
    );
    expect(screen.getByTestId("chart-bar")).toBeTruthy();
  });

  it("uses divergent palette for gain/loss data (smoke test)", () => {
    const data: BarDatum[] = [
      { label: "Gainer", value: 12 },
      { label: "Loser", value: -8 },
      { label: "Flat", value: 0 },
    ];
    render(<Bar data={data} ariaLabel={ariaLabel} width={W} height={H} palette="divergent" />);
    expect(screen.getByTestId("chart-bar")).toBeTruthy();
  });
});

describe("Sparkline", () => {
  it("renders the placeholder em-dash with fewer than 2 points", () => {
    render(<Sparkline data={[]} ariaLabel={ariaLabel} width={W} height={32} />);
    expect(screen.getByTestId("chart-sparkline-empty")).toBeTruthy();
  });

  it("filters non-finite points", () => {
    const data: SparklineDatum[] = [
      { x: 1, y: 10 },
      { x: 2, y: Number.NaN },
      { x: 3, y: 12 },
      { x: 4, y: Number.POSITIVE_INFINITY },
      { x: 5, y: 14 },
    ];
    render(<Sparkline data={data} ariaLabel={ariaLabel} width={W} height={32} />);
    // Three finite points remain → renders the active chart, not empty.
    expect(screen.getByTestId("chart-sparkline")).toBeTruthy();
  });

  it("invokes onTap with the most recent point when clicked", () => {
    const tap = vi.fn();
    const data: SparklineDatum[] = [
      { x: 1, y: 10 },
      { x: 2, y: 12 },
      { x: 3, y: 14 },
    ];
    render(
      <Sparkline data={data} ariaLabel={ariaLabel} width={W} height={32} onTap={tap} />,
    );
    const el = screen.getByTestId("chart-sparkline");
    el.click();
    expect(tap).toHaveBeenCalledTimes(1);
    expect(tap).toHaveBeenCalledWith({ x: 3, y: 14 });
  });
});

describe("Heatmap", () => {
  it("renders an empty state for [] data", () => {
    render(<Heatmap data={[]} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-heatmap-empty")).toBeTruthy();
  });

  it("filters tiles with size <= 0 or non-finite intensity", () => {
    const data: HeatmapDatum[] = [
      { label: "Big winner", size: 100, intensity: 0.8 },
      { label: "Zero size", size: 0, intensity: 0.5 },
      { label: "NaN intensity", size: 50, intensity: Number.NaN },
      { label: "Small loser", size: 30, intensity: -0.4 },
    ];
    render(<Heatmap data={data} ariaLabel={ariaLabel} width={W} height={H} />);
    expect(screen.getByTestId("chart-heatmap")).toBeTruthy();
    expect(screen.queryByTestId("chart-heatmap-empty")).toBeNull();
  });

  it("computes a fallback height from width + aspect ratio", () => {
    render(
      <Heatmap data={[]} ariaLabel={ariaLabel} width={400} aspectRatio={2} />,
    );
    const el = screen.getByTestId("chart-heatmap-empty");
    // 400 / 2 = 200 → element gets height: 200px.
    expect((el as HTMLElement).style.height).toBe("200px");
  });
});
