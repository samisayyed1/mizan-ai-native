/**
 * Mizan Connect runtime configuration.
 *
 * Two layers:
 *
 *   1. **Build-time** (`CONNECT_ENABLED_AT_BUILD`): true when the .env
 *      had both `CONNECT_AUTH_URL` and `CONNECT_AUTH_PUBLISHABLE_KEY`
 *      set when `pnpm tauri build` ran. Production binaries from the
 *      Mizan site should be built this way — zero network round-trip
 *      on first launch, sign-in renders instantly.
 *
 *   2. **Runtime self-discovery** (`resolveConnectConfig`): when the
 *      build-time values are missing but `CONNECT_API_URL` is set,
 *      we fetch `GET /api/v1/config/public` on first load to pick up
 *      `supabaseUrl` + `supabasePublishableKey`. This lets developers
 *      ship dev builds with just the cloud URL and lets operators
 *      rotate Supabase keys without re-cutting installers.
 *
 * Both values are PUBLIC (Supabase explicitly markets the anon key
 * as client-embeddable; RLS policies protect tenant data).
 *
 * The provider awaits `resolveConnectConfig()` once on mount; until
 * it resolves, `isInitializing` is true and no Mizan Connect UI
 * renders. After resolution, `CONNECT_ENABLED` reflects whichever
 * source produced a complete config.
 */

const BUILD_AUTH_URL = (import.meta.env.CONNECT_AUTH_URL as string | undefined) || "";
const BUILD_AUTH_KEY =
  (import.meta.env.CONNECT_AUTH_PUBLISHABLE_KEY as string | undefined) || "";
const API_URL = (import.meta.env.CONNECT_API_URL as string | undefined) || "";

/** True iff the build had both Supabase URL + anon key baked in. */
export const CONNECT_ENABLED_AT_BUILD = Boolean(BUILD_AUTH_URL && BUILD_AUTH_KEY);

export interface ResolvedConnectConfig {
  /** Final Supabase URL — build-time wins; runtime fills in. */
  supabaseUrl: string;
  /** Final Supabase anon/publishable key. */
  supabasePublishableKey: string;
  /** Whether the cloud has Plaid enabled. Defaults true at build time
   *  (so the UI doesn't pre-hide upgrade CTAs on assumption); the
   *  runtime fetch refines it. */
  plaidEnabled: boolean;
  /** Whether the cloud has Stripe billing enabled. */
  billingEnabled: boolean;
  /** Which source produced this resolution — useful for diagnostics. */
  source: "build" | "runtime";
}

/**
 * Result type for the discovery attempt.
 *
 * `enabled: false` means we couldn't get a complete config from any
 * source — the UI surfaces an honest "Mizan Connect not configured"
 * state with operator-facing remediation in the detail string.
 */
export type ConnectConfigState =
  | { enabled: true; config: ResolvedConnectConfig }
  | { enabled: false; reason: string };

let cachedResolution: Promise<ConnectConfigState> | null = null;

/**
 * One-shot resolver. Caches the first result so navigation doesn't
 * re-hit the cloud. Throws are caught and surfaced as
 * `enabled: false`; we never crash the app over a missing optional.
 */
export function resolveConnectConfig(): Promise<ConnectConfigState> {
  if (cachedResolution) return cachedResolution;

  cachedResolution = (async (): Promise<ConnectConfigState> => {
    // Build-time wins. No network round-trip needed.
    if (BUILD_AUTH_URL && BUILD_AUTH_KEY) {
      return {
        enabled: true,
        config: {
          supabaseUrl: BUILD_AUTH_URL,
          supabasePublishableKey: BUILD_AUTH_KEY,
          plaidEnabled: true,
          billingEnabled: true,
          source: "build",
        },
      };
    }

    // No cloud URL means there's nowhere to discover from — fully
    // offline build. Surface that honestly.
    if (!API_URL) {
      return {
        enabled: false,
        reason:
          "Built without CONNECT_API_URL. Drop it into .env to enable Mizan Connect.",
      };
    }

    // Runtime fetch. Plain unauthenticated GET — the response is
    // intentionally public.
    try {
      const url = `${API_URL.replace(/\/+$/, "")}/api/v1/config/public`;
      const ctrl = new AbortController();
      const timeout = setTimeout(() => ctrl.abort(), 5000);
      const res = await fetch(url, { signal: ctrl.signal });
      clearTimeout(timeout);
      if (!res.ok) {
        return {
          enabled: false,
          reason: `Cloud config endpoint returned HTTP ${res.status}.`,
        };
      }
      const json = (await res.json()) as {
        supabaseUrl?: string;
        supabasePublishableKey?: string | null;
        plaidEnabled?: boolean;
        billingEnabled?: boolean;
      };

      if (!json.supabaseUrl || !json.supabasePublishableKey) {
        return {
          enabled: false,
          reason:
            "Cloud is reachable but no Supabase publishable key is set. " +
            "Run: fly secrets set --app mizan-connect SUPABASE_PUBLISHABLE_KEY=eyJhbG...",
        };
      }

      return {
        enabled: true,
        config: {
          supabaseUrl: json.supabaseUrl,
          supabasePublishableKey: json.supabasePublishableKey,
          plaidEnabled: json.plaidEnabled ?? true,
          billingEnabled: json.billingEnabled ?? true,
          source: "runtime",
        },
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      return {
        enabled: false,
        reason: `Cloud config fetch failed: ${msg}`,
      };
    }
  })();

  return cachedResolution;
}

/** Reset the cache. Useful in tests + when the user changes networks. */
export function resetConnectConfigCache(): void {
  cachedResolution = null;
}

/**
 * Backwards-compatible synchronous flag.
 *
 * Returns `true` whenever the build-time path is enabled (the most
 * common production case). Modules that already gate on this constant
 * continue to work for build-time-configured installs; the runtime
 * path is handled by the provider via `resolveConnectConfig()`.
 *
 * **Caveat:** when the build was offline-only but the cloud later
 * provides config via runtime, this constant is still false at module
 * eval time. The provider takes that into account and uses the
 * resolved config directly instead of this flag. Keep this constant
 * for compatibility but prefer the resolver inside async code paths.
 */
export const CONNECT_ENABLED = CONNECT_ENABLED_AT_BUILD;
