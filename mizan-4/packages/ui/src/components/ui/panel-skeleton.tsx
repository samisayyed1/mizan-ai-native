import { cn } from "../../lib/utils";

/**
 * Standard loading skeleton for an asset class panel card.
 *
 * Matches the populated card's silhouette so the layout doesn't jump
 * when data resolves:
 *   - left: round icon chip + two stacked text bars
 *   - right: large value bar + smaller delta bar + chevron-sized cell
 *
 * Use this anywhere you'd otherwise hand-roll a Skeleton-only panel
 * (the bell `PanelSkeleton`, the holdings/news/goals pages, etc.) so
 * loading states feel coherent across the app.
 *
 * Pure presentation — no animation knob beyond Tailwind's
 * `animate-pulse`, which is already what `<Skeleton>` uses elsewhere.
 */
interface PanelSkeletonProps {
  /** Number of skeleton rows to render. Defaults to 1 (a single panel card). */
  rows?: number;
  className?: string;
}

export function PanelSkeleton({ rows = 1, className }: PanelSkeletonProps) {
  return (
    <div className={cn("space-y-4", className)} aria-hidden="true">
      {Array.from({ length: rows }).map((_, i) => (
        <PanelSkeletonRow key={i} />
      ))}
    </div>
  );
}

function PanelSkeletonRow() {
  return (
    <div className="bg-card flex items-center justify-between gap-4 rounded-2xl border p-5 md:p-6">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <div className="bg-muted h-11 w-11 shrink-0 animate-pulse rounded-xl" />
        <div className="flex-1 space-y-2">
          <div className="bg-muted h-4 w-32 animate-pulse rounded" />
          <div className="bg-muted h-3 w-20 animate-pulse rounded" />
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <div className="space-y-2 text-right">
          <div className="bg-muted ml-auto h-6 w-24 animate-pulse rounded" />
          <div className="bg-muted ml-auto h-3 w-16 animate-pulse rounded" />
        </div>
        <div className="bg-muted h-4 w-4 animate-pulse rounded" />
      </div>
    </div>
  );
}
