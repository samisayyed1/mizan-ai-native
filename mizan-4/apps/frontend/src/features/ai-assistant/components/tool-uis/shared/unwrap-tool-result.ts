/**
 * Shared helper for tool-UI `normaliseResult` paths.
 *
 * The chat runtime in `use-chat-runtime.ts` wraps tool results with a
 * `meta` field as `{data, meta}` (see line 333). Every tool-UI's
 * normaliser was looking at the top-level shape (`obj.draft`,
 * `obj.target`, etc.), which silently failed when `meta` was set —
 * the card rendered the empty / error state ("No asset data.",
 * "No draft available.") even though the tool succeeded.
 *
 * `unwrapToolResult` peels the wrapper once if the inner `data`
 * object contains the discriminator key the caller is checking for.
 * Idempotent — calling it on an already-unwrapped value returns the
 * value unchanged.
 *
 * Usage:
 *   const obj = unwrapToolResult(raw, "draft");
 *   if (!obj || !obj.draft) return null;
 */
export function unwrapToolResult(raw: unknown, key: string): unknown {
  if (!raw || typeof raw !== "object") return raw;
  const obj = raw as Record<string, unknown>;
  // Already in the right shape — top-level has the discriminator.
  if (key in obj) return raw;
  // Wrapped: `{data, meta}`. Unwrap once.
  const data = obj.data;
  if (data && typeof data === "object" && key in (data as object)) {
    return data;
  }
  return raw;
}
