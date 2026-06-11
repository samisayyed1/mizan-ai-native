import "server-only";

import { getSupabase } from "@/lib/supabase";

export interface WaitlistStats {
  total: number;
  last24h: number;
  last7d: number;
  countries: { country: string; count: number }[];
  latestAt: string | null;
}

/**
 * Aggregate the waitlist for the private insider dashboard. The list is
 * small pre-launch, so we pull country + created_at for every row and
 * aggregate in JS — no SQL group-by needed. Service-role only.
 */
export async function getWaitlistStats(): Promise<WaitlistStats> {
  const supabase = getSupabase();
  const { data, error } = await supabase
    .from("waitlist")
    .select("country, created_at");

  if (error) throw new Error(error.message);

  const rows = data ?? [];
  const now = Date.now();
  const DAY = 86_400_000;

  let last24h = 0;
  let last7d = 0;
  let latest = 0;
  const byCountry = new Map<string, number>();

  for (const r of rows) {
    const t = new Date(r.created_at as string).getTime();
    if (now - t < DAY) last24h += 1;
    if (now - t < 7 * DAY) last7d += 1;
    if (t > latest) latest = t;
    const c = (r.country as string) || "Other";
    byCountry.set(c, (byCountry.get(c) ?? 0) + 1);
  }

  const countries = Array.from(byCountry.entries())
    .map(([country, count]) => ({ country, count }))
    .sort((a, b) => b.count - a.count);

  return {
    total: rows.length,
    last24h,
    last7d,
    countries,
    latestAt: latest ? new Date(latest).toISOString() : null,
  };
}
