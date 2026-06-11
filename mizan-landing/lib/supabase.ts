import "server-only";

import { createClient, type SupabaseClient } from "@supabase/supabase-js";

/**
 * Service-role Supabase client. Server-only — the `server-only`
 * import above will throw at build time if any client component
 * accidentally imports this module.
 *
 * Lazily instantiated so the module doesn't crash on import when the
 * service-role env var hasn't been provisioned yet (local dev without
 * the secret set, preview builds for branches without Supabase
 * configured, etc).
 */
let cached: SupabaseClient | undefined;

export function getSupabase(): SupabaseClient {
  if (cached) return cached;

  const url = process.env.SUPABASE_URL;
  const serviceRole = process.env.SUPABASE_SERVICE_ROLE;

  if (!url || !serviceRole) {
    throw new Error(
      "Supabase env vars missing: set SUPABASE_URL + SUPABASE_SERVICE_ROLE",
    );
  }

  cached = createClient(url, serviceRole, {
    auth: { persistSession: false, autoRefreshToken: false },
    global: {
      // Next.js patches global fetch and caches by default — which made
      // the /stats count freeze at its first value. Force every Supabase
      // request to be uncached so reads are always live.
      fetch: (input, init) =>
        fetch(input, { ...init, cache: "no-store" }),
    },
  });
  return cached;
}
