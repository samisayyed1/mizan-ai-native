import type { ImportQuoteSyncReport } from "@/lib/types";

/**
 * Banner shown after a successful CSV import that ran a synchronous
 * quote sync. Tells the user exactly how many positions got live
 * prices vs how many are using the cost-basis fallback — so the
 * dashboard total has surgical clarity instead of looking "wrong but
 * eventually consistent".
 *
 * Renders nothing when every asset got a live quote (the common
 * case for established symbols on a working network) — no noise
 * when there's nothing to communicate.
 */
export interface QuoteSyncReportBannerProps {
  report: ImportQuoteSyncReport;
}

export function QuoteSyncReportBanner({ report }: QuoteSyncReportBannerProps) {
  const liveCount = report.livePriced.length;
  const failedCount = report.failed.length;
  const notFoundCount = report.notFound.length;
  const skippedCount = report.skipped.length;
  const totalAttended = liveCount + failedCount + notFoundCount + skippedCount;

  // Nothing went wrong — no banner.
  if (failedCount === 0 && notFoundCount === 0 && skippedCount === 0) {
    return null;
  }

  // All live (defensive — already handled above, but keeps the
  // ratio math safe).
  if (totalAttended === 0) {
    return null;
  }

  const liveRatio = Math.round((liveCount / totalAttended) * 100);

  return (
    <div className="bg-card text-card-foreground rounded-2xl border p-5 shadow-sm">
      <div className="flex flex-wrap items-baseline justify-between gap-4">
        <div>
          <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
            Live quote sync
          </p>
          <h3 className="mt-1 text-xl font-semibold tabular-nums">
            {liveCount}{" "}
            <span className="text-muted-foreground text-base font-normal">
              of {totalAttended} symbols got live prices ({liveRatio}%)
            </span>
          </h3>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {failedCount > 0 ? (
            <Chip
              label="Failed"
              count={failedCount}
              tone="error"
              titleText="Quote provider errored (rate-limit, network, etc.). These positions will show cost basis until the next sync. Will retry automatically."
            />
          ) : null}
          {notFoundCount > 0 ? (
            <Chip
              label="Not found"
              count={notFoundCount}
              tone="warning"
              titleText="Provider doesn't carry these symbols (delisted, OTC, foreign exchange). These positions will permanently show cost basis unless you switch quote providers."
            />
          ) : null}
          {skippedCount > 0 ? (
            <Chip
              label="Skipped"
              count={skippedCount}
              tone="info"
              titleText="Cash, manual-pricing, expired options. Not an error — informational."
            />
          ) : null}
        </div>
      </div>

      {failedCount > 0 || notFoundCount > 0 ? (
        <p className="text-muted-foreground mt-3 text-xs leading-relaxed">
          Positions without live prices are valued at <strong>cost basis</strong> on the dashboard
          until live data arrives. The portfolio total includes them. Open Settings → Data Health to
          manage stale-quote retries.
        </p>
      ) : null}

      {report.quotesAdded > 0 ? (
        <p className="text-muted-foreground mt-2 text-xs">
          {report.quotesAdded} quote{report.quotesAdded === 1 ? "" : "s"} fetched and persisted.
        </p>
      ) : null}
    </div>
  );
}

interface ChipProps {
  label: string;
  count: number;
  tone: "error" | "warning" | "info";
  titleText: string;
}

function Chip({ label, count, tone, titleText }: ChipProps) {
  const toneClass =
    tone === "error"
      ? "border-rose-200 bg-rose-50 text-rose-900 dark:border-rose-900/50 dark:bg-rose-950/40 dark:text-rose-200"
      : tone === "warning"
        ? "border-amber-200 bg-amber-50 text-amber-900 dark:border-amber-900/50 dark:bg-amber-950/40 dark:text-amber-200"
        : "border-muted bg-muted/50 text-muted-foreground";
  return (
    <span
      title={titleText}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${toneClass}`}
    >
      <span>{label}</span>
      <span className="font-mono tabular-nums">{count}</span>
    </span>
  );
}
