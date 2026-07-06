/**
 * POST /api/waitlist
 *
 * Idempotent waitlist signup.
 *
 * Flow:
 *   1. Rate-limit by IP (5 / 10min, Upstash sliding window; in-memory
 *      fallback when Upstash env is missing — dev convenience only).
 *   2. Parse + validate via zod.
 *   3. Insert into `public.waitlist`. On 23505 (unique violation),
 *      look up the existing row and return its position + ref_code so
 *      a refresh / re-submit is a no-op for the user.
 *   4. Fire confirmation email via Resend (stubbed if no API key).
 *   5. Fire Plausible custom event server-side (best-effort, never
 *      throws).
 *
 * Errors are surfaced as `{ error: string }` with conventional status
 * codes so the client can show a usable message without parsing.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import { ConfirmEmail } from "@/emails/Confirm";
import { rateLimit } from "@/lib/rate-limit";
import { getResend, RESEND_FROM } from "@/lib/resend";
import { type WaitlistResponse, waitlistSchema } from "@/lib/schemas";
import { getSupabase } from "@/lib/supabase";

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

  // 3) Insert.
  let supabase;
  try {
    supabase = getSupabase();
  } catch (e) {
    // Env var misconfiguration — surface it so we don't waste an hour
    // hunting a mystery 500 next time envs drop out of the deploy.
    console.error("[waitlist] supabase client init failed", e);
    return NextResponse.json(
      {
        error: "Signup service unavailable — Supabase env vars missing.",
        debug_code: "supabase_env_missing",
      },
      { status: 500 },
    );
  }
  const insertPayload: Record<string, unknown> = { email, country };
  if (painPoint) insertPayload.pain_point = painPoint;
  if (ref) insertPayload.referred_by = ref;

  const inserted = await supabase
    .from("waitlist")
    .insert(insertPayload)
    .select("position, ref_code")
    .single();

  let position: number;
  let refCode: string;

  if (inserted.error) {
    // Unique violation → idempotent re-submit.
    if (inserted.error.code === "23505") {
      const existing = await supabase
        .from("waitlist")
        .select("position, ref_code")
        .eq("email", email)
        .single();
      if (existing.error || !existing.data) {
        return NextResponse.json(
          { error: "Could not retrieve existing signup." },
          { status: 500 },
        );
      }
      position = Number(existing.data.position);
      refCode = existing.data.ref_code;
      // 409 + the existing record so the form can show the success state.
      return NextResponse.json(
        { position, refCode, alreadyRegistered: true },
        { status: 409 },
      );
    }
    // Log the underlying Postgres error to Netlify function logs so we
    // can diagnose prod-only failures. Also surface a compact
    // `debug_code` in the response — Supabase / Postgres error codes are
    // not sensitive (they identify constraint violations, missing
    // tables, RLS blocks, etc.), and this saves us a round-trip through
    // "user reports failure → dev asks for logs → user pastes them" for
    // every prod incident.
    console.error("[waitlist] insert failed", {
      code: inserted.error.code,
      message: inserted.error.message,
      details: inserted.error.details,
      hint: inserted.error.hint,
    });
    return NextResponse.json(
      {
        error: "Could not save signup.",
        debug_code: inserted.error.code ?? null,
        debug_message: inserted.error.message ?? null,
      },
      { status: 500 },
    );
  }

  position = Number(inserted.data.position);
  refCode = inserted.data.ref_code;

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
      react: ConfirmEmail({ refCode, siteUrl: SITE_URL }),
    }),
  ]);

  // 5) Server-side analytics, fire-and-forget.
  void fireServerPlausible(req);

  return NextResponse.json({ position, refCode }, { status: 201 });
}
