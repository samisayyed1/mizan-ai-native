/**
 * Today's Signal — Track A PR-A7 / Goal v3 §V Phase 1 step A7.
 *
 * One card. The single highest-signal notification from the
 * deterministic `crates/insights` rule engine, surfaced on the
 * dashboard so the user sees one actionable thing per day without
 * opening the bell.
 *
 * Selection algorithm (per Goal v3 §V step A7 "single highest-signal
 * card per day"):
 *
 *   1. Filter to notifications from today (since 00:00 local).
 *   2. Drop anything dismissed.
 *   3. Sort by severity (critical > warning > info > success).
 *   4. Within a severity tier, prefer notifications that are still
 *      unread, then most-recent-first.
 *   5. Pick the head.
 *
 * The §23 scenario's *"Your Emaar Sukuk matures in 47 days"* is
 * exactly the shape this surface targets. The current
 * `crates/insights` rule set ships `cash_drag`, `big_move`,
 * `goal_milestone`, `net_worth_dip`, `allocation_drift`,
 * `dividend_posted`, `new_ath`, and `sync_failure`. Track C5.b adds
 * `BondMaturityApproaching`, `ZakatHawlApproaching`,
 * `ShariaStatusChanged`, `FXMovedMaterially`, `ConcentrationRisk` —
 * those flow through this same card surface automatically once they
 * emit `Notification`s with the right severity.
 *
 * Tap behaviour:
 *   - When the notification has a `deepLink` (mizan://...), tap
 *     navigates there (stripping the scheme to a route path).
 *   - Otherwise tap expands the card to show the rule's full body
 *     text — the "reasoning trail" per Goal v3 step A7.
 *
 * What this PR does NOT do (out of §V step A7 scope, deferred to
 * tracked follow-ups):
 *   - AI-rendered natural language paraphrase. The current `body`
 *     comes from the deterministic engine. PR-C5.c selects + renders.
 *   - Dismiss-from-card. Use the bell.
 *   - Cross-day dedupe (Goal v3 says "deduplicated against last 7d").
 *     The engine already dedupes via `dedupe_key`; the card just
 *     consumes whatever the engine emitted today.
 */
import type { Notification, NotificationSeverity } from "@/adapters/types-notifications";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

export interface TodaysSignalCardProps {
  /** Notification feed from `listNotifications(25)` or equivalent. */
  notifications: readonly Notification[] | undefined | null;
  /** Loading flag — renders a skeleton instead of "no signals". */
  isLoading?: boolean;
  /** Override the per-render baseline. Useful for tests. Defaults to now. */
  nowMs?: number;
}

const SEVERITY_RANK: Record<NotificationSeverity, number> = {
  critical: 0,
  warning: 1,
  info: 2,
  success: 3,
};

/**
 * Pure selector — extracted for direct testing. Picks the single
 * highest-signal notification per the algorithm above, or null if
 * today has none.
 */
export function pickTodaysSignal(
  notifications: readonly Notification[],
  nowMs: number,
): Notification | null {
  if (notifications.length === 0) return null;
  // Local day start — matches what the user would consider "today".
  const d = new Date(nowMs);
  const startOfDayMs = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

  const candidates = notifications.filter(
    (n) => n.createdAtMs >= startOfDayMs && n.dismissedAtMs === null,
  );
  if (candidates.length === 0) return null;

  // Sort: severity asc (lower rank = higher severity), unread first,
  // then most-recent first.
  const sorted = [...candidates].sort((a, b) => {
    const rankDiff = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
    if (rankDiff !== 0) return rankDiff;
    const aUnread = a.readAtMs === null ? 0 : 1;
    const bUnread = b.readAtMs === null ? 0 : 1;
    if (aUnread !== bUnread) return aUnread - bUnread;
    return b.createdAtMs - a.createdAtMs;
  });
  return sorted[0] ?? null;
}

function severityStyles(severity: NotificationSeverity): {
  ring: string;
  bg: string;
  text: string;
  icon: string;
  label: string;
} {
  switch (severity) {
    case "critical":
      return {
        ring: "border-rose-500/40",
        bg: "bg-rose-500/10",
        text: "text-rose-700 dark:text-rose-300",
        icon: "text-rose-600 dark:text-rose-400",
        label: "Action",
      };
    case "warning":
      return {
        ring: "border-amber-500/40",
        bg: "bg-amber-500/10",
        text: "text-amber-800 dark:text-amber-200",
        icon: "text-amber-600 dark:text-amber-400",
        label: "Heads up",
      };
    case "success":
      return {
        ring: "border-emerald-500/40",
        bg: "bg-emerald-500/10",
        text: "text-emerald-800 dark:text-emerald-200",
        icon: "text-emerald-600 dark:text-emerald-400",
        label: "Milestone",
      };
    case "info":
    default:
      return {
        ring: "border-sky-500/40",
        bg: "bg-sky-500/10",
        text: "text-sky-800 dark:text-sky-200",
        icon: "text-sky-600 dark:text-sky-400",
        label: "Today's Signal",
      };
  }
}

/**
 * Convert a `mizan://route/path?q=1` deep link to a router-friendly
 * `/route/path?q=1`. Anything that doesn't start with `mizan://` is
 * returned as-is. Unknown shapes fall back to `null` so the caller
 * keeps the click as a tap-to-expand.
 */
function deepLinkToRoute(link: string | null | undefined): string | null {
  if (!link) return null;
  if (link.startsWith("mizan://")) return "/" + link.slice("mizan://".length);
  if (link.startsWith("/")) return link;
  return null;
}

export function TodaysSignalCard({
  notifications,
  isLoading = false,
  nowMs,
}: TodaysSignalCardProps) {
  const navigate = useNavigate();
  const [expanded, setExpanded] = useState(false);

  // Compute once per render. The selector is pure + cheap; no useEffect.
  const baselineMs = nowMs ?? Date.now();
  const signal = useMemo(
    () => pickTodaysSignal(notifications ?? [], baselineMs),
    [notifications, baselineMs],
  );

  if (isLoading) {
    return (
      <section
        aria-label="Today's Signal"
        className="bg-card rounded-2xl border p-4"
        data-testid="todays-signal-loading"
      >
        <div className="bg-muted h-4 w-32 animate-pulse rounded" />
        <div className="bg-muted mt-3 h-5 w-2/3 animate-pulse rounded" />
        <div className="bg-muted mt-2 h-4 w-1/2 animate-pulse rounded" />
      </section>
    );
  }

  if (!signal) {
    return (
      <section
        aria-label="Today's Signal"
        className="bg-card text-muted-foreground rounded-2xl border p-4 text-sm"
        data-testid="todays-signal-empty"
      >
        <div className="flex items-center gap-2">
          <Icons.Sparkles className="h-4 w-4" />
          <span>No signal today — your portfolio is quiet.</span>
        </div>
      </section>
    );
  }

  const styles = severityStyles(signal.severity);
  const route = deepLinkToRoute(signal.deepLink);
  const hasRoute = route !== null;

  const handleTap = () => {
    if (hasRoute && route) {
      navigate(route);
      return;
    }
    setExpanded((prev) => !prev);
  };

  return (
    <section
      aria-label="Today's Signal"
      className={`relative rounded-2xl border ${styles.ring} ${styles.bg} p-4`}
      data-testid="todays-signal-card"
      data-severity={signal.severity}
    >
      <button
        type="button"
        onClick={handleTap}
        className="focus-visible:ring-ring w-full text-left focus-visible:outline-none focus-visible:ring-2 rounded-lg"
        aria-label={hasRoute ? `${styles.label}: ${signal.title} (opens detail)` : `${styles.label}: ${signal.title} (tap to expand reasoning)`}
        data-testid="todays-signal-tap"
      >
        <div className="flex items-start gap-3">
          <div
            className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-full ${styles.bg} ${styles.icon}`}
          >
            <Icons.Sparkles className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <div className={`text-xs uppercase tracking-wider ${styles.text}`}>
              {styles.label}
            </div>
            <div className="text-foreground mt-1 text-sm font-medium">{signal.title}</div>
            {(expanded || !hasRoute) && signal.body && (
              <p
                className="text-muted-foreground mt-2 text-sm leading-5"
                data-testid="todays-signal-body"
              >
                {signal.body}
              </p>
            )}
          </div>
          <div className="text-muted-foreground shrink-0">
            {hasRoute ? (
              <Icons.ChevronRight className="h-4 w-4" />
            ) : expanded ? (
              <Icons.ChevronDown className="h-4 w-4" />
            ) : (
              <Icons.ChevronRight className="h-4 w-4" />
            )}
          </div>
        </div>
      </button>
    </section>
  );
}
