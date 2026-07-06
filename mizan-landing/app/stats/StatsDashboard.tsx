"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { Container } from "@/app/(landing)/_primitives/Container";
import { Wordmark } from "@/app/(landing)/_primitives/Wordmark";
import type { WaitlistStats } from "@/lib/stats";

const POLL_MS = 12_000;

function useCountUp(target: number, duration = 700): number {
  const [value, setValue] = useState(target);
  const fromRef = useRef(target);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const from = fromRef.current;
    if (from === target) return;
    const start = performance.now();
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = 1 - Math.pow(1 - t, 3); // easeOutCubic
      setValue(Math.round(from + (target - from) * eased));
      if (t < 1) rafRef.current = requestAnimationFrame(tick);
      else fromRef.current = target;
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      fromRef.current = target;
    };
  }, [target, duration]);

  return value;
}

function relative(iso: string | null): string {
  if (!iso) return "—";
  const diff = Date.now() - new Date(iso).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

export function StatsDashboard({
  token,
  initial,
  fetchError,
}: {
  token: string;
  initial: WaitlistStats | null;
  fetchError?: string | null;
}) {
  const [stats, setStats] = useState<WaitlistStats | null>(initial);
  const [updatedAt, setUpdatedAt] = useState<number>(Date.now());
  // Re-render once a second so "updated Xs ago" + relative times stay
  // fresh. We only need the setter; the value is discarded.
  const [, forceRerender] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const res = await fetch(`/api/stats?key=${encodeURIComponent(token)}`, {
        cache: "no-store",
      });
      if (!res.ok) return;
      const data = (await res.json()) as WaitlistStats;
      setStats(data);
      setUpdatedAt(Date.now());
    } catch {
      // keep last good data
    }
  }, [token]);

  useEffect(() => {
    const id = setInterval(() => {
      if (!document.hidden) refresh();
    }, POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  // tick every second so the "updated Xs ago" + latest stay fresh
  useEffect(() => {
    const id = setInterval(() => forceRerender((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const total = stats?.total ?? 0;
  const displayed = useCountUp(total);
  const secsAgo = Math.max(0, Math.floor((Date.now() - updatedAt) / 1000));
  const maxCountry = stats?.countries[0]?.count ?? 1;

  return (
    <main className="min-h-screen bg-depth-page py-10 md:py-16">
      <Container className="max-w-2xl">
        {/* Header */}
        <div className="flex items-center justify-between">
          <Wordmark size="sm" />
          <span className="inline-flex items-center gap-2 rounded-full border border-depth-border bg-depth-card px-3 py-1">
            <span className="relative flex h-2 w-2">
              <span className="absolute inset-0 animate-ping rounded-full bg-success/70" />
              <span className="relative inline-block h-2 w-2 rounded-full bg-success" />
            </span>
            <span className="t-micro text-foreground/60">
              LIVE · updated {secsAgo}s ago
            </span>
          </span>
        </div>

        {/* Fetch error banner — shown when the server-side initial
            load threw. The dashboard still renders (client polling may
            recover), but we surface the reason so the auth'd user
            knows what to fix. */}
        {fetchError && !stats && (
          <div className="mt-6 rounded-2xl border border-destructive/40 bg-destructive/5 p-4 text-sm">
            <p className="font-medium text-destructive">
              Auth passed, but the stats query failed.
            </p>
            <p className="mt-1 text-foreground/70 text-xs font-mono break-all">
              {fetchError}
            </p>
            <p className="mt-2 text-foreground/60 text-xs">
              Usually this means the Supabase env vars are missing on
              Netlify, the waitlist table doesn&apos;t exist in prod, or
              RLS is blocking the service-role read.
            </p>
          </div>
        )}

        {/* Hero count */}
        <section className="mt-10 rounded-3xl border border-gold-primary/20 bg-depth-card p-8 text-center md:p-12">
          <p className="t-micro text-gold-deep">INSIDER · WAITLIST SIGNUPS</p>
          <p
            className="mt-3 font-serif font-bold tabular-nums text-gold-cream"
            style={{ fontSize: "clamp(56px, 14vw, 104px)", lineHeight: 1 }}
          >
            {displayed.toLocaleString()}
          </p>
          <p className="mt-3 t-body text-foreground/60">
            people have reserved their spot
          </p>
        </section>

        {/* Stat row */}
        <section className="mt-4 grid grid-cols-3 gap-3">
          {[
            { label: "Last 24h", value: stats ? `+${stats.last24h}` : "—" },
            { label: "Last 7 days", value: stats ? `+${stats.last7d}` : "—" },
            {
              label: "Latest",
              value: relative(stats?.latestAt ?? null),
            },
          ].map((s) => (
            <div
              key={s.label}
              className="rounded-2xl border border-depth-border bg-depth-card p-4 text-center"
            >
              <p className="font-serif text-xl font-bold tabular-nums text-gold-cream md:text-2xl">
                {s.value}
              </p>
              <p className="mt-1 t-micro text-foreground/45">{s.label}</p>
            </div>
          ))}
        </section>

        {/* Country breakdown */}
        <section className="mt-4 rounded-2xl border border-depth-border bg-depth-card p-6">
          <p className="t-micro text-gold-deep">BY COUNTRY</p>
          <ul className="mt-4 space-y-3">
            {stats && stats.countries.length > 0 ? (
              stats.countries.map((c) => (
                <li key={c.country} className="flex items-center gap-3">
                  <span className="w-20 shrink-0 t-caption text-foreground/80">
                    {c.country}
                  </span>
                  <span className="relative h-2 flex-1 overflow-hidden rounded-full bg-depth-elevated">
                    <span
                      className="absolute inset-y-0 left-0 rounded-full bg-gold-primary"
                      style={{ width: `${Math.max(6, (c.count / maxCountry) * 100)}%` }}
                    />
                  </span>
                  <span className="w-8 shrink-0 text-right t-caption tabular-nums text-foreground/70">
                    {c.count}
                  </span>
                </li>
              ))
            ) : (
              <li className="t-caption text-foreground/40">No signups yet.</li>
            )}
          </ul>
        </section>

        {/* Live signups table */}
        <section className="mt-4 rounded-2xl border border-depth-border bg-depth-card">
          <div className="flex items-center justify-between border-b border-depth-border px-6 py-4">
            <p className="t-micro text-gold-deep">EVERY SIGNUP · NEWEST FIRST</p>
            <span className="t-micro tabular-nums text-foreground/40">
              {stats?.signups.length ?? 0} total
            </span>
          </div>

          {/* Header row */}
          <div className="hidden grid-cols-[44px_1fr_120px_120px] gap-4 border-b border-depth-border/70 px-6 py-3 t-micro text-foreground/40 sm:grid">
            <span>#</span>
            <span>Email</span>
            <span>Country</span>
            <span className="text-right">Joined</span>
          </div>

          {stats && stats.signups.length > 0 ? (
            <ul className="divide-y divide-depth-border/60">
              {stats.signups.map((s, i) => (
                <li
                  key={`${s.position}-${s.email}`}
                  className={`grid grid-cols-[44px_1fr] gap-x-3 gap-y-1 px-6 py-3 transition-colors sm:grid-cols-[44px_1fr_120px_120px] sm:gap-4 sm:py-3.5 ${
                    i === 0 ? "bg-gold-primary/[0.04]" : "hover:bg-depth-elevated/40"
                  }`}
                >
                  {/* Position */}
                  <span className="font-serif text-sm font-bold tabular-nums text-gold-cream sm:text-base">
                    {s.position}
                  </span>
                  {/* Email + meta (referred-by + mobile country/time) */}
                  <div className="min-w-0">
                    <p className="t-body break-all font-mono text-[13px] text-foreground/90 sm:truncate sm:font-sans sm:text-sm">
                      {s.email}
                    </p>
                    {s.referredBy ? (
                      <p className="mt-0.5 inline-flex items-center gap-1 t-micro text-gold-deep">
                        <span>↗</span>
                        <span>via {s.referredBy}</span>
                      </p>
                    ) : null}
                    {/* mobile-only inline country + time */}
                    <p className="mt-1 t-micro text-foreground/45 sm:hidden">
                      {s.country} · {relative(s.createdAt)}
                    </p>
                  </div>
                  {/* Country (desktop) */}
                  <span className="hidden items-center sm:flex">
                    <span className="rounded-md border border-depth-border bg-depth-elevated px-2 py-0.5 t-micro text-foreground/75">
                      {s.country}
                    </span>
                  </span>
                  {/* Time (desktop) */}
                  <span className="hidden items-center justify-end t-caption text-foreground/55 sm:flex">
                    {relative(s.createdAt)}
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="px-6 py-8 text-center t-caption text-foreground/40">
              No signups yet.
            </p>
          )}
        </section>

        <p className="mt-8 text-center t-micro text-foreground/30">
          Private · for Mizan eyes only · auto-refreshes every 12s
        </p>
      </Container>
    </main>
  );
}
