/**
 * Static dashboard mockup — a hand-composed React tree that mirrors the
 * actual Tauri desktop's PR-DENSITY-1 panel layout (72px tiles, 8pt
 * gap, gold-tinted heatmap). Ships as JSX rather than a screenshot
 * because (a) we get pixel-perfect at every viewport, (b) no real
 * screenshot exists pre-launch, and (c) the composition stays in
 * sync with the desktop's polish work without an export step.
 *
 * Numbers use the §23 reference fixture portfolio:
 *   net worth $1,712,394
 *   Sukuks $613K (largest tile), Equities $367K, Bank & Cash $228K,
 *   Real Estate excluded (primary residence), etc.
 */
import { ArrowUpRight, Coins, Wallet } from "lucide-react";

export function DashboardMockup() {
  return (
    <div
      aria-label="Mizan dashboard preview"
      className="relative w-full overflow-hidden rounded-xl border border-depth-border bg-[hsl(0_0%_5%)] shadow-[0_40px_120px_-20px_rgba(212,165,116,0.18)] tilt-on-hover"
    >
      {/* Mock browser chrome */}
      <div className="flex items-center gap-2 border-b border-depth-border bg-depth-elevated px-4 py-2.5">
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
        <span aria-hidden="true" className="h-3 w-3 rounded-full bg-depth-border" />
        <span className="ml-3 t-micro text-foreground/40">
          mizan · dashboard
        </span>
      </div>

      <div className="p-5 md:p-6 space-y-4">
        {/* Net Worth strip */}
        <div className="rounded-xl border border-depth-border bg-depth-card p-5">
          <p className="t-micro text-gold-deep">NET WORTH · 30 DAYS</p>
          <div className="mt-2 flex items-baseline gap-3">
            <span className="font-serif text-3xl md:text-4xl font-bold tabular-nums text-gold-cream">
              $1,712,394
            </span>
            <span className="t-caption text-success inline-flex items-center gap-1">
              <ArrowUpRight className="h-3.5 w-3.5" /> +$4,820 · +0.28%
            </span>
          </div>
          <div className="mt-4 flex gap-1.5">
            {["24h", "7d", "30d", "YTD", "All"].map((label, i) => (
              <span
                key={label}
                className={`h-6 rounded-full border border-depth-border px-2.5 t-micro leading-6 ${
                  i === 2
                    ? "bg-gold-primary/15 text-gold-cream border-gold-primary/30"
                    : "text-foreground/50"
                }`}
              >
                {label}
              </span>
            ))}
          </div>
          {/* Sparkline */}
          <svg
            className="mt-3 w-full h-14"
            viewBox="0 0 400 60"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <defs>
              <linearGradient id="spark" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stopColor="hsl(31 49% 64%)" stopOpacity="0.4" />
                <stop offset="100%" stopColor="hsl(31 49% 64%)" stopOpacity="0" />
              </linearGradient>
            </defs>
            <path
              d="M0,42 L40,38 L80,45 L120,32 L160,28 L200,30 L240,22 L280,25 L320,18 L360,12 L400,8"
              fill="none"
              stroke="hsl(31 49% 64%)"
              strokeWidth="1.8"
            />
            <path
              d="M0,42 L40,38 L80,45 L120,32 L160,28 L200,30 L240,22 L280,25 L320,18 L360,12 L400,8 L400,60 L0,60 Z"
              fill="url(#spark)"
            />
          </svg>
        </div>

        {/* Heatmap */}
        <div className="rounded-xl border border-depth-border bg-depth-card p-3 md:p-4">
          <div className="grid grid-cols-6 grid-rows-3 gap-1 h-32 md:h-40">
            {/* Largest tile — Sukuks INDOSUK */}
            <div className="col-span-3 row-span-2 rounded-md flex flex-col justify-between p-2 bg-[hsl(72_46%_18%)] border border-[hsl(72_46%_30%)]">
              <span className="t-micro text-success/90">INDOSUK</span>
              <span className="t-caption text-foreground/90 font-semibold">+2.4%</span>
            </div>
            <div className="col-span-2 row-span-1 rounded-md flex flex-col justify-between p-2 bg-[hsl(72_46%_15%)] border border-[hsl(72_46%_28%)]">
              <span className="t-micro text-success/85">EMAAR</span>
              <span className="t-caption text-foreground/85 font-semibold">+1.1%</span>
            </div>
            <div className="col-span-1 row-span-1 rounded-md p-1.5 bg-[hsl(72_46%_12%)] border border-[hsl(72_46%_25%)]">
              <span className="t-micro text-success/80">SPUS</span>
            </div>
            <div className="col-span-2 row-span-1 rounded-md flex flex-col justify-between p-2 bg-[hsl(72_46%_11%)] border border-[hsl(72_46%_22%)]">
              <span className="t-micro text-success/75">AAPL</span>
              <span className="t-caption text-foreground/70">+0.4%</span>
            </div>
            <div className="col-span-1 row-span-1 rounded-md p-1.5 bg-[hsl(5_61%_18%)] border border-[hsl(5_61%_28%)]">
              <span className="t-micro text-destructive/80">BTC</span>
            </div>
            <div className="col-span-2 row-span-1 rounded-md p-2 bg-[hsl(0_0%_12%)] border border-depth-border">
              <span className="t-micro text-foreground/40">DBSPH</span>
            </div>
            <div className="col-span-2 row-span-1 rounded-md p-2 bg-[hsl(72_46%_10%)] border border-[hsl(72_46%_20%)]">
              <span className="t-micro text-success/70">WAHED</span>
            </div>
            <div className="col-span-1 row-span-1 rounded-md p-1.5 bg-[hsl(5_61%_14%)] border border-[hsl(5_61%_24%)]">
              <span className="t-micro text-destructive/70">ETH</span>
            </div>
          </div>
        </div>

        {/* Asset class tiles (3 visible) */}
        <div className="grid grid-cols-3 gap-2">
          {[
            { icon: Wallet, label: "Equities", value: "$367K", delta: "+0.42%" },
            { icon: Coins, label: "Bonds & Sukuks", value: "$613K", delta: "+0.18%" },
            { icon: Wallet, label: "Bank & Cash", value: "$228K", delta: "—" },
          ].map(({ icon: Icon, label, value, delta }) => (
            <div
              key={label}
              className="h-[72px] rounded-[10px] border border-depth-border bg-depth-card px-3 py-2.5 flex flex-col justify-between"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-1.5">
                  <Icon className="h-3.5 w-3.5 text-foreground/40" />
                  <span className="t-micro text-foreground/85">{label}</span>
                </div>
                <span className="t-body-bold tabular-nums text-gold-cream">
                  {value}
                </span>
              </div>
              <div className="flex justify-between t-micro text-foreground/50">
                <span>2 holdings</span>
                <span className={delta === "—" ? "" : "text-success"}>{delta}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      <style>{`
        .tilt-on-hover {
          transform: perspective(1200px) rotateY(0deg) rotateX(0deg);
          transition: transform 350ms cubic-bezier(0.25, 0.46, 0.45, 0.94);
          will-change: transform;
        }
        @media (hover: hover) {
          .tilt-on-hover:hover {
            transform: perspective(1200px) rotateY(-2deg) rotateX(1deg);
          }
        }
        @media (prefers-reduced-motion: reduce) {
          .tilt-on-hover { transition: none; }
          .tilt-on-hover:hover { transform: none; }
        }
      `}</style>
    </div>
  );
}
