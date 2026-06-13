/**
 * GET /i/{ref} — invite-link redirect.
 *
 * Every signup gets a shareable link `https://getmizan.net/i/{refCode}`
 * embedded in their confirmation card + email. This handler validates
 * the code's format, confirms it exists in Supabase, and forwards the
 * visitor to the waitlist form with `?ref=XXX` so the referrer is
 * captured on insert (the form already reads `?ref` from the URL).
 *
 * Why a server-side redirect:
 *   - 302 is fast, cacheable at the edge, no flash of "loading…"
 *   - Validating against Supabase up front means a typo'd / fake ref
 *     never reaches the form (and the FK constraint on `referred_by`
 *     never has a chance to 500 the signup downstream).
 *   - Bad / unknown refs degrade gracefully — we drop the ref param
 *     and forward to the home page, so the visitor can still join.
 */
import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import { getSupabase } from "@/lib/supabase";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";

// 8 alphanumerics — matches the DB's ref_code DEFAULT
// (`substr(encode(gen_random_bytes(6),'base64'), 1, 8)` with `+`/`/`
// replaced).
const REF_PATTERN = /^[A-Za-z0-9]{8}$/;

function safeOrigin(req: NextRequest): string {
  // Prefer the canonical site URL; fall back to the request origin in
  // case the env var is missing on a preview deploy.
  const site = process.env.NEXT_PUBLIC_SITE_URL;
  if (site) return site.replace(/\/$/, "");
  return new URL(req.url).origin;
}

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ ref: string }> },
): Promise<NextResponse> {
  const { ref } = await params;
  const origin = safeOrigin(req);

  // Bad format → forward to home, drop the bad ref entirely so we don't
  // poison the form with a value the FK will reject.
  if (!REF_PATTERN.test(ref)) {
    return NextResponse.redirect(`${origin}/#waitlist`, { status: 302 });
  }

  // Validate the ref actually exists. Single indexed lookup, sub-50ms.
  // If Supabase is down, we fail OPEN — better to pass a possibly-valid
  // ref through than block the visitor from signing up.
  let isValid = true;
  try {
    const supabase = getSupabase();
    const { data, error } = await supabase
      .from("waitlist")
      .select("ref_code")
      .eq("ref_code", ref)
      .maybeSingle();
    isValid = !error && !!data;
  } catch {
    isValid = true;
  }

  const dest = isValid
    ? `${origin}/?ref=${encodeURIComponent(ref)}#waitlist`
    : `${origin}/#waitlist`;

  return NextResponse.redirect(dest, { status: 302 });
}
