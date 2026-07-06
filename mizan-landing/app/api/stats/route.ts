/**
 * GET /api/stats?key=TOKEN — private waitlist stats for the insider
 * dashboard. Requires the STATS_TOKEN secret; returns 401 otherwise.
 * Never indexed (see robots.ts) and never cached.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import { getWaitlistStats } from "@/lib/stats";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";
export const revalidate = 0;

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return mismatch === 0;
}

export async function GET(req: NextRequest): Promise<NextResponse> {
  const token = process.env.STATS_TOKEN;
  const key = new URL(req.url).searchParams.get("key") ?? "";

  if (!token || !timingSafeEqual(key, token)) {
    return NextResponse.json(
      { error: "unauthorized" },
      { status: 401, headers: { "Cache-Control": "no-store" } },
    );
  }

  try {
    const stats = await getWaitlistStats();
    return NextResponse.json(stats, {
      headers: { "Cache-Control": "no-store" },
    });
  } catch (e) {
    // Surface the underlying failure so we can fix it without a
    // round-trip through the deploy logs. Same pattern as the waitlist
    // route — token check has already passed, so the caller is
    // authorized and the debug info is safe to return.
    console.error("[stats] getWaitlistStats failed", e);
    const message = e instanceof Error ? e.message : String(e);
    // Supabase PostgrestError carries a `code` field. `getSupabase`
    // throws a plain Error with the "Supabase env vars missing" text
    // when SUPABASE_URL/SERVICE_ROLE aren't set.
    const supabaseCode =
      typeof e === "object" && e !== null && "code" in e
        ? String((e as { code: unknown }).code)
        : null;
    return NextResponse.json(
      {
        error: "stats_failed",
        debug_code: supabaseCode,
        debug_message: message,
      },
      {
        status: 500,
        headers: { "Cache-Control": "no-store" },
      },
    );
  }
}
