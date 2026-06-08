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
import { formatDistanceToNowStrict } from "date-fns";
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

  const markReadMutation = useMutation({
    mutationFn: (id: string) => markNotificationRead(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
      queryClient.invalidateQueries({ queryKey: QueryKeys.notifications(25) });
    },
  });

  const dismissMutation = useMutation({
    mutationFn: (id: string) => dismissNotification(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
      queryClient.invalidateQueries({ queryKey: QueryKeys.notifications(25) });
    },
  });

  const markAllMutation = useMutation({
    mutationFn: markAllNotificationsRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.NOTIFICATIONS_UNREAD_COUNT] });
      queryClient.invalidateQueries({ queryKey: QueryKeys.notifications(25) });
    },
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
        sideOffset={12}
        className="w-96 p-0"
      >
        <div className="flex items-center justify-between border-b px-4 py-3">
          <div>
            <h3 className="text-sm font-semibold tracking-tight">Notifications</h3>
            <p className="text-muted-foreground text-xs">
              {unread === 0
                ? "You're all caught up."
                : `${unread} unread`}
            </p>
          </div>
          {unread > 0 && (
            <Button
              variant="ghost"
              size="sm"
              className="text-xs"
              onClick={() => markAllMutation.mutate()}
              disabled={markAllMutation.isPending}
            >
              Mark all read
            </Button>
          )}
        </div>
        <ScrollArea className="max-h-[60vh]">
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
  const relative = useMemo(
    () =>
      formatDistanceToNowStrict(new Date(notification.createdAtMs), {
        addSuffix: true,
      }),
    [notification.createdAtMs],
  );
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
        "group focus-visible:bg-muted/70 hover:bg-muted/50 flex w-full cursor-pointer items-start gap-3 px-4 py-3 outline-none transition-colors",
        isUnread && "bg-muted/30",
      )}
    >
      <KindIcon kind={notification.kind} severity={notification.severity} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <p className="truncate text-sm font-medium tracking-tight">
            {notification.title}
          </p>
          {isUnread && (
            <span
              className="bg-primary inline-block h-1.5 w-1.5 shrink-0 rounded-full"
              aria-label="Unread"
            />
          )}
        </div>
        <p className="text-muted-foreground line-clamp-2 text-xs">
          {notification.body}
        </p>
        <p className="text-muted-foreground/70 mt-1 text-[10px] uppercase tracking-wide">
          {relative}
        </p>
      </div>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onDismiss(notification.id);
        }}
        className="text-muted-foreground/60 hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
        aria-label="Dismiss"
        title="Dismiss"
      >
        <Icons.X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// Helpers

// Semantic palette mapping. Where Flexoki has a named token (success /
// destructive / warning), we use it so dark + light mode flip cleanly
// without explicit dark: variants. The two kinds without a semantic
// match (allocation_drift, ai_digest) keep their Tailwind palettes but
// pair light + dark variants so contrast is right in both themes.
const KIND_PALETTE: Record<NotificationKind, { bg: string; fg: string }> = {
  big_move: { bg: "bg-warning/10", fg: "text-warning" },
  allocation_drift: { bg: "bg-blue-500/10", fg: "text-blue-600 dark:text-blue-400" },
  goal_milestone: { bg: "bg-success/10", fg: "text-success" },
  cash_drag: { bg: "bg-muted", fg: "text-muted-foreground" },
  dividend_posted: { bg: "bg-success/10", fg: "text-success" },
  new_ath: { bg: "bg-success/10", fg: "text-success" },
  net_worth_dip: { bg: "bg-destructive/10", fg: "text-destructive" },
  sync_failure: { bg: "bg-warning/10", fg: "text-warning" },
  ai_digest: { bg: "bg-violet-500/10", fg: "text-violet-600 dark:text-violet-400" },
};

function KindIcon({ kind, severity }: { kind: NotificationKind; severity: NotificationSeverity }) {
  const palette = KIND_PALETTE[kind];
  // Critical severity always uses the destructive palette to override
  // the per-kind colour — a critical sync failure should look critical.
  const useCritical = severity === "critical";
  return (
    <span
      className={cn(
        "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
        useCritical ? "bg-destructive/10" : palette.bg,
      )}
      aria-hidden="true"
    >
      <Icons.Bell
        className={cn(
          "h-4 w-4",
          useCritical ? "text-destructive" : palette.fg,
        )}
      />
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
