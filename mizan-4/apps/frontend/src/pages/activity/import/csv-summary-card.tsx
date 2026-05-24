import { useMemo } from "react";
import type { CsvImportSummary } from "@/lib/types";

/**
 * Headline summary card for a parsed CSV import — the user's
 * confidence-check before committing.
 *
 * Shows three things side-by-side:
 *
 *   1. **Rows kept / dropped** — broken down by reason so the user
 *      can spot a wrong column mapping (e.g. "we kept 5 / 358" is
 *      a clear sign Quantity got mapped to a non-numeric column).
 *   2. **BUY cost basis & SELL proceeds totals** — the headline
 *      dollars (in the account currency the user picks at commit
 *      time). If this number is wildly off what the user expects
 *      their portfolio is worth, the mapping is probably wrong.
 *   3. **Unique symbols / net positions** — sanity check the file
 *      contains the brokers they think.
 *
 * Designed to live above the preview table on the import wizard.
 * Compact, no charts, just numbers.
 */
export interface CsvSummaryCardProps {
  summary: CsvImportSummary;
  /** Account currency the user has chosen for this import (e.g. "USD",
   *  "SGD"). Drives the formatting of the BUY/SELL totals. */
  currency: string;
}

const fmtMoney = (n: number, currency: string) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(n);

const fmtInt = (n: number) => new Intl.NumberFormat().format(n);

interface DropChipProps {
  label: string;
  count: number;
  tone: "watchlist" | "junk" | "dupe";
}

function DropChip({ label, count, tone }: DropChipProps) {
  if (count <= 0) return null;
  const toneClass =
    tone === "junk"
      ? "border-orange-200 bg-orange-50 text-orange-900 dark:border-orange-900/50 dark:bg-orange-950/40 dark:text-orange-200"
      : tone === "dupe"
        ? "border-amber-200 bg-amber-50 text-amber-900 dark:border-amber-900/50 dark:bg-amber-950/40 dark:text-amber-200"
        : "border-muted bg-muted/50 text-muted-foreground";
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${toneClass}`}
    >
      <span>{label}</span>
      <span className="font-mono tabular-nums">{fmtInt(count)}</span>
    </span>
  );
}

export function CsvSummaryCard({ summary, currency }: CsvSummaryCardProps) {
  const { stats } = summary;

  const totalDropped = useMemo(
    () =>
      stats.watchlist_dropped +
      stats.missing_symbol_dropped +
      stats.zero_or_invalid_value_dropped +
      stats.duplicates_dropped,
    [stats],
  );

  const keptPct =
    stats.total_input_rows > 0 ? Math.round((stats.kept / stats.total_input_rows) * 100) : 0;

  return (
    <div className="bg-card text-card-foreground rounded-2xl border p-5 shadow-sm">
      <div className="flex flex-wrap items-baseline justify-between gap-4">
        <div>
          <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
            Import preview
          </p>
          <h3 className="mt-1 text-2xl font-semibold tabular-nums">
            {fmtInt(stats.kept)}{" "}
            <span className="text-muted-foreground text-base font-normal">
              of {fmtInt(stats.total_input_rows)} rows ready ({keptPct}%)
            </span>
          </h3>
        </div>
        {totalDropped > 0 ? (
          <div className="flex flex-wrap items-center gap-1.5">
            <DropChip label="Watchlist" count={stats.watchlist_dropped} tone="watchlist" />
            <DropChip label="Missing symbol" count={stats.missing_symbol_dropped} tone="junk" />
            <DropChip
              label="Zero/invalid"
              count={stats.zero_or_invalid_value_dropped}
              tone="junk"
            />
            <DropChip label="Duplicates" count={stats.duplicates_dropped} tone="dupe" />
          </div>
        ) : null}
      </div>

      <div className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
        <Stat
          label="BUY cost basis"
          value={fmtMoney(summary.total_buy_cost_basis, currency)}
          sub={`${fmtInt(summary.buy_count)} ${summary.buy_count === 1 ? "row" : "rows"}`}
          accent="positive"
        />
        <Stat
          label="SELL proceeds"
          value={fmtMoney(summary.total_sell_proceeds, currency)}
          sub={`${fmtInt(summary.sell_count)} ${summary.sell_count === 1 ? "row" : "rows"}`}
          accent="negative"
        />
        <Stat
          label="Unique symbols"
          value={fmtInt(summary.unique_symbols)}
          sub={`${fmtInt(summary.symbols_with_net_position)} with open position`}
        />
        <Stat
          label="Fees"
          value={fmtMoney(summary.total_fees, currency)}
          sub={summary.other_count > 0 ? `${fmtInt(summary.other_count)} other rows` : ""}
        />
      </div>

      {keptPct < 50 && stats.total_input_rows > 0 ? (
        <p className="text-muted-foreground mt-4 text-xs leading-relaxed">
          Most rows were filtered. Double-check the column mapping below — if the wrong column was
          picked for Quantity or Purchase Price, rows look like &quot;zero/invalid&quot; even when
          the CSV is fine.
        </p>
      ) : null}
    </div>
  );
}

function Stat({
  label,
  value,
  sub,
  accent,
}: {
  label: string;
  value: string;
  sub?: string;
  accent?: "positive" | "negative";
}) {
  const valueClass =
    accent === "positive"
      ? "text-emerald-700 dark:text-emerald-300"
      : accent === "negative"
        ? "text-rose-700 dark:text-rose-300"
        : "text-foreground";
  return (
    <div className="space-y-0.5">
      <p className="text-muted-foreground text-[11px] font-medium uppercase tracking-wider">
        {label}
      </p>
      <p className={`text-lg font-semibold tabular-nums ${valueClass}`}>{value}</p>
      {sub ? <p className="text-muted-foreground text-xs">{sub}</p> : null}
    </div>
  );
}
