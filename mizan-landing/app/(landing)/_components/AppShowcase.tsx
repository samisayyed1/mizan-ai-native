"use client";

/**
 * AppShowcase — a large, premium, auto-rotating product demo. Cycles
 * through six hand-composed screens (Overview → Goals → Mizan AI →
 * Alerts → News → Accounts) inside a browser frame with a tab bar +
 * dwell-progress bar, so it reads as a real product walkthrough.
 *
 * Every screen is JSX (not a screenshot) so it stays pixel-perfect at
 * any viewport. Numbers use the §23 reference portfolio ($1.71M net
 * worth; the six asset classes sum exactly to $1,712,394).
 *
 * Motion: auto-advances every DWELL ms, pauses on hover, respects
 * `prefers-reduced-motion` (no auto-advance, instant tab switches).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  ArrowUpRight,
  Bell,
  Building2,
  Check,
  Coins,
  LayoutDashboard,
  Landmark,
  Newspaper,
  Sparkles,
  Target,
  TrendingDown,
  TrendingUp,
  Wallet,
} from "lucide-react";

const DWELL = 4600;

const TABS = [
  { id: "overview", label: "Overview", icon: LayoutDashboard },
  { id: "goals", label: "Goals", icon: Target },
  { id: "ai", label: "Mizan AI", icon: Sparkles },
  { id: "alerts", label: "Alerts", icon: Bell },
  { id: "news", label: "News", icon: Newspaper },
  { id: "accounts", label: "Accounts", icon: Wallet },
] as const;

export function AppShowcase() {
  const reduce = useReducedMotion();
  const [active, setActive] = useState(0);
  const [hovered, setHovered] = useState(false);
  const [onScreen, setOnScreen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  // Pause when not on screen so the carousel never burns CPU/battery
  // while scrolled away — the single biggest mobile-perf win here.
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const io = new IntersectionObserver(
      ([entry]) => setOnScreen(entry.isIntersecting),
      { threshold: 0.2 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  const paused = hovered || !onScreen;

  const go = useCallback(
    (i: number) => setActive(((i % TABS.length) + TABS.length) % TABS.length),
    [],
  );

  useEffect(() => {
    if (reduce || paused) return;
    timer.current = setTimeout(() => go(active + 1), DWELL);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [active, paused, reduce, go]);

  return (
    <div
      ref={rootRef}
      className="app-showcase relative w-full overflow-hidden rounded-2xl border border-depth-border bg-[hsl(0_0%_5%)] shadow-[0_50px_140px_-30px_rgba(212,165,116,0.28)]"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* Ambient gold glow inside the panel — premium depth cue. */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -right-24 -top-24 h-64 w-64 rounded-full opacity-40 blur-3xl"
        style={{
          background:
            "radial-gradient(circle at center, hsl(31 49% 64% / 0.5) 0%, transparent 65%)",
        }}
      />

      {/* Browser chrome */}
      <div className="relative flex items-center gap-2 border-b border-depth-border bg-depth-elevated px-4 py-3">
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
      </div>

      {/* Tab bar */}
      <div className="relative flex items-center gap-1 overflow-x-auto border-b border-depth-border bg-depth-container/60 px-2.5 py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {TABS.map((t, i) => {
          const isActive = i === active;
          return (
            <button
              key={t.id}
              type="button"
              onClick={() => go(i)}
              aria-pressed={isActive}
              className={`relative flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 t-micro transition-colors ${
                isActive
                  ? "bg-gold-primary/12 text-gold-cream"
                  : "text-foreground/45 hover:text-foreground/70"
              }`}
            >
              <t.icon className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{t.label}</span>
              {isActive && !reduce ? (
                <motion.span
                  layoutId="tab-underline"
                  className="absolute inset-x-1.5 -bottom-[9px] h-[2px] rounded-full bg-gold-primary"
                />
              ) : null}
            </button>
          );
        })}
      </div>

      {/* Screen */}
      <div className="relative min-h-[560px] p-5 md:min-h-[620px] md:p-6">
        <AnimatePresence mode="wait">
          <motion.div
            key={TABS[active].id}
            initial={reduce ? false : { opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={reduce ? undefined : { opacity: 0, y: -12 }}
            transition={{ duration: 0.32, ease: [0.25, 0.46, 0.45, 0.94] }}
          >
            <Screen id={TABS[active].id} />
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}

function Screen({ id }: { id: (typeof TABS)[number]["id"] }) {
  switch (id) {
    case "overview":
      return <OverviewScreen />;
    case "goals":
      return <GoalsScreen />;
    case "ai":
      return <AIScreen />;
    case "alerts":
      return <AlertsScreen />;
    case "news":
      return <NewsScreen />;
    case "accounts":
      return <AccountsScreen />;
  }
}

/* ----------------------------- Overview ----------------------------- */

const ALLOC = [
  { label: "Sukuks", pct: 36, color: "hsl(40 67% 78%)" },
  { label: "Equities", pct: 21, color: "hsl(31 49% 64%)" },
  { label: "Property", pct: 14, color: "hsl(31 42% 52%)" },
  { label: "Cash", pct: 13, color: "hsl(31 32% 41%)" },
  { label: "Crypto", pct: 8, color: "hsl(31 24% 33%)" },
  { label: "Gold", pct: 8, color: "hsl(45 62% 58%)" },
];
const R = 46;
const C = 2 * Math.PI * R;

// Curated holdings for the mobile heatmap — a clean 2-column grid of
// six larger tiles that breathe, instead of the desktop treemap's nine
// (which gets cramped on a narrow phone). Same gradient language.
const MOBILE_HOLDINGS = [
  { name: "INDOSUK", sub: "Sukuk · $487K", delta: "+2.4%", grad: "135deg, hsl(120 58% 24%) 0%, hsl(140 52% 16%) 100%", border: "hsl(120 58% 38%)" },
  { name: "EMAAR", sub: "Sukuk · $312K", delta: "+1.1%", grad: "135deg, hsl(118 50% 20%) 0%, hsl(135 44% 13%) 100%", border: "hsl(118 50% 32%)" },
  { name: "AAPL", sub: "Equity · $168K", delta: "+0.42%", grad: "135deg, hsl(110 38% 17%) 0%, hsl(125 32% 11%) 100%", border: "hsl(110 38% 28%)" },
  { name: "XAU/USD", sub: "Gold · $137K", delta: "+0.6%", grad: "135deg, hsl(40 67% 38%) 0%, hsl(35 55% 24%) 100%", border: "hsl(40 67% 52%)" },
  { name: "BTC", sub: "Crypto · $92K", delta: "-1.8%", grad: "135deg, hsl(5 65% 26%) 0%, hsl(0 55% 16%) 100%", border: "hsl(5 65% 38%)" },
  { name: "ETH", sub: "Crypto · $45K", delta: "-0.9%", grad: "135deg, hsl(5 55% 22%) 0%, hsl(0 45% 13%) 100%", border: "hsl(5 55% 32%)" },
] as const;

function OverviewScreen() {
  let cum = 0;
  const segs = ALLOC.map((s) => {
    const len = (s.pct / 100) * C;
    const off = -cum;
    cum += len;
    return { ...s, len, off };
  });
  return (
    <div className="space-y-4">
      {/* Net worth + sparkline */}
      <div className="rounded-xl border border-depth-border bg-depth-card p-4 sm:p-5">
        <div className="flex items-center justify-between">
          <p className="t-micro text-gold-deep">NET WORTH · 30 DAYS</p>
          {/* Range pills are desktop-only — on mobile they'd crowd the
              label, and "30 DAYS" already states the range. */}
          <div className="hidden gap-1 sm:flex">
            {["24h", "7d", "30d", "YTD", "All"].map((l, i) => (
              <span
                key={l}
                className={`rounded-full px-2 py-0.5 t-micro ${
                  i === 2
                    ? "border border-gold-primary/30 bg-gold-primary/15 text-gold-cream"
                    : "text-foreground/40"
                }`}
              >
                {l}
              </span>
            ))}
          </div>
        </div>
        {/* Number + delta stack on mobile so the big figure never
            collides with the change pill. */}
        <div className="mt-2 flex flex-col gap-0.5 sm:flex-row sm:items-baseline sm:gap-3">
          <span className="font-serif text-3xl font-bold tabular-nums leading-none text-gold-cream sm:text-4xl">
            $1,712,394
          </span>
          <span className="t-caption inline-flex items-center gap-1 text-success">
            <ArrowUpRight className="h-3.5 w-3.5" /> +$4,820 · +0.28%
          </span>
        </div>
        <svg className="mt-3 h-16 w-full" viewBox="0 0 400 64" preserveAspectRatio="none" aria-hidden="true">
          <defs>
            <linearGradient id="sc-spark" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor="hsl(31 49% 64%)" stopOpacity="0.4" />
              <stop offset="100%" stopColor="hsl(31 49% 64%)" stopOpacity="0" />
            </linearGradient>
          </defs>
          <path d="M0,46 L40,40 L80,48 L120,34 L160,28 L200,32 L240,22 L280,26 L320,16 L360,11 L400,6" fill="none" stroke="hsl(31 49% 64%)" strokeWidth="2" />
          <path d="M0,46 L40,40 L80,48 L120,34 L160,28 L200,32 L240,22 L280,26 L320,16 L360,11 L400,6 L400,64 L0,64 Z" fill="url(#sc-spark)" />
        </svg>
      </div>

      {/* Allocation donut + legend. The donut is a fixed, generous size
          on mobile (so the centre label always clears the ring) and
          fills its 130px column on desktop. */}
      <div className="grid gap-5 rounded-xl border border-depth-border bg-depth-card p-5 sm:grid-cols-[130px_1fr] sm:gap-4">
        <div className="relative mx-auto aspect-square w-[168px] sm:w-full">
          <svg viewBox="0 0 128 128" className="block h-full w-full -rotate-90" aria-hidden="true">
            <circle cx="64" cy="64" r={R} fill="none" stroke="hsl(0 0% 12%)" strokeWidth="13" />
            {segs.map((s) => (
              <circle key={s.label} cx="64" cy="64" r={R} fill="none" stroke={s.color} strokeWidth="13" strokeDasharray={`${s.len} ${C}`} strokeDashoffset={s.off} />
            ))}
          </svg>
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center leading-none">
            <span className="font-serif text-xl font-bold text-gold-cream sm:text-lg">$1.71M</span>
            <span className="mt-1 t-micro text-foreground/45">6 classes</span>
          </div>
        </div>
        <ul className="grid grid-cols-2 gap-x-4 gap-y-2 self-center sm:grid-cols-3">
          {ALLOC.map((s) => (
            <li key={s.label} className="flex items-center gap-2">
              <span aria-hidden="true" className="h-2.5 w-2.5 shrink-0 rounded-sm" style={{ backgroundColor: s.color }} />
              <span className="t-caption truncate text-foreground/80">{s.label}</span>
              <span className="t-caption tabular-nums text-foreground/55">{s.pct}%</span>
            </li>
          ))}
        </ul>
      </div>

      {/* Finviz-style heatmap */}
      <div className="rounded-xl border border-depth-border bg-depth-card p-3.5">
        <div className="mb-2.5 flex items-baseline justify-between">
          <p className="t-micro text-gold-deep">HOLDINGS · TODAY</p>
          <span className="t-micro text-success/75">+0.42% net</span>
        </div>
        {/* Desktop: rich Finviz-style treemap (wide enough to breathe). */}
        <div className="hidden h-52 grid-cols-12 grid-rows-4 gap-1 sm:grid">
          <Tile cls="col-span-5 row-span-3" grad="135deg, hsl(120 58% 24%) 0%, hsl(140 52% 16%) 100%" border="hsl(120 58% 38%)" name="INDOSUK" sub="Sukuk · $487K" big="+2.4%" extra="$11,728" />
          <Tile cls="col-span-4 row-span-2" grad="135deg, hsl(118 50% 20%) 0%, hsl(135 44% 13%) 100%" border="hsl(118 50% 32%)" name="EMAAR" sub="Sukuk · $312K" big="+1.1%" />
          <Tile cls="col-span-3 row-span-2" grad="135deg, hsl(110 38% 17%) 0%, hsl(125 32% 11%) 100%" border="hsl(110 38% 28%)" name="AAPL" sub="Equity · $168K" big="+0.42%" />
          <Tile cls="col-span-2" grad="135deg, hsl(105 34% 16%) 0%, hsl(120 28% 10%) 100%" border="hsl(105 34% 26%)" name="SPUS" small="+0.28%" />
          <Tile cls="col-span-2" grad="135deg, hsl(108 28% 14%) 0%, hsl(120 22% 9%) 100%" border="hsl(108 28% 24%)" name="WAHED" small="+0.12%" />
          <Tile cls="col-span-3" grad="135deg, hsl(5 65% 26%) 0%, hsl(0 55% 16%) 100%" border="hsl(5 65% 38%)" name="BTC" small="-1.8%" />
          <Tile cls="col-span-2" grad="135deg, hsl(5 55% 22%) 0%, hsl(0 45% 13%) 100%" border="hsl(5 55% 32%)" name="ETH" small="-0.9%" />
          <Tile cls="col-span-2" grad="none" border="hsl(0 0% 22%)" flat name="DBSPH" small="+0.02%" />
          <Tile cls="col-span-3" grad="135deg, hsl(40 67% 38%) 0%, hsl(35 55% 24%) 100%" border="hsl(40 67% 52%)" name="XAU/USD" small="+0.6%" />
        </div>

        {/* Mobile: clean 2-column grid of six larger tiles. */}
        <div className="grid grid-cols-2 gap-2 sm:hidden">
          {MOBILE_HOLDINGS.map((h) => (
            <article
              key={h.name}
              className="flex min-h-[76px] flex-col justify-between rounded-lg p-3"
              style={{
                background: `linear-gradient(${h.grad})`,
                border: `1px solid ${h.border}`,
              }}
            >
              <div>
                <p className="t-body-bold text-sm text-white/95">{h.name}</p>
                <p className="t-micro text-white/55">{h.sub}</p>
              </div>
              <span className="font-serif text-base font-bold tabular-nums text-white">
                {h.delta}
              </span>
            </article>
          ))}
        </div>
      </div>
    </div>
  );
}

function Tile({
  cls,
  grad,
  border,
  name,
  sub,
  big,
  extra,
  small,
  flat,
}: {
  cls: string;
  grad: string;
  border: string;
  name: string;
  sub?: string;
  big?: string;
  extra?: string;
  small?: string;
  flat?: boolean;
}) {
  return (
    <article
      className={`flex flex-col justify-between overflow-hidden rounded-md p-2 ${cls}`}
      style={{
        background: flat ? "hsl(0 0% 13%)" : `linear-gradient(${grad})`,
        border: `1px solid ${border}`,
      }}
    >
      <div>
        <p className={`${big ? "t-body-bold" : "t-micro font-semibold"} text-white/95`}>{name}</p>
        {sub ? <p className="t-micro text-white/55">{sub}</p> : null}
      </div>
      {big ? (
        <div className="flex items-baseline justify-between gap-1">
          <span className="font-serif text-lg font-bold tabular-nums text-white sm:text-xl">{big}</span>
          {extra ? <span className="hidden t-micro text-white/70 sm:inline">{extra}</span> : null}
        </div>
      ) : small ? (
        <span className={`t-micro tabular-nums ${flat ? "text-foreground/55" : "text-white/80"}`}>{small}</span>
      ) : null}
    </article>
  );
}

/* ------------------------------ Goals ------------------------------- */

const GOALS = [
  { name: "Hajj 2027", saved: 18400, target: 25000, icon: Landmark },
  { name: "Lisbon apartment", saved: 142000, target: 235000, icon: Building2 },
  { name: "Emergency buffer", saved: 72000, target: 72000, icon: Wallet },
];

function GoalsScreen() {
  const totalSaved = GOALS.reduce((s, g) => s + g.saved, 0);
  const totalTarget = GOALS.reduce((s, g) => s + g.target, 0);
  const totalPct = Math.round((totalSaved / totalTarget) * 100);
  return (
    <div className="space-y-4">
      {/* Summary */}
      <div className="rounded-xl border border-gold-primary/20 bg-depth-card p-5">
        <div className="flex items-center justify-between">
          <p className="t-micro text-gold-deep">TOTAL SAVED · 3 GOALS</p>
          <span className="t-caption text-gold-cream tabular-nums">{totalPct}%</span>
        </div>
        <p className="mt-1.5 font-serif text-2xl font-bold tabular-nums text-gold-cream">
          ${totalSaved.toLocaleString()}{" "}
          <span className="t-caption font-sans font-normal text-foreground/45">
            of ${totalTarget.toLocaleString()}
          </span>
        </p>
        <div className="mt-3 h-2 w-full overflow-hidden rounded-full bg-depth-elevated">
          <div className="h-full rounded-full bg-gold-primary" style={{ width: `${totalPct}%` }} />
        </div>
      </div>

      {GOALS.map((g) => {
        const pct = Math.min(100, Math.round((g.saved / g.target) * 100));
        const done = pct >= 100;
        return (
          <div key={g.name} className="rounded-xl border border-depth-border bg-depth-card p-4">
            <div className="flex items-center justify-between">
              <span className="flex items-center gap-2.5">
                <span className="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-depth-elevated text-gold-primary">
                  <g.icon className="h-4 w-4" />
                </span>
                <span className="t-body-bold text-foreground/90 text-sm">{g.name}</span>
              </span>
              <span className={`t-caption tabular-nums ${done ? "text-success" : "text-foreground/70"}`}>
                {done ? (
                  <span className="inline-flex items-center gap-1"><Check className="h-3.5 w-3.5" /> Funded</span>
                ) : (
                  `${pct}%`
                )}
              </span>
            </div>
            <div className="mt-3 h-2 w-full overflow-hidden rounded-full bg-depth-elevated">
              <div className={`h-full rounded-full ${done ? "bg-success" : "bg-gold-primary"}`} style={{ width: `${pct}%` }} />
            </div>
            <div className="mt-2 flex justify-between t-micro text-foreground/50 tabular-nums">
              <span>${g.saved.toLocaleString()}</span>
              <span>${g.target.toLocaleString()}</span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ----------------------------- Mizan AI ----------------------------- */

function AIScreen() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 pb-4">
        <span className="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-gold-primary/12 text-gold-primary">
          <Sparkles className="h-4 w-4" />
        </span>
        <span className="t-body-bold text-foreground/90 text-sm">Mizan AI</span>
        <span className="ml-auto inline-flex items-center gap-1.5 t-micro text-success">
          <span className="h-1.5 w-1.5 rounded-full bg-success" /> online
        </span>
      </div>

      <div className="space-y-3.5">
        <div className="flex justify-end">
          <p className="max-w-[80%] rounded-2xl rounded-br-md border border-gold-primary/20 bg-gold-primary/[0.14] px-4 py-2.5 t-body text-sm text-foreground">
            Am I overweight on emerging markets after this quarter?
          </p>
        </div>
        <div className="flex">
          <p className="max-w-[88%] rounded-2xl rounded-bl-md border border-depth-border bg-depth-elevated px-4 py-3 t-body text-sm leading-relaxed text-foreground/85">
            Slightly. EM is <span className="text-gold-cream">23%</span> of your equity sleeve versus your <span className="text-gold-cream">18%</span> target — the drift came from INDOSUK&apos;s +2.4% run. Here&apos;s a rebalance I&apos;d suggest to bring it back, using your $228K cash.
          </p>
        </div>

        {/* Suggestion preview card — Mizan proposes; the user decides. */}
        <div className="rounded-xl border border-depth-border bg-depth-card p-4">
          <p className="t-micro text-gold-deep">SUGGESTED REBALANCE</p>
          <div className="mt-3 space-y-2.5">
            {[
              { name: "Emerging markets", from: "23%", to: "18%", down: true },
              { name: "Developed equity", from: "61%", to: "64%", down: false },
              { name: "Cash deployed", from: "$228K", to: "$176K", down: true },
            ].map((r) => (
              <div key={r.name} className="flex items-center justify-between t-caption">
                <span className="text-foreground/70">{r.name}</span>
                <span className="flex items-center gap-2 tabular-nums text-foreground/50">
                  {r.from}
                  <span className="text-gold-primary">→</span>
                  <span className="text-gold-cream">{r.to}</span>
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex gap-2">
          <span className="rounded-lg border border-gold-primary/30 bg-gold-primary/10 px-3.5 py-2 t-caption text-gold-cream">
            Draft the trades
          </span>
          <span className="rounded-lg border border-depth-border bg-depth-card px-3.5 py-2 t-caption text-foreground/60">
            Adjust
          </span>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------ Alerts ------------------------------ */

const ALERTS = [
  { tone: "warn", icon: TrendingUp, title: "USDSGD moved +1.2% today", sub: "Net worth +$3,670 · USD exposure $367K" },
  { tone: "info", icon: Coins, title: "EMAAR Sukuk matures in 47 days", sub: "$312K face value · 3 reinvestment matches found" },
  { tone: "gold", icon: Landmark, title: "Zakat due in 12 days", sub: "Estimated $2,847 · computed against today's Nisab" },
  { tone: "down", icon: TrendingDown, title: "BTC down 1.8% overnight", sub: "Crypto sleeve −$1,640 · still within target band" },
  { tone: "info", icon: Building2, title: "Lisbon goal hit 60%", sub: "On pace to fully fund by Q3 2027" },
];

function AlertsScreen() {
  const toneClass: Record<string, string> = {
    warn: "text-success",
    info: "text-gold-primary",
    gold: "text-gold-cream",
    down: "text-destructive",
  };
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between pb-1">
        <p className="t-body-bold text-foreground/95">Notifications</p>
        <span className="t-micro text-gold-primary">5 new</span>
      </div>
      {ALERTS.map((a) => (
        <div key={a.title} className="flex items-start gap-3 rounded-xl border border-depth-border bg-depth-card p-3.5">
          <span className={`mt-0.5 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-depth-elevated ${toneClass[a.tone]}`}>
            <a.icon className="h-4 w-4" />
          </span>
          <div className="min-w-0">
            <p className="t-body-bold text-foreground/90 text-sm">{a.title}</p>
            <p className="mt-0.5 t-caption text-foreground/55 leading-relaxed">{a.sub}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

/* ------------------------------- News ------------------------------- */

const TICKER = [
  { t: "INDOSUK", d: "+2.4%", up: true },
  { t: "EMAAR", d: "+1.1%", up: true },
  { t: "XAU", d: "+0.6%", up: true },
  { t: "AAPL", d: "+0.4%", up: true },
  { t: "BTC", d: "−1.8%", up: false },
  { t: "ETH", d: "−0.9%", up: false },
];

const NEWS = [
  { tag: "EMAAR", delta: "+1.1%", up: true, head: "Emaar posts record Q2; sukuk yields tighten across the curve" },
  { tag: "XAU", delta: "+0.6%", up: true, head: "Fed holds rates steady — gold extends its weekly gain" },
  { tag: "AAPL", delta: "+0.4%", up: true, head: "Apple unveils on-device AI tier; analysts lift price targets" },
  { tag: "BTC", delta: "−1.8%", up: false, head: "Crypto pulls back as ETF inflows cool into month-end" },
];

function NewsScreen() {
  return (
    <div className="space-y-3">
      {/* Ticker strip */}
      <div className="flex flex-wrap gap-2 rounded-xl border border-depth-border bg-depth-card p-3">
        {TICKER.map((x) => (
          <span key={x.t} className="inline-flex items-center gap-1.5 rounded-md bg-depth-elevated px-2.5 py-1 t-micro">
            <span className="font-semibold text-foreground/75">{x.t}</span>
            <span className={`tabular-nums ${x.up ? "text-success" : "text-destructive"}`}>{x.d}</span>
          </span>
        ))}
      </div>

      <div className="flex items-center justify-between pb-0.5">
        <p className="t-body-bold text-foreground/95">News · your holdings</p>
        <span className="t-micro text-foreground/45">live feed</span>
      </div>
      {NEWS.map((n) => (
        <div key={n.tag} className="flex items-center gap-3 rounded-xl border border-depth-border bg-depth-card p-4">
          <span className="inline-flex shrink-0 items-center rounded-md border border-depth-border bg-depth-elevated px-2 py-1 t-micro font-semibold text-foreground/80">
            {n.tag}
          </span>
          <p className="min-w-0 flex-1 t-caption leading-snug text-foreground/85">{n.head}</p>
          <span className={`shrink-0 t-caption tabular-nums ${n.up ? "text-success" : "text-destructive"}`}>
            {n.delta}
          </span>
        </div>
      ))}
    </div>
  );
}

/* ----------------------------- Accounts ----------------------------- */

const CLASSES = [
  { name: "Bonds & Sukuks", value: "$617K", pct: "36%", delta: "+0.18%", up: true, icon: Coins },
  { name: "Equities", value: "$360K", pct: "21%", delta: "+0.42%", up: true, icon: TrendingUp },
  { name: "Property", value: "$240K", pct: "14%", delta: "—", up: true, icon: Building2 },
  { name: "Bank & Cash", value: "$223K", pct: "13%", delta: "—", up: true, icon: Landmark },
  { name: "Crypto", value: "$137K", pct: "8%", delta: "−1.4%", up: false, icon: Wallet },
  { name: "Gold", value: "$137K", pct: "8%", delta: "+0.6%", up: true, icon: Coins },
];

function AccountsScreen() {
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between pb-1">
        <p className="t-body-bold text-foreground/95">Asset classes</p>
        <span className="t-micro text-foreground/45 tabular-nums">$1,712,394 total</span>
      </div>
      {CLASSES.map((c) => (
        <div key={c.name} className="flex items-center gap-3 rounded-xl border border-depth-border bg-depth-card px-4 py-3.5">
          <span className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-depth-elevated text-foreground/50">
            <c.icon className="h-4 w-4" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="t-body-bold text-foreground/90 text-sm">{c.name}</p>
            <p className="t-micro text-foreground/45">{c.pct} of portfolio</p>
          </div>
          <div className="text-right">
            <p className="t-body-bold tabular-nums text-gold-cream">{c.value}</p>
            <p className={`t-micro tabular-nums ${c.delta === "—" ? "text-foreground/40" : c.up ? "text-success" : "text-destructive"}`}>
              {c.delta}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}
