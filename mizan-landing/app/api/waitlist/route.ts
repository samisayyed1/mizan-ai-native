/**
 * POST /api/waitlist
 *
 * Idempotent waitlist signup, backed by Netlify Blobs.
 *
 * Flow:
 *   1. Rate-limit by IP (5 / 10min, Upstash sliding window; in-memory
 *      fallback when Upstash env is missing — dev convenience only).
 *   2. Parse + validate via zod.
 *   3. Upsert into the Netlify Blobs `waitlist` store. Duplicate email
 *      returns the existing position + ref_code (409) so a refresh /
 *      re-submit is a no-op for the user.
 *   4. Fire confirmation email via Resend (stubbed if no API key).
 *   5. Fire Plausible custom event server-side (best-effort, never
 *      throws).
 *
 * Errors are surfaced as `{ error: string }` with conventional status
 * codes so the client can show a usable message without parsing.
 *
 * Storage note: this used to be Supabase. Moved to Netlify Blobs on
 * 2026-06-21 after the free-tier Supabase project auto-paused and
 * every signup started returning 500. Blobs is native to Netlify, has
 * no idle-pause behaviour, and needs no external configuration.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import { ConfirmEmail } from "@/emails/Confirm";
import { rateLimit } from "@/lib/rate-limit";
import { getResend, RESEND_FROM } from "@/lib/resend";
import { type WaitlistResponse, waitlistSchema } from "@/lib/schemas";
import { upsertWaitlistEntry } from "@/lib/waitlist-store";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const SITE_URL =
  process.env.NEXT_PUBLIC_SITE_URL?.replace(/\/$/, "") ??
  "https://getmizan.net";

function clientIp(req: NextRequest): string {
  const forwarded = req.headers.get("x-forwarded-for");
  if (forwarded) return forwarded.split(",")[0]!.trim();
  return req.headers.get("x-real-ip") ?? "anon";
}

async function fireServerPlausible(req: NextRequest): Promise<void> {
  const domain = process.env.NEXT_PUBLIC_PLAUSIBLE_DOMAIN;
  if (!domain) return;
  try {
    await fetch("https://plausible.io/api/event", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": req.headers.get("user-agent") ?? "Mizan-Server",
        "X-Forwarded-For": clientIp(req),
      },
      body: JSON.stringify({
        name: "waitlist_signup",
        url: `${SITE_URL}/`,
        domain,
      }),
    });
  } catch {
    // Plausible failures must never break the signup path.
  }
}

export async function POST(req: NextRequest): Promise<NextResponse<WaitlistResponse | { error: string }>> {
  // 1) Rate limit.
  const ip = clientIp(req);
  const limited = await rateLimit(ip);
  if (!limited.success) {
    return NextResponse.json(
      { error: "Too many attempts. Try again shortly." },
      { status: 429, headers: { "Retry-After": String(limited.retryAfter) } },
    );
  }

  // 2) Parse + validate.
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "Invalid JSON body." }, { status: 400 });
  }
  const parsed = waitlistSchema.safeParse(body);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    return NextResponse.json(
      { error: first?.message ?? "Invalid input." },
      { status: 400 },
    );
  }
  const { email, country, painPoint, ref } = parsed.data;

  // 3) Upsert into Blobs.
  let result;
  try {
    result = await upsertWaitlistEntry({
      email,
      country,
      painPoint: painPoint ?? undefined,
      referredBy: ref ?? undefined,
    });
  } catch (e) {
    console.error("[waitlist] blob upsert failed", e);
    return NextResponse.json(
      {
        error: "Could not save signup.",
        debug_message: e instanceof Error ? e.message : String(e),
      },
      { status: 500 },
    );
  }

  const { inserted, entry } = result;

  // Duplicate → 409 + existing record so the form can show the success state.
  if (!inserted) {
    return NextResponse.json(
      { position: entry.position, refCode: entry.refCode, alreadyRegistered: true },
      { status: 409 },
    );
  }

  // 4) Confirmation email — fire and forget the analytics, but await
  //    the send so we can log failures into the response shape if
  //    needed in the future. Resend client returns `{skipped:true}`
  //    when RESEND_API_KEY isn't set.
  const resend = getResend();
  // Add to the Resend Audience (the mailing list we broadcast to at
  // launch) + send the confirmation. Both no-op cleanly without keys.
  // Run concurrently; neither blocks the success response on failure.
  await Promise.allSettled([
    resend.addContact(email),
    resend.emails.send({
      from: RESEND_FROM,
      to: email,
      subject: "You're on the Mizan waitlist",
      react: ConfirmEmail({ refCode: entry.refCode, siteUrl: SITE_URL }),
    }),
  ]);

  // 5) Server-side analytics, fire-and-forget.
  void fireServerPlausible(req);

  return NextResponse.json(
    { position: entry.position, refCode: entry.refCode },
    { status: 201 },
  );
}
