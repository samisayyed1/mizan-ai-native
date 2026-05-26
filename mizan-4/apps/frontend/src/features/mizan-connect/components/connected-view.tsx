import { formatDistanceToNowStrict } from "date-fns";

import { ComingSoonCard } from "@/components/coming-soon-card";
import { ExternalLink } from "@/components/external-link";
import { DeviceSyncSection } from "@/features/devices-sync";
import { QueryKeys } from "@/lib/query-keys";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ActionConfirm } from "@mizan/ui";
import { Avatar, AvatarFallback, AvatarImage } from "@mizan/ui/components/ui/avatar";
import { Badge } from "@mizan/ui/components/ui/badge";
import { Button } from "@mizan/ui/components/ui/button";
import { Card, CardContent, CardHeader } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@mizan/ui/components/ui/tooltip";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { formatDate } from "@/lib/utils";
import { useCallback, useMemo, useState } from "react";
import { useCreateBrokerLoginPortal } from "../hooks";
import { useEntitlements } from "../hooks/use-entitlements";
import { useIsBrokerSyncRunning, useSyncBrokerData } from "../hooks/use-sync-broker-data";
import { useMizanConnect } from "../providers/mizan-connect-provider";
import { useUpgradeGate } from "../providers/upgrade-gate-provider";
import {
  deleteBrokerConnection,
  listBrokerAccounts,
  listBrokerConnections,
} from "../services/broker-service";
import { useQueryClient } from "@tanstack/react-query";
import type { BrokerAccount, BrokerConnection } from "../types";
import { hasBrokerSync } from "../lib/plan-capabilities";

// ─────────────────────────────────────────────────────────────────────────────
// Custom Hooks
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Hook to fetch broker connections from the backend.
 */
function useBrokerConnections(isConnected: boolean) {
  return useQuery({
    queryKey: [QueryKeys.BROKER_CONNECTIONS],
    queryFn: listBrokerConnections,
    enabled: isConnected,
    staleTime: 30000,
  });
}

/**
 * Hook to fetch broker accounts from the backend.
 */
function useBrokerAccountsQuery(isConnected: boolean) {
  return useQuery({
    queryKey: [QueryKeys.BROKER_ACCOUNTS],
    queryFn: listBrokerAccounts,
    enabled: isConnected,
    staleTime: 30000,
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Sub-Components
// ─────────────────────────────────────────────────────────────────────────────

interface ServiceUnavailableCardProps {
  onRetry: () => void;
  isRetrying: boolean;
}

function ServiceUnavailableCard({ onRetry, isRetrying }: ServiceUnavailableCardProps) {
  return (
    <Card className="border-warning/30 bg-warning/5">
      <CardContent className="py-8">
        <div className="flex flex-col items-center justify-center text-center">
          <div className="bg-warning/15 mb-4 rounded-full p-4">
            <Icons.CloudOff className="text-warning h-8 w-8" />
          </div>
          <h3 className="text-foreground mb-2 text-base font-medium">
            Service Temporarily Unavailable
          </h3>
          <p className="text-muted-foreground mb-4 max-w-sm text-sm">
            We&apos;re having trouble connecting to Mizan Connect. This is usually temporary and
            should resolve shortly.
          </p>
          <Button variant="outline" onClick={onRetry} disabled={isRetrying}>
            {isRetrying ? (
              <>
                <Icons.Spinner className="h-4 w-4 animate-spin" />
                Retrying...
              </>
            ) : (
              <>
                <Icons.Refresh className="h-4 w-4" />
                Try Again
              </>
            )}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function BrokerConnectionSkeleton() {
  return (
    <div className="flex items-center gap-4 rounded-lg border p-4">
      <Skeleton className="h-12 w-12 rounded-lg" />
      <div className="flex-1 space-y-2">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-3 w-24" />
      </div>
    </div>
  );
}

interface BrokerAccountCardProps {
  account: BrokerAccount;
  connections: BrokerConnection[];
}

/**
 * Mask account number to show only last 4 characters
 */
function maskAccountNumber(number?: string): string {
  if (!number) return "";
  const last4 = number.slice(-4);
  return `\u2022\u2022${last4}`;
}

/**
 * Get the latest sync date from transactions or holdings
 */
function getLastSyncDate(account: BrokerAccount): string | null {
  const txDate = account.sync_status?.transactions?.last_successful_sync;
  const holdingsDate = account.sync_status?.holdings?.last_successful_sync;
  if (txDate && holdingsDate) {
    return new Date(txDate) > new Date(holdingsDate) ? txDate : holdingsDate;
  }
  return txDate || holdingsDate || null;
}

function BrokerAccountCard({ account, connections }: BrokerAccountCardProps) {
  const lastSyncDate = getLastSyncDate(account);

  // Find the connection that matches this account's brokerage_authorization
  const connection = connections.find((c) => c.id === account.brokerage_authorization);
  const logoUrl =
    connection?.brokerage?.aws_s3_square_logo_url ?? connection?.brokerage?.aws_s3_logo_url;

  const lastSyncedText = lastSyncDate ? `Data as of ${formatDate(lastSyncDate)}` : "No data yet";

  return (
    <div className="bg-muted/30 rounded-lg border p-3">
      <div className="flex items-start gap-3">
        {/* Logo */}
        <Avatar className="mt-0.5 h-9 w-9 shrink-0 rounded-lg">
          <AvatarImage
            src={logoUrl}
            alt={account.institution_name || "Broker"}
            className="bg-white object-contain p-1"
          />
          <AvatarFallback className="rounded-lg text-sm font-semibold">
            {(account.institution_name || account.name || "B").charAt(0)}
          </AvatarFallback>
        </Avatar>

        {/* Main info - takes remaining space */}
        <div className="min-w-0 flex-1">
          {/* Row 1: name + badges + sync icon */}
          <div className="flex items-center gap-1.5">
            <span className="truncate text-sm font-medium">{account.name || "Account"}</span>
            {account.is_paper && (
              <Badge variant="outline" className="h-5 shrink-0 text-[10px]">
                Paper
              </Badge>
            )}
            <Tooltip>
              <TooltipTrigger>
                {account.sync_enabled ? (
                  <Icons.Eye className="h-3.5 w-3.5 shrink-0 text-info" />
                ) : (
                  <Icons.EyeOff className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                )}
              </TooltipTrigger>
              <TooltipContent>
                {account.sync_enabled ? "Sync enabled" : "Excluded from sync"}
              </TooltipContent>
            </Tooltip>
          </div>
          {/* Row 2: institution · account number · shared */}
          <div className="text-muted-foreground flex flex-wrap items-center gap-x-1.5 text-xs">
            <span className="truncate">{account.institution_name}</span>
            {account.number && (
              <>
                <span className="shrink-0 opacity-40">·</span>
                <span className="shrink-0">{maskAccountNumber(account.number)}</span>
              </>
            )}
            {account.shared_with_household && (
              <>
                <span className="shrink-0 opacity-40">·</span>
                <span className="flex shrink-0 items-center gap-0.5">
                  <Icons.Users className="h-3 w-3" />
                  Shared
                </span>
              </>
            )}
            {/* Sync time inline on sm+ */}
            <span className="ml-auto hidden shrink-0 pl-2 sm:inline">{lastSyncedText}</span>
          </div>
          {/* Row 3: Sync time on its own line on mobile */}
          <p className="text-muted-foreground mt-0.5 text-[11px] sm:hidden">{lastSyncedText}</p>
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Broker Connections Card Component
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Single row in the BrokerConnectionsCard. Owns its own disconnect
 * mutation + confirm-dialog state so multiple rows can disconnect
 * independently without lifting state into the parent.
 */
function BrokerConnectionRow({ connection }: { connection: BrokerConnection }) {
  const queryClient = useQueryClient();
  // Reconnect flow: when a connection lands in "needs_attention" (Plaid's
  // ITEM_LOGIN_REQUIRED — the broker rotated MFA, password changed, etc.),
  // we re-use the same Link mutation. Plaid Link recognises the existing
  // institution and walks the user through re-authentication. Once the
  // cloud also accepts `itemId` for update-mode link tokens, the UX gets
  // a step shorter — but offering the affordance today is what unblocks
  // the otherwise-silent stale-data failure mode.
  const [linkMutation, isReconnecting] = useCreateBrokerLoginPortal();

  const logoUrl =
    connection.brokerage?.aws_s3_square_logo_url ?? connection.brokerage?.aws_s3_logo_url;
  const brokerageName =
    connection.brokerage?.display_name ?? connection.brokerage?.name ?? "Unknown Broker";
  // Cloud surfaces three terminal states: "connected" (healthy), "needs_attention"
  // (last_error set — typically ITEM_LOGIN_REQUIRED, the broker rotated MFA or
  // the user changed their password), and "disconnected" (user revoked). The
  // healthy branch must reject needs_attention too, otherwise stale numbers
  // ride under a green badge and the user has no idea their balances stopped
  // updating days ago.
  const isNeedsAttention = connection.status === "needs_attention" && !connection.disabled;
  const isConnected =
    connection.status === "connected" && !connection.disabled && !isNeedsAttention;

  // M3.5: surface a relative-time "last synced" hint sourced from the
  // connection's own `updated_at` (the cloud bumps it whenever sync results
  // for this connection are persisted, so it's a faithful proxy for "last
  // synced" without needing to join across sync_states).
  const lastSynced = (() => {
    if (!connection.updated_at) return null;
    try {
      return formatDistanceToNowStrict(new Date(connection.updated_at), { addSuffix: true });
    } catch {
      return null;
    }
  })();

  const disconnect = useMutation({
    mutationFn: () => deleteBrokerConnection(connection.id),
    // Optimistically mark the connection disabled in the cache so the
    // row disappears from the UI the moment the user confirms the
    // disconnect dialog — before the round-trip to /connections
    // completes. The downstream filter in `ConnectedView` (and
    // `ConnectPage`) drops rows where `disabled === true`, so a
    // shallow patch is enough; no need to splice the array.
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: [QueryKeys.BROKER_CONNECTIONS] });
      const previous =
        queryClient.getQueryData<BrokerConnection[]>([QueryKeys.BROKER_CONNECTIONS]) ?? [];
      queryClient.setQueryData<BrokerConnection[]>([QueryKeys.BROKER_CONNECTIONS], (current) =>
        (current ?? []).map((c) =>
          c.id === connection.id
            ? { ...c, disabled: true, disabled_date: new Date().toISOString() }
            : c,
        ),
      );
      return { previous };
    },
    onSuccess: () => {
      // Authoritative refresh — backend may have changed other fields
      // (status, updated_at) we haven't mirrored optimistically. Also
      // refresh the accounts list so accounts tied to this connection
      // drop out of the orphan filter on the next render.
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.BROKER_CONNECTIONS] });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.BROKER_ACCOUNTS] });
      toast.success(`Disconnected ${brokerageName}`, {
        description: "Your historical data is preserved locally.",
      });
    },
    onError: (error, _variables, context) => {
      // Roll back the optimistic disable so the row reappears as it
      // was — otherwise the user sees a "Disconnected" row even though
      // the disconnect actually failed.
      if (context?.previous) {
        queryClient.setQueryData([QueryKeys.BROKER_CONNECTIONS], context.previous);
      }
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Unknown error";
      toast.error(`Couldn't disconnect: ${message}`);
    },
  });

  return (
    <div className="bg-muted/30 flex items-center gap-3 rounded-lg border p-3">
      <Avatar className="h-9 w-9 shrink-0 rounded-lg">
        <AvatarImage src={logoUrl} alt={brokerageName} className="bg-white object-contain p-1" />
        <AvatarFallback className="rounded-lg text-sm font-semibold">
          {brokerageName.charAt(0)}
        </AvatarFallback>
      </Avatar>
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium leading-tight">{brokerageName}</p>
        {isNeedsAttention ? (
          <p className="text-amber-600 dark:text-amber-400 mt-0.5 truncate text-[11px]">
            Reconnect to resume syncing — your broker requires sign-in
          </p>
        ) : lastSynced && isConnected ? (
          <p className="text-muted-foreground mt-0.5 truncate text-[11px]">
            Last synced {lastSynced}
          </p>
        ) : null}
      </div>
      <Badge
        className={`shrink-0 ${
          isConnected
            ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
            : isNeedsAttention
              ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
              : "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400"
        }`}
      >
        {isConnected ? "Connected" : isNeedsAttention ? "Needs reconnect" : "Disconnected"}
      </Badge>
      {isNeedsAttention && (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          className="shrink-0"
          aria-label={`Reconnect ${brokerageName}`}
          disabled={linkMutation.isPending || isReconnecting}
          onClick={() => linkMutation.mutate(undefined)}
        >
          {linkMutation.isPending || isReconnecting ? (
            <Icons.Spinner className="h-4 w-4 animate-spin" />
          ) : (
            "Reconnect"
          )}
        </Button>
      )}
      <ActionConfirm
        confirmTitle={`Disconnect ${brokerageName}?`}
        confirmMessage="Live sync stops immediately. Your already-synced accounts and history stay on this device — nothing is deleted locally. You can reconnect any time."
        confirmButtonText="Disconnect"
        pendingText="Disconnecting…"
        confirmButtonVariant="destructive"
        isPending={disconnect.isPending}
        handleConfirm={() => disconnect.mutate()}
        side="left"
        button={
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="text-muted-foreground hover:text-destructive shrink-0 transition-colors"
            aria-label={`Disconnect ${brokerageName}`}
            disabled={disconnect.isPending}
          >
            {disconnect.isPending ? (
              <Icons.Spinner className="h-4 w-4 animate-spin" />
            ) : (
              <Icons.Trash className="h-4 w-4" />
            )}
          </Button>
        }
      />
    </div>
  );
}

interface BrokerConnectionsCardProps {
  connections: BrokerConnection[];
  isLoading: boolean;
  onRefresh: () => void;
  isRefreshing: boolean;
}

function BrokerConnectionsCard({
  connections,
  isLoading,
  onRefresh,
  isRefreshing,
}: BrokerConnectionsCardProps) {
  // Plaid Link is the same call regardless of which
  // button (header "Manage", empty-state "Connect with Plaid") triggers it —
  // every entry point goes through useCreateBrokerLoginPortal so we get
  // the polling refetch and consistent error toasts for free.
  const [portal, isPolling] = useCreateBrokerLoginPortal();
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();

  // Connection cap: Gold = unlimited (-1). The cloud
  // enforces this server-side too; the UI gate is just to avoid a round-
  // trip when the user is already at the cap.
  const cap = entitlements.maxBrokerConnections;
  const isUnlimited = cap < 0;
  const used = connections.length;
  const atCap = !isUnlimited && used >= cap;
  const capLabel = isUnlimited ? null : `${used} / ${cap} used`;

  const handleConnect = () => {
    if (atCap) {
      requestUpgrade("max_broker_connections");
      return;
    }
    portal.mutate(undefined);
  };
  const isPortalBusy = portal.isPending || isPolling;
  const portalLabel = portal.isPending
    ? "Opening portal..."
    : isPolling
      ? "Waiting for broker..."
      : null;

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <div className="bg-muted flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
              <Icons.Link className="text-muted-foreground h-4 w-4" />
            </div>
            <h3 className="text-base font-semibold">Broker connections</h3>
            {capLabel && (
              <Badge variant={atCap ? "destructive" : "secondary"} className="ml-1 text-[10px]">
                {capLabel}
              </Badge>
            )}
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="text-muted-foreground hover:text-foreground h-8 w-8"
              onClick={onRefresh}
              disabled={isRefreshing}
              aria-label="Refresh"
            >
              <Icons.RefreshCw className={`h-4 w-4 ${isRefreshing ? "animate-spin" : ""}`} />
            </Button>
            {/* Mobile: icon only */}
            <Button
              variant="ghost"
              size="icon"
              className="text-muted-foreground hover:text-foreground sm:hidden"
              onClick={handleConnect}
              disabled={isPortalBusy}
              aria-label={atCap ? "Upgrade for more broker connections" : "Add broker"}
            >
              {isPortalBusy ? (
                <Icons.Spinner className="h-4 w-4 animate-spin" />
              ) : atCap ? (
                <Icons.Sparkles className="h-4 w-4" />
              ) : (
                <Icons.Plus className="h-4 w-4" />
              )}
            </Button>
            {/* Desktop: full text */}
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground hidden sm:inline-flex"
              onClick={handleConnect}
              disabled={isPortalBusy}
            >
              {isPortalBusy ? (
                <>
                  <Icons.Spinner className="mr-1 h-4 w-4 animate-spin" />
                  {portalLabel ?? "Connecting..."}
                </>
              ) : atCap ? (
                <>
                  <Icons.Sparkles className="mr-1 h-3.5 w-3.5" />
                  Upgrade for more
                </>
              ) : (
                <>
                  Add broker
                  <Icons.ArrowRight className="ml-1 h-3.5 w-3.5" />
                </>
              )}
            </Button>
          </div>
        </div>

        <div className="mt-4 space-y-2">
          {isLoading ? (
            <>
              <BrokerConnectionSkeleton />
              <BrokerConnectionSkeleton />
            </>
          ) : connections.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-6 text-center">
              <p className="text-muted-foreground text-sm">No broker connections yet</p>
              <Button className="mt-3" onClick={handleConnect} disabled={isPortalBusy}>
                {isPortalBusy ? (
                  <>
                    <Icons.Spinner className="h-4 w-4 animate-spin" />
                    {portalLabel ?? "Connecting..."}
                  </>
                ) : (
                  <>
                    <Icons.Plus className="h-4 w-4" />
                    Connect a broker
                  </>
                )}
              </Button>
            </div>
          ) : (
            connections.map((connection) => (
              <BrokerConnectionRow key={connection.id} connection={connection} />
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Component
// ─────────────────────────────────────────────────────────────────────────────

export function ConnectedView() {
  const {
    user,
    userInfo,
    signOut,
    isLoading,
    isLoadingUserInfo,
    error,
    clearError,
    refetchUserInfo,
  } = useMizanConnect();
  const { requestUpgrade } = useUpgradeGate();
  const [isSigningOut, setIsSigningOut] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);

  // Check if there's a service unavailable error (failed to fetch user info)
  const isServiceUnavailable = !!error && !isLoadingUserInfo && !userInfo;

  // Gold-tier Plaid sync gating: only show the broker UI when the user's
  // team plan + subscription unlocks `plaidSync`. Silver users see the
  // upgrade-CTA empty state from the parent ConnectPage.
  const hasSubscription = hasBrokerSync(userInfo);
  const showBrokerSync = !!userInfo && hasSubscription;

  // Hooks - only fetch broker connections and accounts if user has broker sync
  const connectionsQuery = useBrokerConnections(showBrokerSync);
  const accountsQuery = useBrokerAccountsQuery(showBrokerSync);

  // Retry handler for service unavailable state
  const handleRetry = useCallback(async () => {
    setIsRetrying(true);
    clearError();
    try {
      await refetchUserInfo();
    } finally {
      setIsRetrying(false);
    }
  }, [clearError, refetchUserInfo]);

  // Sync accounts to local database. Sync runs in background, global
  // event listener handles toasts and query invalidation.
  //
  // Previously this component declared its own inline `useMutation` for
  // syncBrokerData, separate from the SyncButton's `useSyncBrokerData`
  // hook. Two buttons → two mutations → two parallel /sync commands
  // when the user clicked them in quick succession. Now both share
  // the same hook (and `BROKER_SYNC_MUTATION_KEY`), and the global
  // `useIsBrokerSyncRunning()` gate below disables every sync
  // trigger in the tree while one is in flight.
  const syncToLocalMutation = useSyncBrokerData();
  const isBrokerSyncRunning = useIsBrokerSyncRunning();

  // Handlers
  const handleSignOut = useCallback(async () => {
    setIsSigningOut(true);
    clearError();
    try {
      await signOut();
    } catch {
      // Error is handled by context
    } finally {
      setIsSigningOut(false);
    }
  }, [clearError, signOut]);

  // Derived state. The backend soft-deletes broker connections (sets
  // `disabled = true` + `disabled_date`) rather than removing rows, so
  // a disconnect leaves the row in the API response. Filter here so
  // disconnected brokers — and any accounts tied to them — disappear
  // from the UI the moment the disconnect mutation invalidates the
  // queries, matching what the user expects when they click the trash
  // icon. The active-id set is the single source of truth for both
  // the connections card and the accounts card below.
  const allConnections = connectionsQuery.data ?? [];
  const allBrokerAccounts = accountsQuery.data ?? [];
  const { connections, brokerAccounts } = useMemo(() => {
    const activeConnections = allConnections.filter((c) => !c.disabled && !c.disabled_date);
    const activeIds = new Set(activeConnections.map((c) => c.id));
    return {
      connections: activeConnections,
      brokerAccounts: allBrokerAccounts.filter((a) =>
        // Keep accounts whose authorization still points at a live
        // connection. Defensive: orphan accounts (authorization missing
        // or pointing at a deleted connection) are dropped too so the
        // user never sees a card they can't act on.
        a.brokerage_authorization ? activeIds.has(a.brokerage_authorization) : false,
      ),
    };
  }, [allConnections, allBrokerAccounts]);
  const isLoadingConnections = connectionsQuery.isLoading;
  const isLoadingAccounts = accountsQuery.isLoading;
  // Use the global "any broker-sync mutation in flight" gate so the
  // Sync Now button below disables even when the in-flight sync was
  // triggered from the header SyncButton (or any other component).
  // `syncToLocalMutation.isPending` would only catch syncs started
  // through *this* component's call to `mutate()`.
  const isSyncing = isBrokerSyncRunning || syncToLocalMutation.isPending;

  return (
    <div className="space-y-6">
      {/* Account Card */}
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center gap-3">
            <Avatar className="h-12 w-12">
              <AvatarImage
                src={user?.user_metadata?.avatar_url}
                alt={user?.email ?? "User avatar"}
              />
              <AvatarFallback className="bg-success/15">
                <span className="text-success text-lg font-semibold">
                  {(userInfo?.full_name?.[0] ?? user?.email?.[0] ?? "U").toUpperCase()}
                </span>
              </AvatarFallback>
            </Avatar>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <h3 className="truncate text-base font-semibold">
                  {userInfo?.full_name ?? user?.email?.split("@")[0] ?? "User"}
                </h3>
                {hasSubscription && (
                  <Badge className="h-5 shrink-0 bg-green-100 px-2 text-[10px] font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                    Active
                  </Badge>
                )}
              </div>
              <p className="text-muted-foreground truncate text-sm">{user?.email}</p>
            </div>
            <ActionConfirm
              handleConfirm={handleSignOut}
              isPending={isSigningOut}
              confirmTitle="Sign out of Mizan Connect?"
              confirmMessage="You'll need to sign in again to access your synced broker accounts. Your local data will not be affected."
              confirmButtonText="Sign Out"
              pendingText="Signing out..."
              cancelButtonText="Cancel"
              confirmButtonVariant="destructive"
              button={
                <Button
                  variant="outline"
                  size="sm"
                  className="text-muted-foreground hover:text-foreground shrink-0"
                  disabled={isSigningOut || isLoading}
                >
                  {isSigningOut ? (
                    <Icons.Spinner className="h-4 w-4 animate-spin" />
                  ) : (
                    <Icons.LogOut className="h-4 w-4" />
                  )}
                  <span>Sign out</span>
                </Button>
              }
            />
          </div>
        </CardContent>
      </Card>

      {/* Show loading skeleton only during initial load (not refreshes) */}
      {!isServiceUnavailable && !userInfo && (
        <Card>
          <CardHeader>
            <Skeleton className="h-5 w-48" />
            <Skeleton className="mt-2 h-4 w-72" />
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <Skeleton className="h-20 w-full rounded-lg" />
              <Skeleton className="h-20 w-full rounded-lg" />
            </div>
          </CardContent>
        </Card>
      )}

      {/* Show Service Unavailable card when API is down */}
      {isServiceUnavailable && (
        <ServiceUnavailableCard onRetry={handleRetry} isRetrying={isRetrying} />
      )}

      {/* Subscription plans backend (/api/v1/subscription/plans) is not in
          Chunk 1 of Mizan Connect. Replaces upstream's <SubscriptionPlans/>
          render with a quiet placeholder until billing ships in Chunk 2. */}
      {!isServiceUnavailable && !hasSubscription && !!userInfo && (
        <ComingSoonCard
          title="Plans & billing"
          message="Subscriptions and plan upgrades arrive with the next Mizan Connect release."
          detail="You're signed in — that's all that's needed for Chunk 2."
        />
      )}

      {/* Brokerage-sync ComingSoonCard removed in Chunk 3 — the real
          BrokerConnectionsCard + Accounts card render below. */}

      {/* Device sync (E2E-encrypted multi-device) follows brokerage. Hidden
          until the encrypted-snapshot service ships. */}
      {!isServiceUnavailable && !!userInfo && (
        <ComingSoonCard
          title="Device sync"
          message="Keep Mizan in sync across your devices with end-to-end encryption."
          detail="Available in a future Mizan Connect release."
        />
      )}

      {/* Broker Connections Card - Only show if user has broker sync */}
      {showBrokerSync && (
        <BrokerConnectionsCard
          connections={connections}
          isLoading={isLoadingConnections}
          onRefresh={() => connectionsQuery.refetch()}
          isRefreshing={connectionsQuery.isFetching}
        />
      )}

      {/* Accounts Card - Only show if user has broker sync */}
      {showBrokerSync && (
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <div className="bg-muted flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
                  <Icons.Wallet className="text-muted-foreground h-4 w-4" />
                </div>
                <h3 className="text-base font-semibold">Accounts</h3>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => syncToLocalMutation.mutate()}
                  disabled={isSyncing || accountsQuery.isFetching}
                >
                  <Icons.CloudSync2 className={`h-4 w-4 ${isSyncing ? "animate-pulse" : ""}`} />
                  Sync Now
                </Button>
                {/* TODO(chunk-5): in-app per-account management UI.
                    The "Manage accounts" buttons used to deep-link into a
                    hosted Mizan Connect portal at MIZAN_CONNECT_PORTAL_URL
                    (currently a parked domain), so they're hidden until we
                    surface a real in-app screen. Adding a broker is still
                    available via the BrokerConnectionsCard "Add broker"
                    header above. */}
              </div>
            </div>

            <div className="mt-4">
              {isLoadingAccounts || isLoadingConnections ? (
                <div className="space-y-2">
                  <BrokerConnectionSkeleton />
                  <BrokerConnectionSkeleton />
                </div>
              ) : brokerAccounts.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-6 text-center">
                  <p className="text-muted-foreground text-sm">No accounts synced yet</p>
                  <p className="text-muted-foreground mt-1 max-w-xs text-xs">
                    Connect a broker to start syncing your accounts.
                  </p>
                </div>
              ) : (
                <div className="space-y-2">
                  {brokerAccounts.map((account) => (
                    <BrokerAccountCard
                      key={account.id}
                      account={account}
                      connections={connections}
                    />
                  ))}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Device Sync Section - Only show if user has an active subscription */}
      {hasSubscription && <DeviceSyncSection />}

      {/* Upgrade callout for Silver users */}
      {hasSubscription && !showBrokerSync && (
        <Card className="border-primary/20 bg-primary/5">
          <CardContent className="p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-medium">Upgrade to sync broker accounts</h3>
                <p className="text-muted-foreground mt-0.5 text-xs">
                  Connect your brokerage accounts for automatic portfolio syncing.
                </p>
              </div>
              {/* Routes through the contextual upgrade-gate modal, which
                  calls openCheckout(plan, interval) on confirm and opens
                  the Stripe-hosted Checkout in the user's default
                  browser. Same code path the dashboard's broker-sync
                  gated-error handler uses, so users get a consistent
                  CTA wherever they hit the Gold paywall. */}
              <Button size="sm" onClick={() => requestUpgrade("broker_sync")}>
                Upgrade
                <Icons.ArrowRight className="ml-1 h-3.5 w-3.5" />
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Privacy Footnote */}
      <footer className="border-t pt-4">
        <p className="text-muted-foreground text-center text-xs leading-relaxed">
          Mizan Connect doesn&apos;t store your brokerage credentials or financial data. Everything
          syncs securely via an aggregator to your local database. Device sync uses end-to-end
          encryption.{" "}
          <ExternalLink
            href="https://mizan.app"
            className="text-muted-foreground hover:text-foreground underline underline-offset-2"
          >
            Learn more
          </ExternalLink>
        </p>
      </footer>
    </div>
  );
}
