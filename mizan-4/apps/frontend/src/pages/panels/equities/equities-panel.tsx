/**
 * Equities panel — Track B PR-B2 / Goal v3 §V Phase 5 step B2.
 *
 * World-class IA redesign (ADR 0018b·v6):
 *   - Tight chrome-less header: eyebrow ("EQUITIES") + big gold value +
 *     tagline ("N positions across M regions").
 *   - Two-column glance row on desktop:
 *       · Allocation by sub-class — gold-laddered donut (the same
 *         `AllocationDonut` used in the Net Worth strip, so the visual
 *         identity is consistent across the app).
 *       · Exposure by region — labelled horizontal bars with gold
 *         gradient fills, value + share on the right.
 *   - Holdings table: symbol badge + name + region + share + value
 *     + a per-row chevron tap target. The list never gets buried —
 *     it sits in its own card with a clear "Holdings (N)" heading.
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { AllocationDonut } from "@/components/dashboard/allocation-donut";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import type { CategoryAllocation, Holding } from "@/lib/types";
import { Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  isEquityHolding,
  rollupByRegion,
  rollupBySubclass,
  totalEquityExposure,
} from "./rollup";

export default function EquitiesPanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const equityHoldings = useMemo(
    () => (allHoldings ?? []).filter(isEquityHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalEquityExposure(allHoldings ?? []),
    [allHoldings],
  );
  const subclassRows = useMemo(
    () => rollupBySubclass(allHoldings ?? []),
    [allHoldings],
  );
  const regionRows = useMemo(() => rollupByRegion(allHoldings ?? []), [allHoldings]);

  // Convert sub-class rows into the donut's `CategoryAllocation[]`
  // shape. The donut overrides colour via its gold ladder, so `color`
  // is just a contract field.
  const donutCategories = useMemo<CategoryAllocation[]>(() => {
    const total = subclassRows.reduce((s, r) => s + r.value, 0);
    if (total <= 0) return [];
    return subclassRows.map((r) => ({
      categoryId: r.label,
      categoryName: r.label,
      color: "",
      value: r.value,
      percentage: (r.value / total) * 100,
    }));
  }, [subclassRows]);

  const empty = equityHoldings.length === 0 && !isLoading;

  return (
    <div className="mx-auto max-w-6xl space-y-4 px-4 py-6 md:px-6 md:py-8 lg:px-10">
      {/* Header — eyebrow + hero value + meta. No card chrome here:
          the value IS the focal point. */}
      <header className="space-y-2 pb-2">
        <div className="text-muted-foreground flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
          <Icons.TrendingUp className="h-3.5 w-3.5" />
          Equities
        </div>
        <div className="flex flex-col gap-1">
          <div className="text-foreground font-serif text-3xl font-semibold tabular-nums md:text-4xl">
            {empty ? "—" : formatAmount(totalExposure, baseCurrency)}
          </div>
          <p className="text-muted-foreground text-sm">
            {empty
              ? "No equity holdings yet."
              : `${equityHoldings.length} ${equityHoldings.length === 1 ? "position" : "positions"}${regionRows.length > 0 ? ` · ${regionRows.length} ${regionRows.length === 1 ? "region" : "regions"}` : ""}`}
          </p>
        </div>
      </header>

      {empty ? null : (
        <>
          {/* Glance row — donut + region bars. Stacks on small screens,
              side-by-side on `md:` and up. */}
          <div className="grid gap-4 md:grid-cols-2">
            <section
              aria-label="Equity sub-class allocation"
              className="bg-card flex flex-col gap-4 rounded-2xl border p-5"
            >
              <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
                By sub-class
              </h2>
              {donutCategories.length === 0 ? (
                <EmptyChart message="No allocation data." />
              ) : (
                <div className="flex flex-col items-center gap-5 sm:flex-row sm:items-center sm:justify-center sm:gap-6">
                  <AllocationDonut
                    categories={donutCategories}
                    baseCurrency={baseCurrency}
                    size={172}
                  />
                  {/* Inline legend — colour dot · label · share */}
                  <ul className="w-full max-w-[200px] space-y-2.5">
                    {donutCategories.map((c, i) => (
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
                          <span className="text-foreground/90 truncate">
                            {c.categoryName}
                          </span>
                        </span>
                        <span className="text-muted-foreground tabular-nums">
                          {c.percentage.toFixed(1)}%
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </section>

            <section
              aria-label="Equity geographic exposure"
              className="bg-card flex flex-col gap-4 rounded-2xl border p-5"
            >
              <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
                By region
              </h2>
              {regionRows.length === 0 ? (
                <EmptyChart message="No region data — classify holdings in their detail page." />
              ) : (
                <RegionBars
                  rows={regionRows}
                  baseCurrency={baseCurrency}
                  total={totalExposure}
                />
              )}
            </section>
          </div>

          {/* Holdings table — gets equal billing with the charts, no
              longer buried at the bottom. */}
          <section
            aria-label="Equity holdings"
            className="bg-card overflow-hidden rounded-2xl border"
          >
            <header className="flex items-center justify-between border-b px-5 py-4">
              <h2 className="text-muted-foreground text-xs font-semibold uppercase tracking-wider">
                Holdings
              </h2>
              <span className="text-muted-foreground text-xs tabular-nums">
                {equityHoldings.length} {equityHoldings.length === 1 ? "row" : "rows"}
              </span>
            </header>
            <ul role="list" className="divide-border divide-y">
              {equityHoldings
                .slice()
                .sort(
                  (a, b) => (b.marketValue?.base ?? 0) - (a.marketValue?.base ?? 0),
                )
                .map((h) => (
                  <HoldingRow
                    key={h.id}
                    holding={h}
                    baseCurrency={baseCurrency}
                    totalExposure={totalExposure}
                    onSelect={() =>
                      navigate(`/holdings/${h.instrument?.id ?? h.id}`)
                    }
                  />
                ))}
            </ul>
          </section>
        </>
      )}
    </div>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Region bars — horizontal bars with gold-gradient fills, share + value
 * on the right. Bars are sized against the largest row so the user
 * reads relative weight at a glance.
 */
function RegionBars({
  rows,
  baseCurrency,
  total,
}: {
  rows: readonly { region: string; exposureBase: number }[];
  baseCurrency: string;
  total: number;
}) {
  const max = rows.reduce((m, r) => Math.max(m, r.exposureBase), 0);
  return (
    <ul className="flex flex-col gap-3.5">
      {rows.map((r, i) => {
        const pct = total > 0 ? (r.exposureBase / total) * 100 : 0;
        const barPct = max > 0 ? (r.exposureBase / max) * 100 : 0;
        return (
          <li key={r.region} className="space-y-1.5">
            <div className="flex items-baseline justify-between gap-3">
              <span className="text-foreground truncate text-[13px] font-medium">
                {r.region}
              </span>
              <span className="text-muted-foreground shrink-0 text-[12px] tabular-nums">
                {formatAmount(r.exposureBase, baseCurrency)}
                <span className="text-foreground/40 ml-2">{pct.toFixed(1)}%</span>
              </span>
            </div>
            <div
              className="bg-muted/60 relative h-2 w-full overflow-hidden rounded-full"
              role="progressbar"
              aria-valuenow={Math.round(pct)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={`${r.region} exposure ${pct.toFixed(1)}%`}
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
  );
}

/* ────────────────────────────────────────────────────────────────────
 * Holding row — symbol pill + name + day change on the left, value +
 * weight on the right. Hover lifts; tap routes to /holdings/:id.
 */
function HoldingRow({
  holding,
  baseCurrency,
  totalExposure,
  onSelect,
}: {
  holding: Holding;
  baseCurrency: string;
  totalExposure: number;
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
        {/* Symbol pill */}
        <span className="bg-muted/60 text-foreground/80 grid h-9 w-12 shrink-0 place-items-center rounded-md text-[11px] font-semibold tracking-tight tabular-nums">
          {symbol.slice(0, 5)}
        </span>
        <span className="min-w-0 flex-1">
          <span className="text-foreground block truncate text-[13px] font-medium">
            {name}
          </span>
          <span className="text-muted-foreground text-[11px] tabular-nums">
            {weight.toFixed(1)}% of equities
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

function EmptyChart({ message }: { message: string }) {
  return (
    <div className="text-muted-foreground flex flex-1 items-center justify-center py-12 text-center text-sm">
      {message}
    </div>
  );
}

/**
 * Mizan gold ladder — 8 stops descending from gold-cream → gold-deep.
 * Wraps at the end so an unexpectedly long list still gets distinct
 * tones. Mirrors the ladder in `AllocationDonut` for cross-surface
 * consistency.
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

function goldLadder(index: number): string {
  return GOLD_LADDER[index % GOLD_LADDER.length]!;
}
