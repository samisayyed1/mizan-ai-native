import { parseGatedError } from "./gated-error";

/**
 * Module-level bridge so the global TanStack `MutationCache.onError` (created
 * above the React provider tree) can route backend GatedErrors to the upgrade
 * modal without prop-drilling. `UpgradeGateProvider` registers the handler on
 * mount; the mutation cache calls `emitGatedError` before showing a toast.
 */
type Handler = (feature: string) => void;

let handler: Handler | null = null;

export function setGatedErrorHandler(fn: Handler | null): void {
  handler = fn;
}

/**
 * If `error` is a GatedError and a handler is registered, raise the upgrade
 * modal and return `true` (so the caller suppresses its generic error UI).
 */
export function emitGatedError(error: unknown): boolean {
  const gated = parseGatedError(error);
  if (gated && handler) {
    handler(gated.feature);
    return true;
  }
  return false;
}
