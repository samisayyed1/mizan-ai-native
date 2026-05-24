// Regression guard for the time-of-day bug in chart period buttons.
//
// Background:
//   Clicking "1W" at 5pm used to set `from` to "7 days ago at 5pm" and
//   `to` to "now". Quotes are typically stamped at market close on the
//   calendar day; the earliest-day quote (e.g. 7 days ago at 4pm close)
//   would fall *before* `from` and silently drop out of the chart, so
//   the leftmost day in the requested window was missing every time.
//
// What this test does:
//   Exercises getInitialIntervalData for every TimePeriod code and
//   asserts that the returned `from`/`to` sit on day boundaries, so a
//   quote stamped anywhere within the boundary day is included by a
//   `date >= from && date <= to` filter.

import { describe, expect, it } from "vitest";
import { getInitialIntervalData, type TimePeriod } from "@mizan/ui";

const PERIODS: TimePeriod[] = ["1D", "1W", "1M", "3M", "6M", "YTD", "1Y", "5Y", "ALL"];

function isStartOfDay(d: Date) {
  return (
    d.getHours() === 0 && d.getMinutes() === 0 && d.getSeconds() === 0 && d.getMilliseconds() === 0
  );
}

function isEndOfDay(d: Date) {
  return (
    d.getHours() === 23 &&
    d.getMinutes() === 59 &&
    d.getSeconds() === 59 &&
    d.getMilliseconds() === 999
  );
}

describe("IntervalSelector range computation", () => {
  it("every relative period's `from` lands at start-of-day (00:00:00.000 local)", () => {
    // ALL is a sentinel (epoch), not a calendar boundary — excluded.
    for (const code of PERIODS.filter((p) => p !== "ALL")) {
      const { range } = getInitialIntervalData(code);
      expect(range, `range missing for ${code}`).toBeDefined();
      expect(
        isStartOfDay(range!.from!),
        `${code}.from is not start-of-day: ${range!.from!.toISOString()}`,
      ).toBe(true);
    }
  });

  it("every period's `to` lands at end-of-day (23:59:59.999 local)", () => {
    for (const code of PERIODS) {
      const { range } = getInitialIntervalData(code);
      expect(range, `range missing for ${code}`).toBeDefined();
      expect(
        isEndOfDay(range!.to!),
        `${code}.to is not end-of-day: ${range!.to!.toISOString()}`,
      ).toBe(true);
    }
  });

  it("includes a quote stamped at market-close 7 days ago when 1W is selected", () => {
    // Reproduces the original bug: pre-fix, this assertion failed
    // because `from` was "7 days ago at the current time of day" and
    // the simulated market-close quote was stamped earlier in the day.
    const { range } = getInitialIntervalData("1W");
    expect(range).toBeDefined();
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    // Simulated 4 PM market close on that day.
    sevenDaysAgo.setHours(16, 0, 0, 0);
    expect(sevenDaysAgo.getTime()).toBeGreaterThanOrEqual(range!.from!.getTime());
    expect(sevenDaysAgo.getTime()).toBeLessThanOrEqual(range!.to!.getTime());
  });

  it("includes a quote stamped earlier today (before 'now') when 1D is selected", () => {
    // The end of the window must include today's full day, not just up
    // to the current moment, otherwise late-arriving intraday quotes
    // disappear.
    const { range } = getInitialIntervalData("1D");
    expect(range).toBeDefined();
    const today = new Date();
    today.setHours(0, 1, 0, 0); // 12:01 AM today
    expect(today.getTime()).toBeGreaterThanOrEqual(range!.from!.getTime());
    expect(today.getTime()).toBeLessThanOrEqual(range!.to!.getTime());
  });
});
