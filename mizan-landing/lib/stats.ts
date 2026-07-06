import "server-only";

/**
 * Backwards-compatible re-export of the waitlist stats implementation.
 *
 * Storage moved from Supabase → Netlify Blobs on 2026-06-21 after the
 * free-tier Supabase project auto-paused and started returning 500 on
 * every signup + stats read. Netlify Blobs is native to the hosting
 * platform, never pauses, and needs zero external configuration. See
 * `lib/waitlist-store.ts` for the storage contract.
 *
 * Keeping this module as a thin shim so existing imports
 * (`getWaitlistStats`, `WaitlistStats`, `WaitlistSignup`) don't need
 * to change.
 */
export {
  getWaitlistStatsFromBlobs as getWaitlistStats,
  type WaitlistStats,
  type WaitlistStatsSignup as WaitlistSignup,
} from "@/lib/waitlist-store";
