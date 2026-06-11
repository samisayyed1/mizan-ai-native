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
  } catch {
    return NextResponse.json({ error: "stats_failed" }, { status: 500 });
  }
}
