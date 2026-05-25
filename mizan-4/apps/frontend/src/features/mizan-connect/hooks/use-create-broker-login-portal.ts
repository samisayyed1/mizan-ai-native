import { QueryKeys } from "@/lib/query-keys";
import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaidLinkTokenResponse } from "@/adapters/shared/connect";
import {
  createPlaidLinkToken,
  exchangePlaidPublicToken,
  listBrokerConnections,
  syncBrokerData,
} from "../services/broker-service";
import type { BrokerConnection } from "../types";

const POLL_INTERVAL_MS = 5_000;
const POLL_DURATION_MS = 5 * 60_000;

type PlaidHandler = {
  open: () => void;
  destroy: () => void;
};

type PlaidCreate = (config: {
  token: string;
  onSuccess: (publicToken: string) => void;
  onExit?: (error: unknown) => void;
}) => PlaidHandler;

declare global {
  interface Window {
    Plaid?: { create: PlaidCreate };
  }
}

function loadPlaidScript(): Promise<PlaidCreate> {
  if (window.Plaid?.create) {
    return Promise.resolve(window.Plaid.create);
  }

  return new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(
      'script[src="https://cdn.plaid.com/link/v2/stable/link-initialize.js"]',
    );
    const script =
      existing ?? document.createElement("script");
    script.src = "https://cdn.plaid.com/link/v2/stable/link-initialize.js";
    script.async = true;
    script.onload = () => {
      if (window.Plaid?.create) {
        resolve(window.Plaid.create);
      } else {
        reject(new Error("Plaid Link failed to initialize"));
      }
    };
    script.onerror = () => reject(new Error("Plaid Link script failed to load"));
    if (!existing) {
      document.head.appendChild(script);
    }
  });
}

/**
 * Mints a Plaid Link token, opens Plaid Link, exchanges the returned public
 * token through Mizan Connect, then polls until the new Plaid Item appears.
 *
 * Polling is started internally on mutation success — callers don't need
 * to wire `onSuccess`. Returns `[mutation, isPolling]` so call sites can
 * surface a "Waiting for broker..." spinner.
 *
 * Stops automatically when:
 * - `BROKER_CONNECTIONS` data length grows beyond the snapshot taken at
 *   mutation success (the new connection landed).
 * - 5 minutes elapse from the last successful mutation.
 * - The component hosting the hook unmounts.
 *
 * Each fresh mutation invocation resets the polling window — the user
 * can chain "Connect a broker" calls and each one gets its own 5-minute
 * grace period.
 *
 * The public token is short-lived. The access token never enters frontend
 * state; Mizan Connect exchanges and encrypts it server-side.
 */
export function useCreateBrokerLoginPortal(): readonly [
  UseMutationResult<PlaidLinkTokenResponse, Error, string | undefined, unknown>,
  boolean,
] {
  const queryClient = useQueryClient();
  const [isPolling, setIsPolling] = useState(false);
  const stopTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Connection count at the moment the portal opened. Polling stops as
  // soon as the cached count exceeds this, which means the callback
  // landed and persisted a new authorization.
  const baselineCountRef = useRef(0);

  const stop = useCallback(() => {
    if (stopTimerRef.current !== null) {
      clearTimeout(stopTimerRef.current);
      stopTimerRef.current = null;
    }
    setIsPolling(false);
  }, []);

  const start = useCallback(() => {
    // Reset any in-flight window. Each mutation gets a fresh snapshot +
    // timer so the user can chain connect attempts.
    if (stopTimerRef.current !== null) {
      clearTimeout(stopTimerRef.current);
    }
    const cached = queryClient.getQueryData<BrokerConnection[]>([QueryKeys.BROKER_CONNECTIONS]);
    baselineCountRef.current = cached?.length ?? 0;
    setIsPolling(true);
    stopTimerRef.current = setTimeout(stop, POLL_DURATION_MS);
  }, [queryClient, stop]);

  // While polling is active, keep an observer on BROKER_CONNECTIONS with a
  // 5 s refetch interval. The shared cache means any other consumer
  // (BrokerConnectionsCard, ConnectPage) gets the fresh data for free.
  const polled = useQuery({
    queryKey: [QueryKeys.BROKER_CONNECTIONS],
    queryFn: listBrokerConnections,
    enabled: isPolling,
    refetchInterval: isPolling ? POLL_INTERVAL_MS : false,
    refetchIntervalInBackground: true,
    staleTime: 0,
  });

  // Stop the moment the new connection appears in the cache. Also kick
  // BROKER_ACCOUNTS once so the accounts card refreshes alongside.
  useEffect(() => {
    if (!isPolling) return;
    const length = polled.data?.length ?? 0;
    if (length > baselineCountRef.current) {
      stop();
      void queryClient.invalidateQueries({
        queryKey: [QueryKeys.BROKER_ACCOUNTS],
      });
    }
  }, [isPolling, polled.data, queryClient, stop]);

  // Clean up the timeout on unmount.
  useEffect(() => stop, [stop]);

  const mutation = useMutation<PlaidLinkTokenResponse, Error, string | undefined>({
    mutationFn: async () => createPlaidLinkToken(),
    onSuccess: async ({ linkToken }) => {
      try {
        const create = await loadPlaidScript();
        const handler = create({
          token: linkToken,
          onSuccess: (publicToken) => {
            void exchangePlaidPublicToken(publicToken)
              .then(async () => {
                toast.success("Plaid connection secured", {
                  id: "plaid-link-connected",
                  description: "Syncing your accounts, holdings, and recent transactions…",
                });
                start();
                // Eagerly invalidate both connections AND accounts so the UI
                // doesn't sit at "0 accounts" while polling waits for the new
                // connection to land — the polling effect still re-invalidates
                // accounts once the connection actually appears as a safety
                // net for slow networks.
                void queryClient.invalidateQueries({
                  queryKey: [QueryKeys.BROKER_CONNECTIONS],
                });
                void queryClient.invalidateQueries({
                  queryKey: [QueryKeys.BROKER_ACCOUNTS],
                });
                // Auto-trigger the first sync so the user doesn't have to
                // hunt for a "sync" button after linking. The sync is
                // fire-and-forget — global event listeners (BROKER_SYNC_*)
                // own the toast lifecycle and React Query invalidation when
                // it completes. Wrapped in try/catch so a rate-limit / cooldown
                // error doesn't blow up the link-success path; the next
                // scheduled or user-initiated sync will catch up.
                try {
                  await syncBrokerData();
                } catch (err) {
                  // Logged but not surfaced — the connection itself succeeded.
                  // The user can retry via the Sync button on the connect page.
                  console.warn("Auto-sync after Plaid link skipped:", err);
                }
              })
              .catch((error) => {
                const msg = error instanceof Error ? error.message : "Unknown error";
                toast.error(`Couldn't secure Plaid connection: ${msg}`, {
                  id: "plaid-exchange-error",
                });
              })
              .finally(() => handler.destroy());
          },
          onExit: () => handler.destroy(),
        });
        handler.open();
      } catch (error) {
        const msg = error instanceof Error ? error.message : "Unknown error";
        toast.error(`Couldn't open Plaid Link: ${msg}`, {
          id: "plaid-link-error",
        });
      }
    },
    onError: (error) => {
      const msg = error instanceof Error ? error.message : "Unknown error";
      toast.error(`Couldn't start Plaid connection: ${msg}`, {
        id: "plaid-link-token-error",
      });
    },
  });

  return [mutation, isPolling] as const;
}
