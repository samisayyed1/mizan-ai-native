import { describe, expect, it } from "vitest";

import {
  dataSafetyMessage,
  errorToPlainString,
  isMizanError,
  parseMizanError,
  retryLabel,
} from "./mizan-error";

describe("parseMizanError", () => {
  it("decodes a structured wire shape", () => {
    const wire = JSON.stringify({
      __mizan_error: true,
      code: "CSV_PARSE_FAILED",
      severity: "error",
      summary: "Couldn't parse the CSV.",
      why: "Header row column mismatch.",
      dataSafety: "untouched",
      nextSteps: ["Open the file in Excel.", "Re-export from your broker."],
      retry: "retryable",
    });

    const parsed = parseMizanError(wire);
    expect(parsed.code).toBe("CSV_PARSE_FAILED");
    expect(parsed.severity).toBe("error");
    expect(parsed.dataSafety).toBe("untouched");
    expect(parsed.nextSteps).toHaveLength(2);
  });

  it("falls back for legacy string errors", () => {
    const parsed = parseMizanError("Something went wrong.");
    expect(parsed.__mizan_error).toBe(true);
    expect(parsed.code).toBe("UNKNOWN_ERROR");
    expect(parsed.summary).toBe("Something went wrong.");
    expect(parsed.retry).toBe("retryable");
  });

  it("strips JSON-shape attempts that don't have the discriminator", () => {
    const parsed = parseMizanError(JSON.stringify({ random: "json" }));
    expect(parsed.code).toBe("UNKNOWN_ERROR");
  });

  it("handles native Error instances", () => {
    const parsed = parseMizanError(new Error("network"));
    expect(parsed.code).toBe("UNKNOWN_ERROR");
    expect(parsed.summary).toBe("network");
  });

  it("recognises a Mizan error wrapped in Error.message", () => {
    const wire = JSON.stringify({
      __mizan_error: true,
      code: "PLAID_AUTH_EXPIRED",
      severity: "warning",
      summary: "Schwab authorisation expired.",
      dataSafety: "untouched",
      nextSteps: ["Reconnect Schwab."],
      retry: "retry_after_prereq",
    });
    const parsed = parseMizanError(new Error(wire));
    expect(parsed.code).toBe("PLAID_AUTH_EXPIRED");
    expect(parsed.retry).toBe("retry_after_prereq");
  });

  it("handles odd thrown values", () => {
    expect(parseMizanError(null).code).toBe("UNKNOWN_ERROR");
    expect(parseMizanError(undefined).code).toBe("UNKNOWN_ERROR");
    expect(parseMizanError(42).code).toBe("UNKNOWN_ERROR");
  });
});

describe("isMizanError", () => {
  it("recognises full shape", () => {
    expect(
      isMizanError({
        __mizan_error: true,
        code: "X",
        summary: "Y",
      }),
    ).toBe(true);
  });

  it("rejects partial shapes", () => {
    expect(isMizanError({ code: "X" })).toBe(false);
    expect(isMizanError({ __mizan_error: false, code: "X", summary: "Y" })).toBe(false);
    expect(isMizanError(null)).toBe(false);
    expect(isMizanError("not an object")).toBe(false);
  });
});

describe("rendering helpers", () => {
  it("renders data-safety messages for every status", () => {
    expect(dataSafetyMessage("untouched")).toContain("safe");
    expect(dataSafetyMessage("rolled_back")).toContain("rolled back");
    expect(dataSafetyMessage("partially_applied")).toContain("Some changes landed");
  });

  it("returns a retry label for retryable errors only", () => {
    expect(retryLabel("retryable")).toBe("Try again");
    expect(retryLabel("retry_after_prereq")).toBe("Try again");
    expect(retryLabel("not_retryable")).toBeNull();
  });

  it("renders a plain-string summary including code when present", () => {
    const err = parseMizanError(
      JSON.stringify({
        __mizan_error: true,
        code: "CSV_PARSE_FAILED",
        severity: "error",
        summary: "Couldn't parse the CSV.",
        dataSafety: "untouched",
        nextSteps: ["Open it in Excel."],
        retry: "retryable",
      }),
    );
    const s = errorToPlainString(err);
    expect(s).toContain("Couldn't parse the CSV.");
    expect(s).toContain("Open it in Excel.");
    expect(s).toContain("CSV_PARSE_FAILED");
  });

  it("hides the [code] footer for legacy fallbacks", () => {
    const s = errorToPlainString(parseMizanError("legacy"));
    expect(s).not.toContain("UNKNOWN_ERROR");
  });
});
