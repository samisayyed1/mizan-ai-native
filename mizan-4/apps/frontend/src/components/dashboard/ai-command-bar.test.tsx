/**
 * Tests for Track A PR-A6 AI Command Bar.
 *
 * Covers:
 *  - text input + submit navigates to /assistant with encoded prompt
 *  - empty input cannot be submitted (button disabled, enter no-op)
 *  - suggestion chips submit their canned prompts
 *  - voice button navigates to /assistant?voice=1
 *  - custom toRoute override threads through
 *  - initialValue seeds the input
 *  - suggestions chips render only when input is empty
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { AiCommandBar } from "./ai-command-bar";

/** Test harness: renders the bar + a sniffer route that echoes location. */
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

describe("<AiCommandBar />", () => {
  it("submitting a typed question navigates to /assistant with encoded prompt", () => {
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "What's my Zakat this year?" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    const { pathname, search } = readLocation();
    expect(pathname).toBe("/assistant");
    expect(search).toContain("intent=command");
    expect(search).toContain("prompt=What%27s+my+Zakat+this+year%3F");
  });

  it("empty input keeps submit disabled and pressing enter is a no-op", () => {
    withRouter(<AiCommandBar />);
    const submit = screen.getByTestId("ai-command-bar-submit");
    expect((submit as HTMLButtonElement).disabled).toBe(true);

    // Submit via Enter — form submit should be a no-op because handleSubmit
    // bails on empty trimmed value. Location stays at "/".
    const form = submit.closest("form");
    if (!form) throw new Error("form not found");
    fireEvent.submit(form);
    expect(readLocation().pathname).toBe("/");
  });

  it("trimmed empty input (whitespace only) also no-ops", () => {
    withRouter(<AiCommandBar />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "    " },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    expect(readLocation().pathname).toBe("/");
  });

  it("clicking a suggestion chip submits its canned prompt", () => {
    withRouter(<AiCommandBar />);
    fireEvent.click(screen.getByRole("button", { name: "What's my Zakat?" }));
    const { pathname, search } = readLocation();
    expect(pathname).toBe("/assistant");
    expect(search).toContain("prompt=What%27s+my+Zakat+this+year%3F");
  });

  it("voice button navigates to /assistant?voice=1", () => {
    withRouter(<AiCommandBar />);
    fireEvent.click(screen.getByTestId("ai-command-bar-voice"));
    const { pathname, search } = readLocation();
    expect(pathname).toBe("/assistant");
    expect(search).toContain("voice=1");
  });

  it("custom toRoute override threads through to navigation", () => {
    withRouter(<AiCommandBar toRoute="/somewhere" />);
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "hello" },
    });
    fireEvent.click(screen.getByTestId("ai-command-bar-submit"));
    expect(readLocation().pathname).toBe("/somewhere");
  });

  it("initialValue seeds the input and hides suggestions", () => {
    withRouter(<AiCommandBar initialValue="pre-seeded" />);
    const input = screen.getByTestId("ai-command-bar-input");
    expect((input as HTMLInputElement).value).toBe("pre-seeded");
    expect(screen.queryByTestId("ai-command-bar-suggestions")).toBeNull();
  });

  it("suggestions surface only when input is empty", () => {
    withRouter(<AiCommandBar />);
    expect(screen.getByTestId("ai-command-bar-suggestions")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("ai-command-bar-input"), {
      target: { value: "x" },
    });
    expect(screen.queryByTestId("ai-command-bar-suggestions")).toBeNull();
  });

  it("renders all four §23-themed suggestion chips by default", () => {
    withRouter(<AiCommandBar />);
    expect(screen.getByRole("button", { name: "What's my Zakat?" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show net worth this week" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bonds by maturity" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cash flow this quarter" })).toBeInTheDocument();
  });
});
