import { useIsMutating, useMutation } from "@tanstack/react-query";
import { syncBrokerData } from "../services/broker-service";
import { toast } from "@mizan/ui/components/ui/use-toast";

/**
 * Shared mutation key for the broker-sync command. Tagging the
 * mutation with this key lets *any* component (the header SyncButton,
 * the "Sync Now" button inside ConnectedView, a future bulk-action
 * surface, etc.) check `useIsBrokerSyncRunning()` and avoid firing a
 * second sync while one is already in flight. The race used to
 * produce duplicate /sync commands on the wire when the user
 * spammed two different buttons.
 */
export const BROKER_SYNC_MUTATION_KEY = ["mizan-connect", "sync-broker-data"] as const;

/**
 * Tauri's `invoke` rejects with the inner `String` for `Result<_, String>`
 * commands — i.e. the rejection is a plain JS string, NOT an `Error`
 * instance. This helper extracts a human-readable message regardless of
 * whether the caller threw an `Error`, a string, or some other shape.
 */
function extractErrorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err && typeof err === "object") {
    const maybe = err as { message?: unknown };
    if (typeof maybe.message === "string") return maybe.message;
    try {
      return JSON.stringify(err);
    } catch {
      // fall through
    }
  }
  return "Unknown error";
}

/**
 * Hook to trigger broker data sync.
 * The actual sync runs in the background and results are handled via
 * global event listeners (SSE events trigger toasts and query invalidation).
 *
 * The mutation is tagged with `BROKER_SYNC_MUTATION_KEY` so any
 * component anywhere in the tree can call `useIsBrokerSyncRunning()`
 * and disable its sync trigger while a sync is in flight — even when
 * the trigger came from a different component.
 */
export function useSyncBrokerData() {
  return useMutation({
    mutationKey: [...BROKER_SYNC_MUTATION_KEY],
    mutationFn: syncBrokerData,
    onSuccess: () => {
      toast.loading("Syncing broker data...", { id: "broker-sync-start" });
    },
    onError: (error) => {
      toast.error(`Failed to start sync: ${extractErrorMessage(error)}`);
    },
  });
}

/**
 * True when *any* broker-sync mutation is currently in flight,
 * regardless of which component triggered it. Use this in sync
 * buttons to disable the trigger and avoid firing a duplicate
 * command on the wire.
 */
export function useIsBrokerSyncRunning(): boolean {
  return useIsMutating({ mutationKey: [...BROKER_SYNC_MUTATION_KEY] }) > 0;
}
