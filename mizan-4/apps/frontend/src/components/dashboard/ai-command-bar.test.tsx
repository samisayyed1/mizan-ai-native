/**
 * Tests for the inline-streaming dashboard AI Command Bar.
 *
 * Per Sami feedback (2026-06-14): the bar must answer in place on the
 * dashboard rather than navigate. These tests pin the new contract:
 *  - submit streams an inline response (no navigation)
 *  - "Open in full chat" navigates with the thread_id deep-link
 *  - voice button still navigates (PR-C18 hook for real transcription)
 *  - suggestion chips submit inline
 *  - errors render inline without crashing
 *  - sending a second question aborts the first via AbortController
 *  - input + submit disable while streaming so accidental double-fire
 *    can't race
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Hoisted mock state — vi.mock factories run before any imports, so a
// plain `let` declared at module top wouldn't be initialised yet. The
// vi.hoisted block runs in the right order.
const mockState = vi.hoisted(() => ({
  /** Queue of events the next streamChatResponse call will yield. */
  nextEvents: [] as unknown[],
  /** Optional async iterator override (when a test needs to control timing). */
  nextOverride: null as null | (() => AsyncGenerator<unknown>),
  /** Records the AbortSignal the last call received. */
  lastSignal: undefined as undefined | AbortSignal,
  /** Records every (request, signal) tuple. */
  calls: [] as { request: unknown; signal?: AbortSignal }[],
}));

vi.mock("@/features/ai-assistant/api", () => ({
  streamChatResponse: async function* (request: unknown, signal?: AbortSignal) {
    mockState.lastSignal = signal;
    mockState.calls.push({ request, signal });
    if (mockState.nextOverride) {
      yield* mockState.nextOverride();
      return;
    }
    for (const ev of mockState.nextEvents) {
      yield ev;
    }
  },
}));

beforeEach(() => {
  mockState.nextEvents = [];
  mockState.nextOverride = null;
  mockState.lastSignal = undefined;
  mockState.calls = [];
});

import { AiCommandBar } from "./ai-command-bar";

function withRouter(node: React.ReactNode, initialEntries = ["/"]) {
  function LocationProbe() {
    const loc = useLocation();
    return (
      <div
        data-testid="location-probe"
        data-pathname={loc.pathname}
        data-search={loc.search}
      />
    );
  }
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <Routes>
        <Route
          path="/"
          element={
            <>
              {node}
              <LocationProbe />
            </>
          }
        />
        <Route path="/assistant" element={<LocationProbe />} />
        <Route path="/somewhere" element={<LocationProbe />} />
      </Routes>
    </MemoryRouter>,
  );
}

function readLocation() {
  const probe = screen.getByTestId("location-probe");
  return {
    pathname: probe.getAttribute("data-pathname"),
    search: probe.getAttribute("data-search"),
  };
}

describe("<AiCommandBar /> (inline streaming)", () => {
  it("submitting a typed question streams an inline response without navigating", async () => {
    mockState.nextEvents = [
      { type: "system", threadId: "thread-abc", runId: "r1", messageId: "m1" },
      { type: "textDelta", threadId: "thread-abc", runId: "r1", messageId: "m1", delta: "Net" },
      { type: "textDelta", threadId: "thread-abc", runId: "r1", messageId: "m1", delta: " worth" },
      { type: "textDelta", threadId: "thread-abc", runId: "r1", messageId: "m1", delta: " is $1.71M." },
      { type: "done", threadId: "thread-abc", runId: "r1", messageId: "m1" },
    ];
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "What's my net worth?" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));

    await waitFor(() =>
      expect(screen.getByTestId("ai-command-bar-response").textContent).toContain(
        "Net worth is $1.71M.",
      ),
    );
    // Critical: we did NOT navigate to /assistant. Inline-only.
    expect(readLocation().pathname).toBe("/");
    expect(mockState.calls).toHaveLength(1);
  });

  it("'Open in full chat' navigates with the thread id deep-link", async () => {
    mockState.nextEvents = [
      { type: "system", threadId: "thread-xyz", runId: "r2", messageId: "m2" },
      { type: "textDelta", threadId: "thread-xyz", runId: "r2", messageId: "m2", delta: "ok" },
      { type: "done", threadId: "thread-xyz", runId: "r2", messageId: "m2" },
    ];
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "ping" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    await waitFor(() => screen.getByTestId("ai-command-bar-response"));

    fireEvent.click(screen.getByTestId("ai-command-bar-open-full"));
    const { pathname, search } = readLocation();
    expect(pathname).toBe("/assistant");
    expect(search).toContain("thread=thread-xyz");
  });

  it("error event renders inline without throwing", async () => {
    mockState.nextEvents = [
      { type: "system", threadId: "thread-err", runId: "r3", messageId: "m3" },
      {
        type: "error",
        threadId: "thread-err",
        runId: "r3",
        messageId: "m3",
        code: "upstream_failure",
        message: "Anthropic API returned 503",
      },
    ];
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "anything" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("ai-command-bar-error").textContent).toBe(
        "Anthropic API returned 503",
      ),
    );
  });

  it("a thrown error in the stream surfaces inline without navigating away", async () => {
    mockState.nextOverride = async function* () {
      yield { type: "system", threadId: "t", runId: "r", messageId: "m" };
      throw new Error("network exploded");
    };
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "anything" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("ai-command-bar-error").textContent).toContain(
        "network exploded",
      ),
    );
    expect(readLocation().pathname).toBe("/");
  });

  it("voice button still navigates (PR-C18 placeholder)", () => {
    withRouter(<AiCommandBar />);
    fireEvent.click(screen.getByTestId("ai-command-bar-voice"));
    const { pathname, search } = readLocation();
    expect(pathname).toBe("/assistant");
    expect(search).toContain("voice=1");
  });

  it("suggestion chip click runs the canned prompt inline", async () => {
    mockState.nextEvents = [
      { type: "system", threadId: "t", runId: "r", messageId: "m" },
      { type: "textDelta", threadId: "t", runId: "r", messageId: "m", delta: "Zakat answer" },
      { type: "done", threadId: "t", runId: "r", messageId: "m" },
    ];
    withRouter(<AiCommandBar />);
    fireEvent.click(screen.getByRole("button", { name: "What's my Zakat?" }));
    await waitFor(() =>
      expect(screen.getByTestId("ai-command-bar-response").textContent).toContain(
        "Zakat answer",
      ),
    );
    // Stayed on /, did not navigate.
    expect(readLocation().pathname).toBe("/");
    expect(mockState.calls[0].request).toMatchObject({
      content: "What's my Zakat this year?",
    });
  });

  it("input + submit are disabled while a stream is in flight", async () => {
    // Use the override to hold the stream open indefinitely so we can
    // assert disabled state mid-flight without races.
    let resolveHold: (() => void) | undefined;
    const hold = new Promise<void>((r) => {
      resolveHold = r;
    });
    mockState.nextOverride = async function* () {
      yield { type: "system", threadId: "t", runId: "r", messageId: "m" };
      yield { type: "textDelta", threadId: "t", runId: "r", messageId: "m", delta: "..." };
      await hold;
    };

    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "slow" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));

    await waitFor(() =>
      expect(
        (screen.getByTestId("ai-command-bar-input") as HTMLInputElement).disabled,
      ).toBe(true),
    );
    expect(
      (screen.getByTestId("ai-command-bar-submit") as HTMLButtonElement).disabled,
    ).toBe(true);

    resolveHold?.();
  });

  it("clicking × aborts an in-flight stream via AbortController", async () => {
    let aborted = false;
    const hold = new Promise<void>(() => {
      // Never resolves on its own — only completes via abort.
    });
    mockState.nextOverride = async function* () {
      const sig = mockState.lastSignal;
      sig?.addEventListener("abort", () => {
        aborted = true;
      });
      yield { type: "system", threadId: "t1", runId: "r1", messageId: "m1" };
      yield { type: "textDelta", threadId: "t1", runId: "r1", messageId: "m1", delta: "..." };
      await hold;
    };
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "long question" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    await waitFor(() => screen.getByTestId("ai-command-bar-response"));

    // Stream still running — the clear button must abort it so the
    // user isn't stuck waiting on a hung call.
    fireEvent.click(screen.getByTestId("ai-command-bar-clear"));
    await waitFor(() => expect(aborted).toBe(true));
    // Answer pane gone, suggestions restored.
    expect(screen.queryByTestId("ai-command-bar-answer")).toBeNull();
  });

  it("clear (×) button removes the answer and re-shows suggestions when input emptied", async () => {
    mockState.nextEvents = [
      { type: "system", threadId: "t", runId: "r", messageId: "m" },
      { type: "textDelta", threadId: "t", runId: "r", messageId: "m", delta: "hi" },
      { type: "done", threadId: "t", runId: "r", messageId: "m" },
    ];
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "x" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    await waitFor(() => screen.getByTestId("ai-command-bar-response"));
    expect(screen.queryByTestId("ai-command-bar-suggestions")).toBeNull();

    fireEvent.click(screen.getByTestId("ai-command-bar-clear"));
    expect(screen.queryByTestId("ai-command-bar-answer")).toBeNull();
    expect(screen.getByTestId("ai-command-bar-suggestions")).toBeInTheDocument();
  });

  it("empty input still keeps submit disabled", () => {
    withRouter(<AiCommandBar />);
    expect(
      (screen.getByTestId("ai-command-bar-submit") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("renders all four §23-themed suggestion chips when empty", () => {
    withRouter(<AiCommandBar />);
    expect(screen.getByRole("button", { name: "What's my Zakat?" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show net worth this week" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bonds by maturity" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cash flow this quarter" })).toBeInTheDocument();
  });
});
