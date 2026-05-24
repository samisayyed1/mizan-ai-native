/**
 * Structured Mizan error decoder — frontend half of the §A24 "No silent
 * failure" contract.
 *
 * Tauri commands carry the structured error across the IPC boundary as a
 * JSON string with a `__mizan_error: true` discriminator. Anywhere the
 * frontend handles a thrown command-rejection error, run the raw message
 * through [`parseMizanError`] to either get the structured shape or a
 * sensible legacy fallback.
 *
 * Mirrors `crates/core/src/mizan_error.rs`.
 */

export type MizanErrorSeverity = "warning" | "error" | "critical";
export type DataSafetyStatus = "untouched" | "rolled_back" | "partially_applied";
export type RetryPolicy = "not_retryable" | "retryable" | "retry_after_prereq";

export interface MizanError {
  __mizan_error: true;
  code: string;
  severity: MizanErrorSeverity;
  summary: string;
  why?: string;
  dataSafety: DataSafetyStatus;
  nextSteps: string[];
  retry: RetryPolicy;
  raw?: string;
}

const FALLBACK_CODE = "UNKNOWN_ERROR";

/**
 * Parse anything thrown out of a Tauri command. Returns:
 *   - the structured MizanError when the wire shape matches,
 *   - or a synthetic MizanError wrapping the legacy string message.
 *
 * Always returns a MizanError so call sites can render uniformly. Never
 * throws. Never returns null/undefined.
 */
export function parseMizanError(thrown: unknown): MizanError {
  // 1. Already an instance? (Rare — most thrown values are strings from Tauri.)
  if (isMizanError(thrown)) return thrown;

  // 2. String wire shape — try JSON.parse first, then fall back to legacy.
  if (typeof thrown === "string") {
    const trimmed = thrown.trim();
    if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
      try {
        const parsed = JSON.parse(trimmed);
        if (isMizanError(parsed)) return parsed;
      } catch {
        // fall through to legacy
      }
    }
    return legacyFallback(thrown);
  }

  // 3. Native Error instances (network, JS bugs).
  if (thrown instanceof Error) {
    // Some error.message values themselves carry the JSON payload.
    const nested = parseMizanError(thrown.message);
    if (nested.code !== FALLBACK_CODE) return nested;
    return legacyFallback(thrown.message);
  }

  // 4. Anything else.
  return legacyFallback(String(thrown));
}

/** Type guard for already-parsed MizanError shapes. */
export function isMizanError(value: unknown): value is MizanError {
  if (!value || typeof value !== "object") return false;
  const obj = value as Partial<MizanError>;
  return (
    obj.__mizan_error === true &&
    typeof obj.code === "string" &&
    typeof obj.summary === "string"
  );
}

/**
 * Wraps a plain string error in the structured shape so call sites can
 * render the unified UI without branching. Code = "UNKNOWN_ERROR" is the
 * tell to suppress the "code: …" footer in the toast.
 */
function legacyFallback(message: string): MizanError {
  const summary = message?.trim() || "Something went wrong.";
  return {
    __mizan_error: true,
    code: FALLBACK_CODE,
    severity: "error",
    summary,
    dataSafety: "untouched",
    nextSteps: ["Try again. If it persists, send a diagnostic report."],
    retry: "retryable",
    raw: message,
  };
}

// ───────────────────────────────────────────────────────────────────────────
// Rendering helpers (kept here so toasts + banners share one set of strings)
// ───────────────────────────────────────────────────────────────────────────

/** A one-line data-safety blurb the toast can render under the summary. */
export function dataSafetyMessage(status: DataSafetyStatus): string {
  switch (status) {
    case "untouched":
      return "Your Mizan data is safe — nothing was changed.";
    case "rolled_back":
      return "Your Mizan data is safe — partial changes were rolled back.";
    case "partially_applied":
      return "Some changes landed before the failure. Check the affected items before retrying.";
  }
}

/** Label for the retry button, or null if retry doesn't make sense. */
export function retryLabel(policy: RetryPolicy): string | null {
  switch (policy) {
    case "retryable":
      return "Try again";
    case "retry_after_prereq":
      return "Try again";
    case "not_retryable":
      return null;
  }
}

/**
 * Convert the structured error to a plain string fallback (for places that
 * can't render a rich toast yet — e.g. legacy console logging).
 */
export function errorToPlainString(err: MizanError): string {
  const parts = [err.summary];
  if (err.why) parts.push(err.why);
  if (err.nextSteps.length > 0) parts.push(err.nextSteps.join(" "));
  if (err.code !== FALLBACK_CODE) parts.push(`[${err.code}]`);
  return parts.join(" ");
}
