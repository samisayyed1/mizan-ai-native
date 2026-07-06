/**
 * POST /api/waitlist/migrate?key=STATS_TOKEN
 *
 * One-shot migration of legacy Supabase waitlist rows into Netlify Blobs.
 *
 * Context: the waitlist previously lived in a Supabase Postgres table.
 * The free-tier project auto-paused after ~10 days idle → every write
 * started 500ing → we migrated to Netlify Blobs so signups are
 * pause-proof. This endpoint bridges the two: it reads every row from
 * the old `public.waitlist` table and upserts it into the Blobs store
 * so the real signups aren't lost.
 *
 * Preconditions the caller must arrange:
 *   - Supabase project restored from paused state (user does this
 *     manually in the Supabase dashboard — free tier)
 *   - `SUPABASE_URL` + `SUPABASE_SERVICE_ROLE` env vars still present
 *     on Netlify (they are, verified via `netlify env:list`)
 *   - `STATS_TOKEN` supplied as the `key` query param (same secret
 *     that gates /api/stats + /stats)
 *
 * Idempotent: re-running is safe because `upsertWaitlistEntry` no-ops
 * on emails that already exist in Blobs. Position numbers from the old
 * table are NOT preserved (Blobs mints a fresh monotonic counter);
 * ref_codes ARE preserved so any old confirmation email links keep
 * working.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";
import { createClient } from "@supabase/supabase-js";

import { getStore } from "@netlify/blobs";
import type { WaitlistEntry } from "@/lib/waitlist-store";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return mismatch === 0;
}

interface LegacyRow {
  email: string;
  country: string;
  pain_point: string | null;
  ref_code: string;
  referred_by: string | null;
  position: number;
  created_at: string;
}

const STORE_NAME = "waitlist";
const COUNTER_KEY = "__meta:counter";

export async function POST(req: NextRequest): Promise<NextResponse> {
  const token = process.env.STATS_TOKEN;
  const key = new URL(req.url).searchParams.get("key") ?? "";
  if (!token || !timingSafeEqual(key, token)) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const supaUrl = process.env.SUPABASE_URL;
  const supaKey = process.env.SUPABASE_SERVICE_ROLE;
  if (!supaUrl || !supaKey) {
    return NextResponse.json(
      { error: "Supabase env not configured on this deploy." },
      { status: 500 },
    );
  }

  // 1) Read every row from the legacy Supabase table.
  const supa = createClient(supaUrl, supaKey, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
  const { data: rows, error } = await supa
    .from("waitlist")
    .select("email, country, pain_point, ref_code, referred_by, position, created_at")
    .order("position", { ascending: true });

  // Extract the project ref (subdomain) from the URL so we can tell
  // the user exactly which Supabase project to un-pause. The full URL
  // isn't secret (only SERVICE_ROLE is), but we return just the ref
  // for a cleaner display.
  const projectRef = supaUrl.match(/https:\/\/([a-z0-9]+)\.supabase\.co/)?.[1] ?? null;

  if (error) {
    return NextResponse.json(
      {
        error: "Supabase read failed — is the project restored?",
        supabaseProjectRef: projectRef,
        supabaseDashboardUrl: projectRef
          ? `https://supabase.com/dashboard/project/${projectRef}`
          : null,
        debug_code: error.code ?? null,
        debug_message: error.message,
      },
      { status: 502 },
    );
  }

  const legacy = (rows ?? []) as LegacyRow[];

  // 2) Upsert every row into Blobs, preserving ref_code + created_at.
  const store = getStore({ name: STORE_NAME, consistency: "strong" });

  let migrated = 0;
  let skipped = 0;
  const errors: { email: string; reason: string }[] = [];
  let maxPosition = 0;

  for (const row of legacy) {
    const emailKey = row.email.toLowerCase().trim();
    try {
      const existing = await store.get(emailKey, { type: "json" });
      if (existing) {
        skipped += 1;
        continue;
      }
      const entry: WaitlistEntry = {
        email: emailKey,
        country: row.country || "Other",
        painPoint: row.pain_point ?? undefined,
        referredBy: row.referred_by ?? undefined,
        refCode: row.ref_code,
        position: row.position,
        createdAt: row.created_at,
      };
      await store.setJSON(emailKey, entry);
      migrated += 1;
      if (row.position > maxPosition) maxPosition = row.position;
    } catch (e) {
      errors.push({
        email: emailKey,
        reason: e instanceof Error ? e.message : String(e),
      });
    }
  }

  // 3) Sync the counter blob to max(legacy position, current counter)
  //    so new signups continue AFTER the imported set — no collisions.
  if (maxPosition > 0) {
    const current = await store.get(COUNTER_KEY, { type: "json" });
    const currentN =
      typeof current === "object" &&
      current !== null &&
      "value" in current &&
      typeof (current as { value: unknown }).value === "number"
        ? (current as { value: number }).value
        : 0;
    if (maxPosition > currentN) {
      await store.setJSON(COUNTER_KEY, { value: maxPosition });
    }
  }

  return NextResponse.json({
    scannedLegacyRows: legacy.length,
    migrated,
    skippedAlreadyPresent: skipped,
    errors,
    counterAfter: maxPosition,
  });
}
