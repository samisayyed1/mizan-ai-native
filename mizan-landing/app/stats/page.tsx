import type { Metadata } from "next";

import { getWaitlistStats } from "@/lib/stats";
import { StatsDashboard } from "./StatsDashboard";
import { StatsLock } from "./StatsLock";

// Never index this page, and never cache it.
export const metadata: Metadata = {
  title: "Insider",
  robots: { index: false, follow: false, nocache: true },
};
export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";
export const revalidate = 0;

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return mismatch === 0;
}

export default async function StatsPage({
  searchParams,
}: {
  searchParams: Promise<{ key?: string }>;
}) {
  const { key = "" } = await searchParams;
  const token = process.env.STATS_TOKEN;
  const authed = !!token && timingSafeEqual(key, token);

  if (!authed) {
    return <StatsLock />;
  }

  // Seed the dashboard with server-fetched data so it's instant; the
  // client island then polls for live updates.
  let initial = null;
  let fetchError: string | null = null;
  try {
    initial = await getWaitlistStats();
  } catch (e) {
    // Log for Netlify function output + surface a short message on the
    // page so the auth'd user knows the auth worked but the DB read
    // failed (usually missing Supabase env or missing waitlist table).
    console.error("[stats/page] getWaitlistStats failed", e);
    fetchError = e instanceof Error ? e.message : String(e);
    initial = null;
  }

  return (
    <StatsDashboard token={key} initial={initial} fetchError={fetchError} />
  );
}
