/**
 * Shared panel primitives — the world-class look-and-feel for every
 * asset-class panel page (Equities, Sukuks, Brokerage, …).
 *
 * Replaces the prior "categorical-palette Donut + Bar in a card with a
 * generic title" pattern that rendered every slice in the same orange
 * peach tone and buried the holdings list off-screen.
 *
 * The pieces:
 *   - <PanelHero>            — chrome-less hero header (eyebrow + big
 *                              gold value + meta line)
 *   - <AllocationDonutCard>  — donut + inline gold-dot legend in a card
 *   - <GoldBarsCard>         — labelled horizontal bars with gold-
 *                              gradient fills + value + share on right
 *   - <HoldingsCard>         — premium holdings list (symbol pill,
 *                              name, weight + optional day-delta, value,
 *                              chevron). Sorted by value desc.
 *
 * Every panel uses the gold ladder so the visual identity is one
 * across the app — no surprise colours per panel.
 */
import type { ReactNode } from "react";

import { AllocationDonut } from "@/components/dashboard/allocation-donut";
import type { CategoryAllocation, Holding } from "@/lib/types";
import { Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

/**
 * Mizan gold ladder — 8 stops, gold-cream → gold-deep. Wraps if a list
 * is longer than 8 (rare). Mirrors the ladder inside `AllocationDonut`
 * so a donut segment and the matching bar / legend dot share a colour.
 */
const GOLD_LADDER = [
  "hsl(40 67% 87%)",
  "hsl(31 49% 64%)",
  "hsl(45 62% 58%)",
  "hsl(31 42% 52%)",
  "hsl(31 38% 46%)",
  "hsl(31 32% 41%)",
  "hsl(31 28% 36%)",
  "hsl(31 24% 33%)",
] as const;

export function goldLadder(index: number): string {
  return GOLD_LADDER[index % GOLD_LADDER.length]!;
}

/* ────────────────────────────────────────────────────────────────────
 * Hero
 */
export function PanelHero({
  icon: Icon,
  label,
  value,
  baseCurrency,
  meta,
  empty = false,
  onAdd,
  addLabel = "Add",
}: {
  icon: typeof Icons.TrendingUp;
  label: string;
  value: number;
  baseCurrency: string;
  meta: string;
  empty?: boolean;
  /**
   * When provided, a primary "+ Add" action renders top-right of the
   * hero. Calling it should open the inline AddAssetDialog with a
   * panel-specific prompt so the assistant lands on the right
   * capability.
   */
  onAdd?: () => void;
  /** Override the button label — defaults to "Add". */
  addLabel?: string;
}) {
  return (
    <header className="flex items-start justify-between gap-4 pb-2">
      <div className="min-w-0 flex-1 space-y-2">
        <div className="text-muted-foreground flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
          <Icon className="h-3.5 w-3.5" />
          {label}
        </div>
        <div className="flex flex-col gap-1">
          <div className="text-foreground font-serif text-3xl font-semibold tabular-nums md:text-4xl">
            {empty ? "—" : formatAmount(value, baseCurrency)}
          </div>
          <p className="text-muted-foreground text-sm">{meta}</p>
        </div>
      </div>
      {onAdd && (
        <button
          type="button"
          onClick={onAdd}
          aria-label={`${addLabel} ${label}`}
          className="bg-foreground text-background hover:bg-foreground/90 focus-visible:ring-ring inline-flex h-9 shrink-0 items-center gap-1.5 rounded-full pl-3 pr-4 text-[13px] font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
        >
          <Icons.Plus className="h-4 w-4" />
          {addLabel}
        </button>
      )}
    </header>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Section card with the consistent eyebrow heading
 */
export function PanelCard({
  title,
  children,
  className = "",
  ariaLabel,
}: {
  title: string;
  children: ReactNode;
  className?: string;
  ariaLabel?: string;
}) {
  return (
    <section
      aria-label={ariaLabel ?? title}
      className={`bg-card flex flex-col gap-4 rounded-2xl border p-5 ${className}`}
    >
      <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
        {title}
      </h2>
      {children}
    </section>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Donut + inline legend
 */
export function AllocationDonutCard({
  title,
  categories,
  baseCurrency,
  emptyMessage = "No allocation data.",
  size = 172,
}: {
  title: string;
  categories: readonly CategoryAllocation[];
  baseCurrency: string;
  emptyMessage?: string;
  size?: number;
}) {
  if (categories.length === 0) {
    return (
      <PanelCard title={title}>
        <PanelEmpty message={emptyMessage} />
      </PanelCard>
    );
  }
  return (
    <PanelCard title={title} ariaLabel={`${title} allocation`}>
      <div className="flex flex-col items-center gap-5 sm:flex-row sm:items-center sm:justify-center sm:gap-6">
        <AllocationDonut
          categories={categories}
          baseCurrency={baseCurrency}
          size={size}
        />
        <ul className="w-full max-w-[200px] space-y-2.5">
          {categories.map((c, i) => (
            <li
              key={c.categoryId}
              className="flex items-center justify-between gap-3 text-[13px]"
            >
              <span className="flex min-w-0 items-center gap-2">
                <span
                  aria-hidden="true"
                  className="h-2.5 w-2.5 shrink-0 rounded-sm"
                  style={{ backgroundColor: goldLadder(i) }}
                />
                <span className="text-foreground/90 truncate">{c.categoryName}</span>
              </span>
              <span className="text-muted-foreground tabular-nums">
                {c.percentage.toFixed(1)}%
              </span>
            </li>
          ))}
        </ul>
      </div>
    </PanelCard>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Gold-gradient horizontal bars
 */
export interface BarRow {
  label: string;
  value: number;
  /** Optional secondary text beneath the label (e.g. "5 holdings"). */
  sub?: string;
}

export function GoldBarsCard({
  title,
  rows,
  baseCurrency,
  total,
  emptyMessage = "No data.",
}: {
  title: string;
  rows: readonly BarRow[];
  baseCurrency: string;
  /** Total used to compute each row's share %. Falls back to row sum. */
  total?: number;
  emptyMessage?: string;
}) {
  if (rows.length === 0) {
    return (
      <PanelCard title={title}>
        <PanelEmpty message={emptyMessage} />
      </PanelCard>
    );
  }
  const safeTotal = total ?? rows.reduce((s, r) => s + r.value, 0);
  const max = rows.reduce((m, r) => Math.max(m, r.value), 0);
  return (
    <PanelCard title={title}>
      <ul className="flex flex-col gap-3.5">
        {rows.map((r, i) => {
          const pct = safeTotal > 0 ? (r.value / safeTotal) * 100 : 0;
          const barPct = max > 0 ? (r.value / max) * 100 : 0;
          return (
            <li key={`${r.label}-${i}`} className="space-y-1.5">
              <div className="flex items-baseline justify-between gap-3">
                <span className="text-foreground truncate text-[13px] font-medium">
                  {r.label}
                  {r.sub ? (
                    <span className="text-muted-foreground ml-2 text-[11px]">
                      {r.sub}
                    </span>
                  ) : null}
                </span>
                <span className="text-muted-foreground shrink-0 text-[12px] tabular-nums">
                  {formatAmount(r.value, baseCurrency)}
                  <span className="text-foreground/40 ml-2">{pct.toFixed(1)}%</span>
                </span>
              </div>
              <div
                className="bg-muted/60 relative h-2 w-full overflow-hidden rounded-full"
                role="progressbar"
                aria-valuenow={Math.round(pct)}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-label={`${r.label} share ${pct.toFixed(1)}%`}
              >
                <span
                  className="absolute inset-y-0 left-0 rounded-full"
                  style={{
                    width: `${Math.max(2, barPct)}%`,
                    background: `linear-gradient(90deg, ${goldLadder(i)} 0%, ${goldLadder(i + 1)} 100%)`,
                  }}
                />
              </div>
            </li>
          );
        })}
      </ul>
    </PanelCard>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Holdings card — premium list, sorted by value desc.
 */
export function HoldingsCard({
  holdings,
  baseCurrency,
  totalExposure,
  classNoun,
  emptyMessage = "No holdings yet.",
  onSelect,
  title = "Holdings",
}: {
  holdings: readonly Holding[];
  baseCurrency: string;
  totalExposure: number;
  /** e.g. "equities" / "sukuks" / "crypto" — used in the per-row weight subtitle. */
  classNoun?: string;
  emptyMessage?: string;
  onSelect: (h: Holding) => void;
  title?: string;
}) {
  const sorted = holdings
    .slice()
    .sort((a, b) => (b.marketValue?.base ?? 0) - (a.marketValue?.base ?? 0));
  return (
    <section
      aria-label={title}
      className="bg-card overflow-hidden rounded-2xl border"
    >
      <header className="flex items-center justify-between border-b px-5 py-4">
        <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
          {title}
        </h2>
        <span className="text-muted-foreground text-xs tabular-nums">
          {sorted.length} {sorted.length === 1 ? "row" : "rows"}
        </span>
      </header>
      {sorted.length === 0 ? (
        <PanelEmpty message={emptyMessage} className="py-12" />
      ) : (
        <ul role="list" className="divide-border divide-y">
          {sorted.map((h) => (
            <HoldingRow
              key={h.id}
              holding={h}
              baseCurrency={baseCurrency}
              totalExposure={totalExposure}
              classNoun={classNoun}
              onSelect={() => onSelect(h)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function HoldingRow({
  holding,
  baseCurrency,
  totalExposure,
  classNoun,
  onSelect,
}: {
  holding: Holding;
  baseCurrency: string;
  totalExposure: number;
  classNoun?: string;
  onSelect: () => void;
}) {
  const name =
    holding.instrument?.name?.trim() ?? holding.instrument?.symbol ?? "—";
  const symbol = holding.instrument?.symbol?.toUpperCase() ?? "—";
  const value = holding.marketValue?.base ?? 0;
  const weight = totalExposure > 0 ? (value / totalExposure) * 100 : 0;
  const day = holding.dayChangePct;
  const dayUp = (day ?? 0) >= 0;
  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className="hover:bg-muted/40 focus-visible:bg-muted/40 group flex w-full items-center gap-3 px-5 py-3.5 text-left transition-colors focus:outline-none"
      >
        <span className="bg-muted/60 text-foreground/80 grid h-9 w-12 shrink-0 place-items-center rounded-md text-[11px] font-semibold tracking-tight tabular-nums">
          {symbol.slice(0, 5)}
        </span>
        <span className="min-w-0 flex-1">
          <span className="text-foreground block truncate text-[13px] font-medium">
            {name}
          </span>
          <span className="text-muted-foreground text-[11px] tabular-nums">
            {weight.toFixed(1)}%
            {classNoun ? ` of ${classNoun}` : ""}
            {typeof day === "number" && day !== 0 && (
              <span className={dayUp ? "text-success ml-2" : "text-destructive ml-2"}>
                {dayUp ? "▲" : "▼"} {Math.abs(day).toFixed(2)}%
              </span>
            )}
          </span>
        </span>
        <span className="text-foreground shrink-0 text-right text-[14px] font-semibold tabular-nums">
          {formatAmount(value, baseCurrency)}
        </span>
        <Icons.ChevronRight
          className="text-muted-foreground/40 group-hover:text-muted-foreground h-4 w-4 shrink-0 transition-colors"
          aria-hidden="true"
        />
      </button>
    </li>
  );
}

export function PanelEmpty({
  message,
  className = "py-12",
}: {
  message: string;
  className?: string;
}) {
  return (
    <div
      className={`text-muted-foreground flex flex-1 items-center justify-center text-center text-sm ${className}`}
    >
      {message}
    </div>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Shared page shell — `<div>` with the canonical max-width / padding /
 * stacking, used so all panels feel the same at the page level too.
 */
export function PanelShell({ children }: { children: ReactNode }) {
  return (
    <div className="mx-auto max-w-6xl space-y-4 px-4 py-6 md:px-6 md:py-8 lg:px-10">
      {children}
    </div>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Two-column glance row helper. Pass two children; on `md:` they sit
 * side-by-side, on small screens they stack.
 */
export function PanelGlanceRow({ children }: { children: ReactNode }) {
  return <div className="grid gap-4 md:grid-cols-2">{children}</div>;
}

/* ────────────────────────────────────────────────────────────────────
 * Adapter: turn an array of `{label, value}` rows into the
 * `CategoryAllocation[]` shape the donut expects. Sorted desc so the
 * brightest gold lands on the largest slice.
 */
export function rowsToCategories(
  rows: readonly { label: string; value: number }[],
): CategoryAllocation[] {
  const positive = rows.filter((r) => r.value > 0);
  const total = positive.reduce((s, r) => s + r.value, 0);
  if (total <= 0) return [];
  return positive
    .slice()
    .sort((a, b) => b.value - a.value)
    .map((r) => ({
      categoryId: r.label,
      categoryName: r.label,
      color: "",
      value: r.value,
      percentage: (r.value / total) * 100,
    }));
}
