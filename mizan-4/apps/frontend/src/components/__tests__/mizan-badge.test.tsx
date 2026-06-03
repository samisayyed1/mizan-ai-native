/**
 * Tests for Track E PR-E2 Mizan Badge.
 *
 * Tests live in the frontend app because @mizan/ui doesn't yet ship
 * its own vitest config; the imports below exercise the published
 * surface of the package (the same way every other consumer reads it).
 *
 * Coverage:
 *  - sortModifiersBySeverity: ADR 0023 ordering, stability, unknowns
 *  - <MizanBadge />: origin chip rendering for all 13 origins;
 *    modifier stack renders in severity order; rendering cap at 3;
 *    overflow `+N more` chip fires the callback with the hidden list;
 *    accessibility (role=group + composed aria-label).
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  MAX_VISIBLE_MODIFIERS,
  MizanBadge,
  type MizanBadgeModifier,
  type MizanBadgeOrigin,
  sortModifiersBySeverity,
} from "@mizan/ui";

const ALL_ORIGINS: MizanBadgeOrigin[] = [
  "manual",
  "synced",
  "imported",
  "setu",
  "sgfindex",
  "tink",
  "basiq",
  "lean",
  "ccxt",
  "chain_reader",
  "twelve_data",
  "metalprice_api",
  "bondevalue",
];

const ALL_MODIFIERS: MizanBadgeModifier[] = [
  "stale",
  "pending-reconciliation",
  "ai-estimated",
  "mixed-compliance",
  "halal-screened",
  "agent-modified",
  "audit-trail",
  "advisor-reviewed",
  "mcp",
];

describe("sortModifiersBySeverity", () => {
  it("returns ADR 0023 fixed severity order when given them out of order", () => {
    const shuffled: MizanBadgeModifier[] = [
      "mcp",
      "ai-estimated",
      "stale",
      "audit-trail",
      "halal-screened",
      "pending-reconciliation",
    ];
    expect(sortModifiersBySeverity(shuffled)).toEqual([
      "stale",
      "pending-reconciliation",
      "ai-estimated",
      "halal-screened",
      "audit-trail",
      "mcp",
    ]);
  });

  it("preserves duplicate ordering (caller bug, but tolerated)", () => {
    expect(sortModifiersBySeverity(["stale", "stale"])).toEqual(["stale", "stale"]);
  });

  it("ranks all 9 ADR-0023 modifiers in the documented severity order", () => {
    expect(sortModifiersBySeverity(ALL_MODIFIERS)).toEqual(ALL_MODIFIERS);
  });

  it("returns empty for empty input", () => {
    expect(sortModifiersBySeverity([])).toEqual([]);
  });

  it("places unknown-future modifiers at the end (forward-compat)", () => {
    // Cast through unknown — this models a future variant added before
    // the consumer updates its type defs.
    const future = "totally-new" as unknown as MizanBadgeModifier;
    expect(sortModifiersBySeverity([future, "halal-screened"])).toEqual([
      "halal-screened",
      future,
    ]);
  });
});

describe("<MizanBadge />", () => {
  it("renders the origin chip with the correct display label", () => {
    render(<MizanBadge origin="bondevalue" />);
    expect(screen.getByTestId("mizan-badge-origin").textContent).toBe("Bondevalue");
  });

  it("renders all 13 origin variants without throwing", () => {
    for (const origin of ALL_ORIGINS) {
      const { unmount } = render(<MizanBadge origin={origin} />);
      expect(screen.getByTestId("mizan-badge").getAttribute("data-origin")).toBe(origin);
      unmount();
    }
  });

  it("renders modifiers in ADR 0023 severity order regardless of input order", () => {
    render(
      <MizanBadge
        origin="synced"
        modifiers={["audit-trail", "stale", "halal-screened"]}
      />,
    );
    const root = screen.getByTestId("mizan-badge");
    const modifierEls = Array.from(
      root.querySelectorAll('[data-testid^="mizan-badge-modifier-"]'),
    );
    const orderedTestIds = modifierEls.map((el) => el.getAttribute("data-testid"));
    expect(orderedTestIds).toEqual([
      "mizan-badge-modifier-stale",
      "mizan-badge-modifier-halal-screened",
      "mizan-badge-modifier-audit-trail",
    ]);
  });

  it("caps visible modifiers at MAX_VISIBLE_MODIFIERS and shows overflow chip", () => {
    expect(MAX_VISIBLE_MODIFIERS).toBe(3);
    render(<MizanBadge origin="synced" modifiers={ALL_MODIFIERS} />);
    // First 3 in severity order are visible
    expect(screen.getByTestId("mizan-badge-modifier-stale")).toBeInTheDocument();
    expect(screen.getByTestId("mizan-badge-modifier-pending-reconciliation")).toBeInTheDocument();
    expect(screen.getByTestId("mizan-badge-modifier-ai-estimated")).toBeInTheDocument();
    // Remaining 6 are hidden
    expect(screen.queryByTestId("mizan-badge-modifier-mixed-compliance")).toBeNull();
    expect(screen.queryByTestId("mizan-badge-modifier-mcp")).toBeNull();
    // Overflow chip says +6 more
    expect(screen.getByTestId("mizan-badge-overflow-button").textContent).toBe("+6 more");
  });

  it("respects a custom visible cap", () => {
    render(
      <MizanBadge
        origin="synced"
        modifiers={["stale", "halal-screened", "audit-trail"]}
        maxVisibleModifiers={1}
      />,
    );
    expect(screen.getByTestId("mizan-badge-modifier-stale")).toBeInTheDocument();
    expect(screen.queryByTestId("mizan-badge-modifier-halal-screened")).toBeNull();
    expect(screen.getByTestId("mizan-badge-overflow-button").textContent).toBe("+2 more");
  });

  it("does not render the overflow chip when nothing overflows", () => {
    render(<MizanBadge origin="synced" modifiers={["halal-screened"]} />);
    expect(screen.queryByTestId("mizan-badge-overflow")).toBeNull();
  });

  it("fires onOverflowActivate with the hidden modifier list when overflow is tapped", () => {
    const onOverflow = vi.fn();
    render(
      <MizanBadge
        origin="synced"
        modifiers={["stale", "halal-screened", "audit-trail", "mcp"]}
        maxVisibleModifiers={2}
        onOverflowActivate={onOverflow}
      />,
    );
    fireEvent.click(screen.getByTestId("mizan-badge-overflow-button"));
    expect(onOverflow).toHaveBeenCalledTimes(1);
    expect(onOverflow).toHaveBeenCalledWith(["audit-trail", "mcp"]);
  });

  it("uses role=group and a composed aria-label including modifier count", () => {
    render(
      <MizanBadge
        origin="setu"
        modifiers={["halal-screened", "audit-trail"]}
      />,
    );
    const root = screen.getByTestId("mizan-badge");
    expect(root.getAttribute("role")).toBe("group");
    expect(root.getAttribute("aria-label")).toMatch(/Setu/);
    expect(root.getAttribute("aria-label")).toMatch(/2 modifiers/);
  });

  it("omits modifier count from aria-label when there are none", () => {
    render(<MizanBadge origin="manual" />);
    const root = screen.getByTestId("mizan-badge");
    expect(root.getAttribute("aria-label")).toBe("Origin Manual");
  });

  it("accepts an aria-label override", () => {
    render(
      <MizanBadge
        origin="synced"
        modifiers={["halal-screened"]}
        ariaLabel="Source verified by AAOIFI screener"
      />,
    );
    expect(screen.getByTestId("mizan-badge").getAttribute("aria-label")).toBe(
      "Source verified by AAOIFI screener",
    );
  });
});
