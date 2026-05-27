// SnapTrade brokerage connections card — sits next to the Plaid
// `BrokerConnectionsCard`. Both feed into the same downstream
// holdings/activities pipeline; we keep the UI surfaces separate so
// the user can tell which integration owns which connection (and so
// each card can degrade independently if its cloud side is missing).

import {
  createSnapTradeLoginPortal,
  disconnectSnapTradeAuthorization,
  listSnapTradeConnections,
  openUrlInBrowser,
  snaptradeHealth,
  snaptradeSyncNow,
} from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNowStrict } from "date-fns";
import { useState } from "react";
import { toast } from "sonner";

export function SnapTradeConnectionsCard() {
  const queryClient = useQueryClient();
  const [openingPortal, setOpeningPortal] = useState(false);
  const [syncing, setSyncing] = useState(false);

  // Health gate — never render the connect button if the cloud isn't
  // configured for SnapTrade. The Plaid card has the same pattern.
  const healthQuery = useQuery({
    queryKey: [QueryKeys.SNAPTRADE_HEALTH ?? "snaptradeHealth"],
    queryFn: snaptradeHealth,
    staleTime: 5 * 60_000,
    retry: false,
  });

  const connectionsQuery = useQuery({
    queryKey: [QueryKeys.SNAPTRADE_CONNECTIONS ?? "snaptradeConnections"],
    queryFn: listSnapTradeConnections,
    enabled: healthQuery.data?.configured === true,
    staleTime: 60_000,
  });

  const disconnectMutation = useMutation({
    mutationFn: (authorizationId: string) =>
      disconnectSnapTradeAuthorization(authorizationId),
    onSuccess: () => {
      toast.success("Brokerage disconnected");
      queryClient.invalidateQueries({
        queryKey: [QueryKeys.SNAPTRADE_CONNECTIONS ?? "snaptradeConnections"],
      });
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : "Failed to disconnect";
      toast.error(msg);
    },
  });

  const handleConnect = async () => {
    setOpeningPortal(true);
    try {
      const portal = await createSnapTradeLoginPortal();
      await openUrlInBrowser(portal.redirectUri);
      toast.info(
        "Complete the SnapTrade flow in your browser. We'll pick up the new connection when you return.",
      );
      // Refresh once the user comes back — SnapTrade's portal usually
      // posts the authorization within a few seconds.
      const onFocus = () => {
        queryClient.invalidateQueries({
          queryKey: [QueryKeys.SNAPTRADE_CONNECTIONS ?? "snaptradeConnections"],
        });
        window.removeEventListener("focus", onFocus);
      };
      window.addEventListener("focus", onFocus);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Could not open SnapTrade";
      toast.error(msg);
    } finally {
      setOpeningPortal(false);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      const summary = await snaptradeSyncNow();
      toast.success(
        `Synced ${summary.accountsSynced} account(s) · ${summary.positionsSynced} position(s) · ${summary.activitiesSynced} activity record(s)`,
      );
      queryClient.invalidateQueries({
        queryKey: [QueryKeys.SNAPTRADE_CONNECTIONS ?? "snaptradeConnections"],
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Sync failed";
      toast.error(msg);
    } finally {
      setSyncing(false);
    }
  };

  // When the cloud isn't configured for SnapTrade we still render the
  // card so the user knows the feature exists — just disabled with a
  // hint. Better UX than silently hiding.
  const configured = healthQuery.data?.configured === true;
  const connections = connectionsQuery.data ?? [];

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div>
            <CardTitle>Brokerages (SnapTrade)</CardTitle>
            <CardDescription>
              Connect via SnapTrade to sync stocks, funds, and crypto from any
              of 70+ brokerages worldwide. Works alongside Plaid.
              {healthQuery.data?.environment === "sandbox" && (
                <span className="text-muted-foreground ml-2 rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-700 dark:text-amber-400">
                  Sandbox
                </span>
              )}
            </CardDescription>
          </div>
          {configured && connections.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={handleSync}
              disabled={syncing}
            >
              {syncing ? (
                <Icons.Spinner className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Icons.Refresh className="h-3.5 w-3.5" />
              )}
              <span className="ml-1.5">Sync now</span>
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {!configured ? (
          <div className="text-muted-foreground rounded-md border border-dashed p-4 text-sm">
            SnapTrade is not yet configured on this Mizan Connect deployment.
            Operators can flip it on by setting <code>SNAPTRADE_CLIENT_ID</code>,
            <code>SNAPTRADE_CONSUMER_KEY</code>, and
            <code>MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY</code>.
          </div>
        ) : connections.length === 0 ? (
          <div className="flex flex-col items-start gap-3">
            <p className="text-muted-foreground text-sm">
              No brokerage connected yet. Connecting opens SnapTrade in your
              system browser; you'll be returned to Mizan when you finish.
            </p>
            <Button
              type="button"
              size="lg"
              onClick={handleConnect}
              disabled={openingPortal}
              className="from-primary to-primary/90 bg-linear-to-r"
            >
              {openingPortal ? (
                <>
                  <Icons.Spinner className="h-4 w-4 animate-spin" />
                  Opening SnapTrade…
                </>
              ) : (
                <>
                  <Icons.Plus className="h-4 w-4" />
                  Connect a brokerage
                </>
              )}
            </Button>
          </div>
        ) : (
          <div className="divide-border divide-y">
            {connections.map((c) => (
              <div
                key={c.authorizationId}
                className="flex items-center justify-between py-3"
              >
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-semibold tracking-tight">
                    {c.displayName ?? c.brokerageName}
                  </p>
                  <p className="text-muted-foreground text-xs">
                    {c.disabled
                      ? "Disabled — reconnect required"
                      : c.connectedAtMs
                        ? `Connected ${formatDistanceToNowStrict(new Date(c.connectedAtMs), { addSuffix: true })}`
                        : "Connected"}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => disconnectMutation.mutate(c.authorizationId)}
                  disabled={disconnectMutation.isPending}
                  aria-label="Disconnect"
                  title="Disconnect"
                >
                  <Icons.Trash className="text-muted-foreground h-4 w-4" />
                </Button>
              </div>
            ))}
            <div className="pt-3">
              <Button
                variant="outline"
                size="sm"
                onClick={handleConnect}
                disabled={openingPortal}
              >
                {openingPortal ? (
                  <Icons.Spinner className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Icons.Plus className="h-3.5 w-3.5" />
                )}
                <span className="ml-1.5">Connect another brokerage</span>
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
