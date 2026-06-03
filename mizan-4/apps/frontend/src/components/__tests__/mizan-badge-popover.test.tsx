/**
 * Tests for Track E PR-E3 — MizanBadge per-modifier popover renderers.
 *
 * Two layers:
 *   1. renderBadgePopoverContent (pure helper) — table-driven coverage
 *      for all 9 modifier contexts; pinned nowMs for deterministic
 *      relative-time strings.
 *   2. <MizanBadgePopover /> — render + click action wiring + content
 *      slot composition (each renderer exposes a testid the host can
 *      assert on).
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  MizanBadgePopover,
  renderBadgePopoverContent,
  type MizanBadgePopoverContext,
} from "@mizan/ui";

const NOW = Date.UTC(2026, 5, 4, 14, 0, 0); // 2026-06-04 14:00 UTC
const ONE_HOUR_MS = 60 * 60 * 1000;
const ONE_DAY_MS = 24 * ONE_HOUR_MS;

describe("renderBadgePopoverContent", () => {
  it("renders the stale modifier with relative time + policy window", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "stale",
        lastSyncedAtMs: NOW - 14 * ONE_HOUR_MS,
        policyWindowMs: 6 * ONE_HOUR_MS,
      },
      NOW,
    );
    expect(out.why).toMatch(/hasn't been refreshed/);
    expect(out.why).toMatch(/14h ago/);
    expect(out.data).toMatch(/6h/);
    expect(out.action).toBeUndefined();
  });

  it("exposes an action on stale when onRefresh is provided", () => {
    const onRefresh = vi.fn();
    const out = renderBadgePopoverContent(
      {
        modifier: "stale",
        lastSyncedAtMs: NOW - 14 * ONE_HOUR_MS,
        policyWindowMs: 6 * ONE_HOUR_MS,
        onRefresh,
      },
      NOW,
    );
    expect(out.action?.label).toBe("Refresh now");
    out.action?.onClick();
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("renders pending-reconciliation with prior/current diff", () => {
    const onReview = vi.fn();
    const out = renderBadgePopoverContent(
      {
        modifier: "pending-reconciliation",
        priorValue: 100_000,
        currentValue: 105_500,
        currency: "USD",
        onReview,
      },
      NOW,
    );
    expect(out.why).toMatch(/differs/);
    expect(out.data).toMatch(/\$100,000/);
    expect(out.data).toMatch(/\$105,500/);
    expect(out.action?.label).toBe("Review diff");
    out.action?.onClick();
    expect(onReview).toHaveBeenCalledTimes(1);
  });

  it("renders ai-estimated with range + confidence + source", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "ai-estimated",
        confidence: 0.78,
        low: 220_000,
        high: 260_000,
        currency: "USD",
        source: "Dubai Land Department",
      },
      NOW,
    );
    expect(out.why).toMatch(/AI estimation/);
    expect(out.data).toMatch(/\$220,000.*\$260,000/);
    expect(out.data).toMatch(/78%/);
    expect(out.data).toMatch(/Dubai Land Department/);
  });

  it("clamps ai-estimated confidence outside [0,1] for the displayed pct", () => {
    const high = renderBadgePopoverContent(
      {
        modifier: "ai-estimated",
        confidence: 1.5,
        low: 1,
        high: 2,
        currency: "USD",
        source: "X",
      },
      NOW,
    );
    expect(high.data).toMatch(/100%/);
    const low = renderBadgePopoverContent(
      {
        modifier: "ai-estimated",
        confidence: -0.2,
        low: 1,
        high: 2,
        currency: "USD",
        source: "X",
      },
      NOW,
    );
    expect(low.data).toMatch(/0%/);
  });

  it("renders halal-screened with screener + relative time", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "halal-screened",
        screenedAtMs: NOW - 2 * ONE_DAY_MS,
        screener: "AAOIFI v2",
      },
      NOW,
    );
    expect(out.why).toMatch(/Sharia compliance/);
    expect(out.data).toMatch(/AAOIFI v2/);
    expect(out.data).toMatch(/2d ago/);
  });

  it("renders mixed-compliance with joined reasons", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "mixed-compliance",
        screenedAtMs: NOW - 3 * ONE_HOUR_MS,
        reasons: ["debt ratio 35%", "interest income 6%"],
      },
      NOW,
    );
    expect(out.why).toMatch(/mixed verdict/);
    expect(out.data).toMatch(/debt ratio 35%; interest income 6%/);
  });

  it("falls back to a default reason for mixed-compliance when reasons is empty", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "mixed-compliance",
        screenedAtMs: NOW - 3 * ONE_HOUR_MS,
        reasons: [],
      },
      NOW,
    );
    expect(out.data).toMatch(/ambiguous criteria/);
  });

  it("renders audit-trail with truncated hash + action", () => {
    const onView = vi.fn();
    const out = renderBadgePopoverContent(
      {
        modifier: "audit-trail",
        truthLedgerHash: "deadbeefcafef00d1234567890abcdef",
        onViewLedger: onView,
      },
      NOW,
    );
    expect(out.why).toMatch(/Truth Ledger/);
    expect(out.data).toMatch(/deadbeefca/);
    expect(out.action?.label).toBe("Open Truth Ledger");
    out.action?.onClick();
    expect(onView).toHaveBeenCalled();
  });

  it("renders advisor-reviewed with advisor name", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "advisor-reviewed",
        advisorName: "Aisha Khan",
        reviewedAtMs: NOW - 5 * ONE_DAY_MS,
      },
      NOW,
    );
    expect(out.why).toMatch(/advisor/);
    expect(out.data).toMatch(/Aisha Khan/);
    expect(out.data).toMatch(/5d ago/);
  });

  it("renders agent-modified with tool name", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "agent-modified",
        toolName: "update_holding",
        modifiedAtMs: NOW - 15 * 60 * 1000,
      },
      NOW,
    );
    expect(out.why).toMatch(/Mizan AI/);
    expect(out.data).toMatch(/update_holding/);
    expect(out.data).toMatch(/15 min ago/);
  });

  it("renders mcp with server name + optional open action", () => {
    const out = renderBadgePopoverContent(
      {
        modifier: "mcp",
        serverName: "Notion (work)",
        serverUrl: "https://notion.so",
      },
      NOW,
    );
    expect(out.why).toMatch(/MCP server/);
    expect(out.data).toMatch(/Notion \(work\)/);
    expect(out.action?.label).toBe("Open server");
  });

  it("handles non-currency-code currency gracefully", () => {
    // Intl.NumberFormat throws on unknown ISO codes — the helper
    // catches and falls back to "CODE value".
    const out = renderBadgePopoverContent(
      {
        modifier: "ai-estimated",
        confidence: 0.5,
        low: 100,
        high: 200,
        currency: "ZZZ",
        source: "X",
      },
      NOW,
    );
    expect(out.data).toMatch(/ZZZ\s?100/);
  });
});

describe("<MizanBadgePopover />", () => {
  function withRouter(node: React.ReactNode) {
    return render(<>{node}</>);
  }

  it("renders the trigger and content with the correct testid", () => {
    withRouter(
      <MizanBadgePopover
        modifier="halal-screened"
        context={{
          modifier: "halal-screened",
          screenedAtMs: NOW - ONE_HOUR_MS,
          screener: "AAOIFI v2",
        }}
        nowMs={NOW}
      >
        <button type="button">chip</button>
      </MizanBadgePopover>,
    );
    fireEvent.click(screen.getByText("chip"));
    expect(
      screen.getByTestId("mizan-badge-popover-halal-screened"),
    ).toBeInTheDocument();
  });

  it("wires the action button to call its onClick", () => {
    const onRefresh = vi.fn();
    withRouter(
      <MizanBadgePopover
        modifier="stale"
        context={{
          modifier: "stale",
          lastSyncedAtMs: NOW - 14 * ONE_HOUR_MS,
          policyWindowMs: 6 * ONE_HOUR_MS,
          onRefresh,
        }}
        nowMs={NOW}
      >
        <button type="button">chip</button>
      </MizanBadgePopover>,
    );
    fireEvent.click(screen.getByText("chip"));
    fireEvent.click(screen.getByTestId("mizan-badge-popover-action-stale"));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("logs a console.error when modifier prop and context.modifier disagree (dev only)", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {
      /* swallow */
    });
    const ctx: MizanBadgePopoverContext = {
      modifier: "stale",
      lastSyncedAtMs: NOW - ONE_HOUR_MS,
      policyWindowMs: ONE_HOUR_MS,
    };
    withRouter(
      <MizanBadgePopover modifier="halal-screened" context={ctx} nowMs={NOW}>
        <button type="button">chip</button>
      </MizanBadgePopover>,
    );
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});
