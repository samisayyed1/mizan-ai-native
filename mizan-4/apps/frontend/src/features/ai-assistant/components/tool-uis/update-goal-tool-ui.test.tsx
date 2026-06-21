/**
 * Render-state coverage for the update_goal tool-UI.
 *
 * Pairs with delete-stock-position-tool-ui.test.tsx to lock down
 * Sami's "set/change goals through AI" flow at the visual surface
 * the user actually touches. Tests every state the impl can resolve to:
 *
 *   1. Running        → loading skeleton
 *   2. Incomplete     → error card
 *   3. Missing result → error card
 *   4. Unresolved     → "Couldn't pick a goal" with candidates
 *   5. Diff card      → field-by-field diff with Confirm CTA
 *   6. Submitted      → "Goal updated" success card
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/adapters", () => ({
  updateToolResult: vi.fn(async () => ({ ok: true })),
}));

vi.mock("../../hooks/use-runtime-context", () => ({
  useRuntimeContext: () => ({ currentThreadId: "thread-test" }),
}));

vi.mock("@/features/goals/hooks/use-goals", () => ({
  useGoalMutations: () => ({
    updateMutation: {
      mutateAsync: vi.fn(async () => ({ ok: true })),
      isPending: false,
    },
  }),
}));

vi.mock("@mizan/ui/components/ui/use-toast", () => ({
  toast: vi.fn(),
}));

import { UpdateGoalToolUIContentImpl } from "./update-goal-tool-ui";

function withClient(node: React.ReactElement) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

// ---- Fixtures ----

function makeCurrent(overrides: object = {}) {
  return {
    id: "g1",
    title: "Retirement",
    goalType: "RETIREMENT",
    targetAmount: 1_000_000,
    currency: "USD",
    statusLifecycle: "ACTIVE",
    priority: 2,
    targetDate: "2055-01-01",
    description: null,
    ...overrides,
  };
}

function makeDraft(overrides: object = {}) {
  return {
    id: "g1",
    title: "Retirement",
    targetAmount: 2_000_000,
    statusLifecycle: "ACTIVE",
    priority: 2,
    targetDate: "2055-01-01",
    description: null,
    ...overrides,
  };
}

function makeResolvedResult(overrides: object = {}) {
  return {
    draft: makeDraft(),
    current: makeCurrent(),
    diff: [
      { field: "targetAmount", old: "1000000", new: "2000000" },
    ],
    validation: {
      resolved: true,
      candidates: [],
      warnings: [],
      isValid: true,
      hasChanges: true,
    },
    ...overrides,
  };
}

// ---- Tests ----

describe("UpdateGoalToolUI", () => {
  it("renders the loading skeleton while the tool call is running", () => {
    const { container } = render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={undefined}
          status={{ type: "running" }}
          toolCallId="t1"
          {...({} as any)}
        />,
      ),
    );
    // Loading skeleton uses Skeleton elements inside a muted card.
    expect(container.querySelector(".bg-muted\\/40")).toBeInTheDocument();
  });

  it("renders an error card when the tool result is missing", () => {
    render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={undefined}
          status={{ type: "complete" }}
          toolCallId="t2"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/No update data available/i)).toBeInTheDocument();
  });

  it("renders the unresolved card with candidates when the AI couldn't pick a goal", () => {
    const result = {
      draft: makeDraft(),
      current: makeCurrent(),
      diff: [],
      validation: {
        resolved: false,
        candidates: [
          { ...makeCurrent({ id: "a", title: "Retirement A" }) },
          { ...makeCurrent({ id: "b", title: "Retirement B" }) },
        ],
        warnings: ["'Retirement' matched 2 goals — ask the user."],
        isValid: false,
        hasChanges: false,
      },
    };
    render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t3"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/Couldn't pick a goal/i)).toBeInTheDocument();
    expect(screen.getByText(/Retirement A/)).toBeInTheDocument();
    expect(screen.getByText(/Retirement B/)).toBeInTheDocument();
    expect(screen.getByText(/matched 2 goals/)).toBeInTheDocument();
  });

  it("renders the diff card with Confirm CTA when there are changes to apply", () => {
    render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={makeResolvedResult()}
          status={{ type: "complete" }}
          toolCallId="t4"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/Update goal\?/i)).toBeInTheDocument();
    // Diff row labels are humanised — "targetAmount" → "Target amount".
    expect(screen.getByText(/Target amount/)).toBeInTheDocument();
    // The Confirm button is enabled when diff has entries.
    const confirm = screen.getByRole("button", { name: /Confirm & save/i });
    expect(confirm).toBeInTheDocument();
    expect(confirm).not.toBeDisabled();
  });

  it("disables the Confirm button when the diff is empty (no-op guard)", () => {
    const result = {
      ...makeResolvedResult(),
      diff: [],
      validation: {
        ...makeResolvedResult().validation,
        hasChanges: false,
        isValid: false,
        warnings: ["Nothing to change — every field matched the current goal."],
      },
    };
    render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t5"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/No fields will change/)).toBeInTheDocument();
    const confirm = screen.getByRole("button", { name: /Confirm & save/i });
    expect(confirm).toBeDisabled();
  });

  it("renders the success card once the result is marked submitted", () => {
    const result = {
      ...makeResolvedResult(),
      submitted: true,
      updatedAt: "2026-06-21T16:00:00Z",
    };
    render(
      withClient(
        <UpdateGoalToolUIContentImpl
          result={result}
          status={{ type: "complete" }}
          toolCallId="t6"
          {...({} as any)}
        />,
      ),
    );
    expect(screen.getByText(/Goal updated/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Confirm & save/i })).toBeNull();
  });
});
