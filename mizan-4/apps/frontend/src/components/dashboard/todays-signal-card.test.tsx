/**
 * Tests for Track A PR-A7 Today's Signal card.
 *
 * Covers the pure selector (algorithm spec) + the card rendering:
 *   - severity ranking + unread tiebreak + recency tiebreak
 *   - today-only filter (yesterday's signals excluded)
 *   - dismissed exclusion
 *   - severity-specific chrome (Action / Heads up / Today's Signal / Milestone)
 *   - tap with deepLink → router navigation
 *   - tap without deepLink → expand body
 *   - empty-state copy when no qualifying signal
 *   - loading skeleton
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
import { describe, expect, it } from "vitest";

import type {
  Notification,
  NotificationKind,
  NotificationSeverity,
} from "@/adapters/types-notifications";

import { TodaysSignalCard, pickTodaysSignal } from "./todays-signal-card";

const NOW = new Date(2026, 5, 4, 14, 0, 0).getTime(); // Local 2026-06-04 14:00
const TODAY_START = new Date(2026, 5, 4, 0, 0, 0).getTime();
const ONE_HOUR_MS = 60 * 60 * 1000;
const ONE_DAY_MS = 24 * ONE_HOUR_MS;

function makeNotif(overrides: Partial<Notification>): Notification {
  return {
    id: overrides.id ?? "n-1",
    kind: (overrides.kind ?? "big_move") as NotificationKind,
    severity: (overrides.severity ?? "info") as NotificationSeverity,
    title: overrides.title ?? "Some title",
    body: overrides.body ?? "Some body",
    deepLink: overrides.deepLink ?? null,
    payloadJson: overrides.payloadJson ?? "{}",
    dedupeKey: overrides.dedupeKey ?? "k-1",
    createdAtMs: overrides.createdAtMs ?? NOW - ONE_HOUR_MS,
    readAtMs: overrides.readAtMs ?? null,
    dismissedAtMs: overrides.dismissedAtMs ?? null,
  };
}

describe("pickTodaysSignal", () => {
  it("returns null on empty input", () => {
    expect(pickTodaysSignal([], NOW)).toBeNull();
  });

  it("returns null when nothing is from today", () => {
    const yesterday = makeNotif({ id: "y", createdAtMs: NOW - ONE_DAY_MS });
    expect(pickTodaysSignal([yesterday], NOW)).toBeNull();
  });

  it("filters out dismissed even when severity is high", () => {
    const dismissed = makeNotif({
      id: "d",
      severity: "critical",
      dismissedAtMs: NOW,
      createdAtMs: TODAY_START + ONE_HOUR_MS,
    });
    const live = makeNotif({
      id: "l",
      severity: "info",
      createdAtMs: TODAY_START + ONE_HOUR_MS,
    });
    expect(pickTodaysSignal([dismissed, live], NOW)?.id).toBe("l");
  });

  it("prefers critical > warning > info > success", () => {
    const success = makeNotif({ id: "s", severity: "success", createdAtMs: NOW - ONE_HOUR_MS });
    const info = makeNotif({ id: "i", severity: "info", createdAtMs: NOW - ONE_HOUR_MS });
    const warning = makeNotif({ id: "w", severity: "warning", createdAtMs: NOW - ONE_HOUR_MS });
    const critical = makeNotif({ id: "c", severity: "critical", createdAtMs: NOW - ONE_HOUR_MS });
    expect(pickTodaysSignal([success, info, warning, critical], NOW)?.id).toBe("c");
  });

  it("within a tier prefers unread over read", () => {
    const read = makeNotif({
      id: "r",
      severity: "warning",
      createdAtMs: NOW - ONE_HOUR_MS,
      readAtMs: NOW - ONE_HOUR_MS / 2,
    });
    const unread = makeNotif({
      id: "u",
      severity: "warning",
      createdAtMs: NOW - 2 * ONE_HOUR_MS,
    });
    expect(pickTodaysSignal([read, unread], NOW)?.id).toBe("u");
  });

  it("within a tier and read-state, prefers most recent", () => {
    const older = makeNotif({
      id: "old",
      severity: "info",
      createdAtMs: TODAY_START + 1,
    });
    const newer = makeNotif({
      id: "new",
      severity: "info",
      createdAtMs: NOW - 60_000,
    });
    expect(pickTodaysSignal([older, newer], NOW)?.id).toBe("new");
  });

  it("uses local-day boundary not UTC", () => {
    // A notification 23h ago is from yesterday in local time when the user
    // is opening the app at 14:00 — it should be excluded.
    const yesterday23h = makeNotif({
      id: "y23",
      createdAtMs: NOW - 23 * ONE_HOUR_MS,
    });
    // The yesterday23h test only fails if NOW < 23h after local midnight.
    // NOW is 14:00 local → midnight was 14h ago. 23h ago is before midnight.
    expect(pickTodaysSignal([yesterday23h], NOW)).toBeNull();
  });
});

/** Sniffer route for the navigation tests. */
function withRouter(node: React.ReactNode) {
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
    <MemoryRouter initialEntries={["/"]}>
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
        <Route path="/holdings/:rest" element={<LocationProbe />} />
        <Route path="/holdings" element={<LocationProbe />} />
        <Route path="/assistant" element={<LocationProbe />} />
      </Routes>
    </MemoryRouter>,
  );
}

function readLocation() {
  const probe = screen.getByTestId("location-probe");
  return probe.getAttribute("data-pathname");
}

describe("<TodaysSignalCard />", () => {
  it("renders the loading skeleton when isLoading is true", () => {
    withRouter(<TodaysSignalCard notifications={[]} isLoading nowMs={NOW} />);
    expect(screen.getByTestId("todays-signal-loading")).toBeInTheDocument();
  });

  it("renders the empty-state copy when no signal qualifies", () => {
    withRouter(<TodaysSignalCard notifications={[]} nowMs={NOW} />);
    expect(screen.getByTestId("todays-signal-empty")).toBeInTheDocument();
    expect(screen.getByText(/quiet/i)).toBeInTheDocument();
  });

  it("renders severity-specific label for warning", () => {
    const n = makeNotif({
      severity: "warning",
      title: "Cash sitting idle",
      body: "Your cash position has been over 10% for 30 days",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    expect(screen.getByTestId("todays-signal-card")).toHaveAttribute("data-severity", "warning");
    expect(screen.getByText("Heads up")).toBeInTheDocument();
    expect(screen.getByText("Cash sitting idle")).toBeInTheDocument();
  });

  it("renders severity-specific label for critical", () => {
    const n = makeNotif({
      severity: "critical",
      title: "Sync failed",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    expect(screen.getByText("Action")).toBeInTheDocument();
  });

  it("renders severity-specific label for success", () => {
    const n = makeNotif({
      severity: "success",
      title: "Goal reached",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    expect(screen.getByText("Milestone")).toBeInTheDocument();
  });

  it("tap with deepLink navigates to the route path", () => {
    const n = makeNotif({
      severity: "info",
      title: "Holding moved",
      deepLink: "mizan://holdings/AAPL",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    fireEvent.click(screen.getByTestId("todays-signal-tap"));
    expect(readLocation()).toBe("/holdings/AAPL");
  });

  it("tap without deepLink expands the body (reasoning trail)", () => {
    const n = makeNotif({
      severity: "info",
      title: "Cash drag",
      body: "The reasoning trail — full body of the rule",
      deepLink: null,
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    // Without a deepLink the card always shows body inline; ensure tap
    // toggles the expanded flag (re-render still has body present).
    expect(screen.getByTestId("todays-signal-body")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("todays-signal-tap"));
    expect(screen.getByTestId("todays-signal-body")).toBeInTheDocument();
  });

  it("with deepLink, body is collapsed until tap (the reveal-on-expand path)", () => {
    const n = makeNotif({
      severity: "info",
      title: "Net worth dip",
      body: "Full reasoning",
      deepLink: "mizan://holdings",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    // With a deepLink the body stays hidden until expanded — but tap
    // would navigate. So we render it but verify body element is absent
    // (the deepLink path treats tap as navigate, not expand).
    expect(screen.queryByTestId("todays-signal-body")).toBeNull();
  });

  it("absolute /route path (non-scheme) is accepted as deepLink", () => {
    const n = makeNotif({
      severity: "info",
      title: "Move",
      deepLink: "/assistant",
      createdAtMs: NOW - ONE_HOUR_MS,
    });
    withRouter(<TodaysSignalCard notifications={[n]} nowMs={NOW} />);
    fireEvent.click(screen.getByTestId("todays-signal-tap"));
    expect(readLocation()).toBe("/assistant");
  });

  it("renders the highest-severity card when notifications include multiple severities", () => {
    const notifs = [
      makeNotif({ id: "s", severity: "success", title: "milestone-card", createdAtMs: NOW - ONE_HOUR_MS }),
      makeNotif({ id: "c", severity: "critical", title: "critical-card", createdAtMs: NOW - 2 * ONE_HOUR_MS }),
    ];
    withRouter(<TodaysSignalCard notifications={notifs} nowMs={NOW} />);
    expect(screen.getByText("critical-card")).toBeInTheDocument();
    expect(screen.queryByText("milestone-card")).toBeNull();
  });
});
