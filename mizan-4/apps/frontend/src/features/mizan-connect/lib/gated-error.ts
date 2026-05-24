import type { GatedError } from "../types";

/**
 * Decode a backend [`GatedError`] from a rejected command/HTTP call.
 *
 * Premium Rust commands reject with a JSON-encoded `{ "__gated": true, … }`
 * payload (see `commands/entitlements.rs`). Tauri surfaces that as a string;
 * the web adapter as an `Error` whose message is the body. This normalizes
 * both (and a raw object) into a typed `GatedError`, or `null` when the error
 * is something else.
 */
export function parseGatedError(error: unknown): GatedError | null {
  const candidate = extractGatedObject(error);
  if (
    candidate &&
    typeof candidate === "object" &&
    (candidate as Record<string, unknown>).__gated === true &&
    typeof (candidate as Record<string, unknown>).feature === "string"
  ) {
    return candidate as GatedError;
  }
  return null;
}

function extractGatedObject(error: unknown): unknown {
  if (error && typeof error === "object" && "__gated" in error) {
    return error;
  }
  const raw =
    typeof error === "string" ? error : error instanceof Error ? error.message : undefined;
  if (!raw) return null;
  // Find the JSON object even if it's embedded in a larger message.
  const start = raw.indexOf("{");
  const end = raw.lastIndexOf("}");
  if (start === -1 || end === -1 || end < start) return null;
  try {
    return JSON.parse(raw.slice(start, end + 1));
  } catch {
    return null;
  }
}
