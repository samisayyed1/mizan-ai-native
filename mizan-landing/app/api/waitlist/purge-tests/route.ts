/**
 * POST /api/waitlist/purge-tests?key=STATS_TOKEN
 *
 * Delete the diagnostic test signups this session wrote while wiring up
 * the Netlify Blobs migration (emails matching known test patterns).
 * One-shot cleanup — safe to remove after the real migration runs.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import { getStore } from "@netlify/blobs";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return mismatch === 0;
}

const TEST_EMAILS: readonly string[] = [
  "sami-post-migration@tenetlabs.uk",
  "sami-v2@tenetlabs.uk",
  "ibrar-v2@example.com",
  "debug-test-01@example.com",
  "debug-post-deploy@example.com",
  "debug@example.com",
];

export async function POST(req: NextRequest): Promise<NextResponse> {
  const token = process.env.STATS_TOKEN;
  const key = new URL(req.url).searchParams.get("key") ?? "";
  if (!token || !timingSafeEqual(key, token)) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const store = getStore({ name: "waitlist", consistency: "strong" });
  const deleted: string[] = [];
  const missing: string[] = [];

  for (const email of TEST_EMAILS) {
    const existing = await store.get(email, { type: "json" });
    if (existing) {
      await store.delete(email);
      deleted.push(email);
    } else {
      missing.push(email);
    }
  }

  return NextResponse.json({ deleted, missing });
}
