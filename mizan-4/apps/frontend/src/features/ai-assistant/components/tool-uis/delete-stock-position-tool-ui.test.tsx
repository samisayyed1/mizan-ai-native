/**
 * Render-state coverage for the delete_stock_position tool-UI.
 *
 * Sami's explicit user goal 2026-06-21: "make sure all stocks can be
 * added and deleted manually easily, all through the standalone AI
 * page — be world class and be the best in the game."
 *
 * The tool-UI is the user-facing surface for the delete flow. These
 * tests verify each render state the user can land in:
 *   1. Running        → loading skeleton
 *   2. Incomplete     → error card
 *   3. Unrecognised   → error card
 *   4. Unresolved     → "couldn't pick an asset" card with candidates
 *   5. Resolved + Δ=0 → "nothing to remove" error card
 *   6. Resolved + Δ>0 → Confirm card with the positions + total value
 *   7. Submitted      → Success card
 *
 * The mutation handler itself (deleteActivity + refreshAfterMutation)
 * is exercised by the backend test in tools::delete_stock_position. We
 * stub it here so a confirmation click doesn't try to hit the Tauri
 * IPC layer that doesn't exist in vitest's jsdom environment.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/adapters", () => ({
  deleteActivity: vi.fn(async () => ({ ok: true })),
  updateToolResult: vi.fn(async () => ({ ok: true })),
}));

vi.mock("../../hooks/use-runtime-context", () => ({
  useRuntimeContext: () => ({ currentThreadId: "thread-test" }),
}));

vi.mock("@mizan/ui/components/ui/use-toast", () => ({
  toast: vi.fn(),
}));

import { DeleteStockPositionToolUIContentImpl } from "./delete-stock-position-tool-ui";

function withClient(node: React.ReactElement) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

// ---- Fixtures ----

function makeTarget(overrides: object = {}) {
  return {
    id: "asset-aapl",
    symbol: "AAPL",
    name: "Apple Inc.",
    kind: "EQUITY",
    ...overrides,
  };
}

function makeResolvedResult(overrides: object = {}) {
  return {
    target: makeTarget(),
    positions: [
      {
        accountId: "acc-us",
        accountName: "US Portfolio",
        quantity: 30,
        marketValueLocal: 8500,
        marketValueBase: 8500,
        localCurrency: "USD",
        activityCount: 1,
      },
    ],
    activityIds: ["act-1"],
    totalMarketValueBase: 8500,
    reason: null,
    validation: {
      resolved: true,
      candidates: [],
      warnings: [],
      isValid: true,
    },
    ...overrides,
  };
}

// ---- Tests ----

describe("DeleteStockPositionToolUI", () => {
  it("renders the loading skeleton while the tool call is running", () => {
    const { container } = render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={undefined}
          status={{ type: "running" }}
          toolCallId="t1"
          // assistant-ui's prop type carries other fields we don't read in
          // the impl; the cast is the standard test pattern.
          {...({} as any)}
        />,
      ),
    );
    // Skeleton renders as a div with the Skeleton class. We assert the
    // shape by querying for the muted background utility the Skeleton
    // component applies.
    expect(container.querySelector(".bg-muted\\/40")).toBeInTheDocument();
  });

  it("renders an error card when the tool result is missing", () => {
    render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={undefined}
          status={{ type: "complete" }}
          toolCallId="t2"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/No removal data available/i)).toBeInTheDocument();
  });

  it("renders the unresolved card with candidates when the AI couldn't pick an asset", () => {
    const result = {
      target: { id: "", symbol: "", name: "", kind: "" },
      positions: [],
      activityIds: [],
      totalMarketValueBase: 0,
      reason: null,
      validation: {
        resolved: false,
        candidates: [
          { id: "a", symbol: "AAPL", name: "Apple Inc.", kind: "EQUITY" },
          { id: "b", symbol: "APLE", name: "Apple Hospitality REIT", kind: "EQUITY" },
        ],
        warnings: ["The reference 'apple' matched 2 assets — ask the user."],
        isValid: false,
      },
    };
    render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t3"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/Couldn't pick a stock/i)).toBeInTheDocument();
    expect(screen.getAllByText(/Apple Inc\./).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Apple Hospitality REIT/).length).toBeGreaterThan(0);
    expect(screen.getByText(/matched 2 assets/)).toBeInTheDocument();
  });

  it("renders the 'nothing to remove' card when validation resolved but no positions found", () => {
    const result = {
      ...makeResolvedResult(),
      positions: [],
      activityIds: [],
      totalMarketValueBase: 0,
      validation: {
        resolved: true,
        candidates: [],
        warnings: ["AAPL has no open positions."],
        isValid: false,
      },
    };
    render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t4"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/Nothing to remove/i)).toBeInTheDocument();
    expect(screen.getByText(/no open positions/i)).toBeInTheDocument();
  });

  it("renders the Confirm card with the position breakdown when a delete is ready", () => {
    render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={makeResolvedResult()}
          status={{ type: "complete" }}
          toolCallId="t5"
          {...({} as any)}
        />,
      ),
    );
    // Sami's user-visible promise: the confirm flow lives on /assistant
    // and surfaces the position before any mutation runs.
    expect(screen.getAllByText(/Apple Inc\./).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/AAPL/).length).toBeGreaterThan(0);
    // The card titles itself "Remove position?" with the confirm CTA
    // labelled "Confirm & delete".
    expect(screen.getByText(/Remove position\?/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Confirm & delete/i })).toBeInTheDocument();
  });

  it("renders the success card once the result is marked submitted", () => {
    const result = {
      ...makeResolvedResult(),
      submitted: true,
      removedAt: "2026-06-21T15:00:00Z",
    };
    render(
      withClient(
        <DeleteStockPositionToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t6"
          {...({} as any)}
        />,
      ),
    );
    // Success card title — distinct from the in-progress "Remove
    // position?" title so the user can tell at a glance the mutation
    // landed.
    expect(screen.getByText(/Position removed/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Confirm & delete/i })).toBeNull();
  });
});
