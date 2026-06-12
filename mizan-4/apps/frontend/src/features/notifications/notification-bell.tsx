// Apple-grade notification bell + popover panel (Notify-7).
//
// Lives in the sidebar chrome. The badge polls the unread count via
// React Query (cheap, just a SQLite COUNT) and the Notify-5 scheduler
// pushes a `notifications:new` event that invalidates the query for
// instant refresh — so the user never sees a stale count.

import {
  dismissNotification,
  listenNotificationsNew,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  notificationsUnreadCount,
} from "@/adapters";
import type {
  Notification,
  NotificationKind,
  NotificationSeverity,
  NotificationsPage,
} from "@/adapters/types-notifications";
import { QueryKeys } from "@/lib/query-keys";
import { cn } from "@/lib/utils";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@mizan/ui/components/ui/popover";
import { ScrollArea } from "@mizan/ui/components/ui/scroll-area";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

interface NotificationBellProps {
  collapsed: boolean;
}

export function NotificationBell({ collapsed }: NotificationBellProps) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);

  const unreadQuery = useQuery({
    queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT],
    queryFn: notificationsUnreadCount,
    // 30s background refresh — keeps the badge fresh without
    // hammering SQLite. The Notify-5 push event invalidates this
    // instantly on actual new rows, so the polling is a safety net
    // for missed events / cold-start.
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
  });

  const pageQuery = useQuery({
    queryKey: QueryKeys.notifications(25),
    queryFn: () => listNotifications(25),
    // Only fetch the heavy page when the popover is actually open;
    // the badge alone doesn't need the rows.
    enabled: open,
  });

  // All three mutations use the optimistic-update / rollback / settle
  // pattern so a click instantly reflects in the UI — no perceived
  // "did the button do anything?" pause while the round-trip lands.
  // If the backend later errors, we roll back to the snapshot. On
  // settle we invalidate so the next refetch reconciles with the DB.

  const cancelNotifQueries = async () => {
    await queryClient.cancelQueries({ queryKey: QueryKeys.notifications(25) });
    await queryClient.cancelQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
  };

  const snapshotNotifQueries = () => ({
    page: queryClient.getQueryData<NotificationsPage>(QueryKeys.notifications(25)),
    unread: queryClient.getQueryData<number>([QueryKeys.NOTIFICATIONS_UNREAD_COUNT]),
  });

  const rollbackNotifQueries = (snap: ReturnType<typeof snapshotNotifQueries>) => {
    if (snap.page !== undefined) {
      queryClient.setQueryData(QueryKeys.notifications(25), snap.page);
    }
    if (snap.unread !== undefined) {
      queryClient.setQueryData([QueryKeys.NOTIFICATIONS_UNREAD_COUNT], snap.unread);
    }
  };

  const invalidateNotifQueries = () => {
    queryClient.invalidateQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
    queryClient.invalidateQueries({ queryKey: QueryKeys.notifications(25) });
  };

  const markReadMutation = useMutation({
    mutationFn: (id: string) => markNotificationRead(id),
    onMutate: async (id) => {
      await cancelNotifQueries();
      const snap = snapshotNotifQueries();
      const nowMs = Date.now();
      queryClient.setQueryData<NotificationsPage>(QueryKeys.notifications(25), (old) => {
        if (!old) return old;
        let touched = false;
        const items = old.items.map((n) => {
          if (n.id === id && n.readAtMs === null) {
            touched = true;
            return { ...n, readAtMs: nowMs };
          }
          return n;
        });
        return {
          ...old,
          items,
          unreadCount: Math.max(0, old.unreadCount - (touched ? 1 : 0)),
        };
      });
      queryClient.setQueryData<number>([QueryKeys.NOTIFICATIONS_UNREAD_COUNT], (n) =>
        Math.max(0, (n ?? 0) - 1),
      );
      return snap;
    },
    onError: (_err, _id, snap) => snap && rollbackNotifQueries(snap),
    onSettled: invalidateNotifQueries,
  });

  const dismissMutation = useMutation({
    mutationFn: (id: string) => dismissNotification(id),
    onMutate: async (id) => {
      await cancelNotifQueries();
      const snap = snapshotNotifQueries();
      // Look at the snapshot (not the live cache) to decide whether
      // the dismissed row was unread — we're about to mutate the
      // cache so reading from it would race.
      const dropped = snap.page?.items.find((n) => n.id === id);
      const wasUnread =
        !!dropped && dropped.readAtMs === null && dropped.dismissedAtMs === null;
      queryClient.setQueryData<NotificationsPage>(QueryKeys.notifications(25), (old) =>
        old
          ? {
              ...old,
              items: old.items.filter((n) => n.id !== id),
              unreadCount: Math.max(0, old.unreadCount - (wasUnread ? 1 : 0)),
            }
          : old,
      );
      queryClient.setQueryData<number>([QueryKeys.NOTIFICATIONS_UNREAD_COUNT], (n) =>
        Math.max(0, (n ?? 0) - (wasUnread ? 1 : 0)),
      );
      return snap;
    },
    onError: (_err, _id, snap) => snap && rollbackNotifQueries(snap),
    onSettled: invalidateNotifQueries,
  });

  const markAllMutation = useMutation({
    mutationFn: markAllNotificationsRead,
    onMutate: async () => {
      await cancelNotifQueries();
      const snap = snapshotNotifQueries();
      const nowMs = Date.now();
      queryClient.setQueryData<NotificationsPage>(QueryKeys.notifications(25), (old) => {
        if (!old) return old;
        return {
          ...old,
          items: old.items.map((n) =>
            n.readAtMs === null && n.dismissedAtMs === null ? { ...n, readAtMs: nowMs } : n,
          ),
          unreadCount: 0,
        };
      });
      queryClient.setQueryData<number>([QueryKeys.NOTIFICATIONS_UNREAD_COUNT], 0);
      return snap;
    },
    onError: (_err, _vars, snap) => snap && rollbackNotifQueries(snap),
    onSettled: invalidateNotifQueries,
  });

  // Subscribe to the Notify-5 push event so the badge refreshes
  // the instant the scheduler lands new rows — no 30s polling lag.
  useEffect(() => {
    let unlisten: (() => Promise<void>) | undefined;
    listenNotificationsNew(() => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
      queryClient.invalidateQueries({ queryKey: QueryKeys.notifications(25) });
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [queryClient]);

  const unread = unreadQuery.data ?? 0;
  const items = pageQuery.data?.items ?? [];

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          className={cn(
            "text-foreground [&_svg]:size-5! relative mb-1 h-12 transition-all duration-300",
            collapsed
              ? "justify-center rounded-md"
              : "hover:bg-muted/60 justify-start rounded-full px-4",
          )}
          aria-label={
            unread > 0 ? `Notifications, ${unread} unread` : "Notifications"
          }
          title="Notifications"
        >
          <span className="relative">
            <Icons.Bell className="h-5 w-5 opacity-70" />
            {unread > 0 && (
              <span
                className="bg-destructive border-background text-destructive-foreground absolute -right-1.5 -top-1.5 flex h-4 min-w-4 items-center justify-center rounded-full border-2 px-1 text-[10px] font-semibold leading-none tabular-nums"
                aria-hidden="true"
              >
                {unread > 99 ? "99+" : unread}
              </span>
            )}
          </span>
          {!collapsed && (
            <span className="text-muted-foreground ml-2 flex-1 text-left text-sm">
              Notifications
            </span>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        side="right"
        sideOffset={16}
        // Flex column with a fixed max height — the header is fixed,
        // the row list takes the remaining height and scrolls. This
        // guarantees the popover never grows taller than the viewport
        // and the list is always scrollable when content overflows.
        // shadow-2xl + a tighter border = a popover that floats above
        // the dashboard cleanly instead of crashing into it.
        className="flex w-[26rem] max-h-[min(640px,calc(100vh-6rem))] flex-col overflow-hidden rounded-2xl border-border/70 p-0 shadow-2xl"
      >
        <div className="bg-card flex shrink-0 items-center justify-between border-b px-4 py-3.5">
          <div className="flex items-center gap-2">
            <h3 className="text-[15px] font-semibold tracking-tight">
              Notifications
            </h3>
            {unread > 0 && (
              <span
                className="bg-muted text-foreground/80 rounded-full px-2 py-0.5 text-[11px] font-semibold leading-none tabular-nums"
                aria-label={`${unread} unread`}
              >
                {unread > 99 ? "99+" : unread}
              </span>
            )}
          </div>
          {unread > 0 ? (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground h-7 text-xs"
              onClick={() => markAllMutation.mutate()}
              disabled={markAllMutation.isPending}
            >
              {markAllMutation.isPending ? "Marking…" : "Mark all read"}
            </Button>
          ) : (
            <span className="text-muted-foreground text-xs">All caught up</span>
          )}
        </div>
        {/* min-h-0 is the magic — without it a flex child won't shrink
            below its content size, and the scroll area never engages. */}
        <ScrollArea className="min-h-0 flex-1">
          <div className="divide-border divide-y">
            {pageQuery.isLoading ? (
              <PanelSkeleton />
            ) : pageQuery.error ? (
              // Explicit error UI — silently falling back to EmptyState
              // hides a real failure behind "You're all caught up", which
              // is confusing when the badge says e.g. 23 unread. The
              // user needs to know the panel couldn't load and have a
              // retry path.
              <ErrorState
                error={pageQuery.error}
                onRetry={() => void pageQuery.refetch()}
              />
            ) : items.length === 0 ? (
              <EmptyState />
            ) : (
              items.map((n) => (
                <NotificationRow
                  key={n.id}
                  notification={n}
                  onMarkRead={(id) => markReadMutation.mutate(id)}
                  onDismiss={(id) => dismissMutation.mutate(id)}
                  onSelect={() => setOpen(false)}
                />
              ))
            )}
          </div>
        </ScrollArea>
      </PopoverContent>
    </Popover>
  );
}

// ────────────────────────────────────────────────────────────────────
// Row

interface RowProps {
  notification: Notification;
  onMarkRead: (id: string) => void;
  onDismiss: (id: string) => void;
  onSelect: () => void;
}

function NotificationRow({ notification, onMarkRead, onDismiss, onSelect }: RowProps) {
  const navigate = useNavigate();
  const isUnread =
    notification.readAtMs === null && notification.dismissedAtMs === null;
  const handleClick = () => {
    if (isUnread) onMarkRead(notification.id);
    const route = deepLinkToRoute(notification.deepLink);
    if (route) {
      navigate(route);
      onSelect();
    }
  };
  const compactTime = useMemo(
    () => compactRelativeTime(notification.createdAtMs, Date.now()),
    [notification.createdAtMs],
  );
  const { rail } = appearanceFor(notification.kind, notification.severity);
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleClick();
        }
      }}
      className={cn(
        "group focus-visible:bg-muted/70 hover:bg-muted/50 relative flex w-full cursor-pointer items-start gap-3 px-4 py-3.5 outline-none transition-colors",
        // Subtle unread tint — premium iOS/Linear-style "the eye lands
        // here first" cue. The severity-coloured rail (3px, absolutely
        // positioned on the left edge) makes the unread state legible
        // even without colour, since rail width is the signal.
        isUnread && "bg-muted/25",
      )}
    >
      {/* Severity rail — only renders on unread rows. Hairline width
          (3px) so it accents without screaming. */}
      {isUnread && (
        <span
          aria-hidden="true"
          className={cn("absolute inset-y-1.5 left-0 w-[3px] rounded-full", rail)}
        />
      )}
      <KindIcon kind={notification.kind} severity={notification.severity} />
      <div className="min-w-0 flex-1">
        <div className="flex items-start gap-2">
          <p
            className={cn(
              "min-w-0 flex-1 line-clamp-2 text-[13px] leading-snug tracking-tight",
              isUnread ? "font-semibold text-foreground" : "font-medium text-foreground/90",
            )}
          >
            {notification.title}
          </p>
          <span
            className="text-muted-foreground/70 mt-px shrink-0 text-[11px] tabular-nums"
            title={new Date(notification.createdAtMs).toLocaleString()}
          >
            {compactTime}
          </span>
        </div>
        <p className="text-muted-foreground mt-1 line-clamp-2 text-[12px] leading-snug">
          {notification.body}
        </p>
      </div>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onDismiss(notification.id);
        }}
        className="text-muted-foreground/50 hover:text-foreground hover:bg-muted/70 mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
        aria-label="Dismiss"
        title="Dismiss"
      >
        <Icons.X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

/**
 * "now" / "5m" / "3h" / "2d" / "3w" / "Jun 12" — compact relative time
 * for a tight notification row. Falls back to a localised short date
 * when the event is older than 4 weeks, so the user always sees
 * something concrete (not "2 months ago").
 */
function compactRelativeTime(createdAtMs: number, nowMs: number): string {
  const diff = Math.max(0, nowMs - createdAtMs);
  const sec = Math.floor(diff / 1000);
  if (sec < 45) return "now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h`;
  const day = Math.floor(hr / 24);
  if (day < 7) return `${day}d`;
  const wk = Math.floor(day / 7);
  if (wk < 4) return `${wk}w`;
  return new Date(createdAtMs).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

// ────────────────────────────────────────────────────────────────────
// Helpers

// Per-kind appearance: a semantic icon (no more identical bells) plus
// a tone (`success | warning | destructive | gold | neutral`) that
// drives both the icon tint and the row's unread accent rail.
//
// MUST cover every variant in `NotificationKind` (see
// adapters/types-notifications.ts) — but `KindIcon` also falls back to
// a neutral icon + tone at runtime so a backend-only addition can
// never crash the bell again.
type IconComp = typeof Icons.Bell;
type Tone = "success" | "warning" | "destructive" | "gold" | "neutral";

interface KindAppearance {
  Icon: IconComp;
  tone: Tone;
}

const NEUTRAL_APPEARANCE: KindAppearance = {
  Icon: Icons.Bell,
  tone: "neutral",
};

const KIND_APPEARANCE: Record<NotificationKind, KindAppearance> = {
  // Market moves — direction-aware icons
  big_move: { Icon: Icons.TrendingUp, tone: "warning" },
  net_worth_dip: { Icon: Icons.TrendingDown, tone: "destructive" },
  new_ath: { Icon: Icons.TrendingUp, tone: "success" },

  // Targets / drift / risk
  allocation_drift: { Icon: Icons.Target, tone: "neutral" },
  concentration_risk: { Icon: Icons.Shield, tone: "warning" },

  // Goals + milestones
  goal_milestone: { Icon: Icons.Star, tone: "success" },

  // Cash + income
  cash_drag: { Icon: Icons.Wallet, tone: "neutral" },
  cash_drag_opportunity: { Icon: Icons.Sparkles, tone: "success" },
  dividend_posted: { Icon: Icons.HandCoins, tone: "success" },
  tax_optimization_window: { Icon: Icons.BadgeDollarSign, tone: "success" },

  // Time-sensitive
  bond_maturity_approaching: { Icon: Icons.Clock, tone: "warning" },
  fx_moved_materially: { Icon: Icons.ArrowLeftRight, tone: "warning" },

  // Mizan moats (Sharia + Zakat) — gold tone honours them visually
  sharia_status_changed: { Icon: Icons.ShieldAlert, tone: "warning" },
  zakat_hawl_approaching: { Icon: Icons.Moon, tone: "gold" },

  // Infrastructure
  sync_failure: { Icon: Icons.CloudOff, tone: "warning" },
  ai_digest: { Icon: Icons.Sparkles, tone: "neutral" },
};

const TONE_STYLES: Record<Tone, { bg: string; fg: string; rail: string }> = {
  success: {
    bg: "bg-success/10",
    fg: "text-success",
    rail: "bg-success",
  },
  warning: {
    bg: "bg-warning/10",
    fg: "text-warning",
    rail: "bg-warning",
  },
  destructive: {
    bg: "bg-destructive/10",
    fg: "text-destructive",
    rail: "bg-destructive",
  },
  gold: {
    bg: "bg-amber-500/10",
    fg: "text-amber-600 dark:text-amber-400",
    rail: "bg-amber-500",
  },
  neutral: {
    bg: "bg-muted",
    fg: "text-muted-foreground",
    rail: "bg-muted-foreground/40",
  },
};

function appearanceFor(kind: NotificationKind, severity: NotificationSeverity): {
  Icon: IconComp;
  bg: string;
  fg: string;
  rail: string;
} {
  const a = KIND_APPEARANCE[kind] ?? NEUTRAL_APPEARANCE;
  // Critical severity always escalates to destructive, regardless of
  // the per-kind tone — a critical sync failure should look critical.
  const tone = severity === "critical" ? "destructive" : a.tone;
  const t = TONE_STYLES[tone];
  return { Icon: a.Icon, ...t };
}

function KindIcon({ kind, severity }: { kind: NotificationKind; severity: NotificationSeverity }) {
  const { Icon, bg, fg } = appearanceFor(kind, severity);
  return (
    <span
      className={cn(
        "flex h-9 w-9 shrink-0 items-center justify-center rounded-full",
        bg,
      )}
      aria-hidden="true"
    >
      <Icon className={cn("h-4 w-4", fg)} />
    </span>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center gap-2 px-4 py-12 text-center">
      <span className="bg-muted flex h-10 w-10 items-center justify-center rounded-full">
        <Icons.BellOff className="text-muted-foreground h-5 w-5" />
      </span>
      <p className="text-sm font-medium tracking-tight">No notifications</p>
      <p className="text-muted-foreground max-w-[260px] text-xs leading-relaxed">
        Your AI assistant will let you know about big moves, goal milestones,
        and anything else worth your attention.
      </p>
    </div>
  );
}

function ErrorState({ error, onRetry }: { error: Error; onRetry: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-4 py-10 text-center">
      <span className="bg-destructive/10 text-destructive flex h-10 w-10 items-center justify-center rounded-full">
        <Icons.AlertTriangle className="h-5 w-5" />
      </span>
      <div>
        <p className="text-foreground text-sm font-medium">
          Couldn&apos;t load notifications
        </p>
        <p className="text-muted-foreground mt-1 max-w-[260px] text-xs leading-relaxed">
          {error.message || "Something went wrong fetching your notifications."}
        </p>
      </div>
      <Button size="sm" variant="outline" onClick={onRetry}>
        Try again
      </Button>
    </div>
  );
}

function PanelSkeleton() {
  return (
    <div className="space-y-3 px-4 py-4">
      {[0, 1, 2].map((i) => (
        <div key={i} className="flex items-start gap-3">
          <div className="bg-muted h-8 w-8 shrink-0 animate-pulse rounded-full" />
          <div className="flex-1 space-y-2">
            <div className="bg-muted h-3 w-3/4 animate-pulse rounded" />
            <div className="bg-muted h-2 w-full animate-pulse rounded" />
          </div>
        </div>
      ))}
    </div>
  );
}

/**
 * Translate the Rust-side `mizan://...` deep link into the desktop's
 * react-router path. Returns null for links we don't yet recognise
 * (the row still marks itself read, but doesn't navigate).
 */
function deepLinkToRoute(deepLink: string | null): string | null {
  if (!deepLink) return null;
  if (!deepLink.startsWith("mizan://")) return null;
  const rest = deepLink.slice("mizan://".length);
  if (rest === "dashboard") return "/";
  if (rest.startsWith("account/")) return `/accounts/${rest.slice("account/".length)}`;
  if (rest.startsWith("holding/")) return `/holdings/${rest.slice("holding/".length)}`;
  if (rest.startsWith("goal/")) return `/goals/${rest.slice("goal/".length)}`;
  if (rest.startsWith("settings/")) return `/settings/${rest.slice("settings/".length)}`;
  return null;
}
