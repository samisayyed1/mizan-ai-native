/**
 * Tests for WhyThisMatters — Track D PR-D4.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { WhyThisMatters } from "./why-this-matters";

describe("WhyThisMatters", () => {
  it("renders nothing when rationale is undefined", () => {
    const { container } = render(<WhyThisMatters />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when rationale is an empty array", () => {
    const { container } = render(<WhyThisMatters rationale={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the heading + rationale list", () => {
    render(<WhyThisMatters rationale={["You hold DAR"]} />);
    expect(screen.getByText(/why this matters to you/i)).toBeInTheDocument();
    expect(screen.getByText("You hold DAR")).toBeInTheDocument();
  });

  it("shows only the first 2 entries collapsed by default", () => {
    render(
      <WhyThisMatters
        rationale={[
          "You hold DAR",
          "Touches your Sukuks positions",
          'Matches your memory note "ramadan"',
        ]}
      />,
    );
    expect(screen.getByText("You hold DAR")).toBeInTheDocument();
    expect(screen.getByText("Touches your Sukuks positions")).toBeInTheDocument();
    expect(
      screen.queryByText('Matches your memory note "ramadan"'),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/\+1 more/i)).toBeInTheDocument();
  });

  it("expands to show all entries when '+N more' clicked", () => {
    render(
      <WhyThisMatters
        rationale={[
          "You hold DAR",
          "Touches your Sukuks positions",
          'Matches your memory note "ramadan"',
        ]}
      />,
    );
    fireEvent.click(screen.getByText(/\+1 more/i));
    expect(screen.getByText('Matches your memory note "ramadan"')).toBeInTheDocument();
  });

  it("respects custom collapsedLimit", () => {
    render(
      <WhyThisMatters
        rationale={["one", "two", "three", "four"]}
        collapsedLimit={1}
      />,
    );
    expect(screen.getByText("one")).toBeInTheDocument();
    expect(screen.queryByText("two")).not.toBeInTheDocument();
    expect(screen.getByText(/\+3 more/i)).toBeInTheDocument();
  });

  it("no '+N more' button when rationale fits within collapsed limit", () => {
    render(<WhyThisMatters rationale={["only one"]} />);
    expect(screen.queryByText(/more/i)).not.toBeInTheDocument();
  });

  it("§23 fixture: surfaces sukuk-flavoured rationale", () => {
    render(
      <WhyThisMatters
        rationale={[
          "You hold DAR",
          "Touches your Sukuks positions",
          'Semantically matches your memory note about "Saudi issuers"',
        ]}
      />,
    );
    // First two render collapsed
    expect(screen.getByText("You hold DAR")).toBeInTheDocument();
    expect(screen.getByText("Touches your Sukuks positions")).toBeInTheDocument();
    fireEvent.click(screen.getByText(/\+1 more/i));
    expect(
      screen.getByText('Semantically matches your memory note about "Saudi issuers"'),
    ).toBeInTheDocument();
  });

  it("has correct aria-label for accessibility", () => {
    render(<WhyThisMatters rationale={["one"]} />);
    expect(screen.getByLabelText(/why this matters to you/i)).toBeInTheDocument();
  });
});
