// Event Listeners
import type {
  EventCallback as TauriEventCallback,
  UnlistenFn as TauriUnlistenFn,
} from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";

import type { EventCallback, UnlistenFn } from "../types";

// Helper to adapt Tauri's event callback to our unified type
const adaptCallback = <T>(handler: EventCallback<T>): TauriEventCallback<T> => {
  return (event) => handler({ event: event.event, payload: event.payload, id: event.id });
};

// Helper to adapt Tauri's unlisten function to our unified type.
//
// Tauri's `UnlistenFn` is typed as `() => void` but the returned function
// actually kicks off an async `invoke()` under the hood. That async work
// can reject *after* the sync call site returns, which meant our sync
// try/catch was letting the rejection escape as an unhandled promise
// error ("Cannot read properties of undefined (reading 'unregisterListener')"
// from Tauri's own event chunk). React strict mode unmount + navigation
// during boot are the two paths that hit this in the wild.
//
// Fix: capture whatever `unlisten()` returns, await it if it looks like a
// promise, and swallow any rejection. Safe because the failure modes are
// all "listener wasn't fully registered / was already removed".
const adaptUnlisten = (unlisten: TauriUnlistenFn): UnlistenFn => {
  return async () => {
    try {
      const maybePromise = unlisten() as unknown;
      if (
        maybePromise &&
        typeof (maybePromise as { then?: unknown }).then === "function"
      ) {
        await (maybePromise as Promise<unknown>).catch(() => {});
      }
    } catch {
      // Listener was never fully registered or already removed — safe to ignore.
    }
  };
};

export const listenFileDropHover = async <T>(handler: EventCallback<T>): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("tauri://file-drop-hover", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenFileDrop = async <T>(handler: EventCallback<T>): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("tauri://file-drop", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenFileDropCancelled = async <T>(
  handler: EventCallback<T>,
): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("tauri://file-drop-cancelled", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenPortfolioUpdateStart = async <T>(
  handler: EventCallback<T>,
): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("portfolio:update-start", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenPortfolioUpdateComplete = async <T>(
  handler: EventCallback<T>,
): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("portfolio:update-complete", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenDatabaseRestored = async <T>(handler: EventCallback<T>): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("database-restored", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export const listenPortfolioUpdateError = async <T>(
  handler: EventCallback<T>,
): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("portfolio:update-error", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

export async function listenMarketSyncComplete<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("market:sync-complete", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenMarketSyncStart<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("market:sync-start", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenMarketSyncError<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("market:sync-error", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenBrokerSyncStart<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("broker:sync-start", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenBrokerSyncComplete<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("broker:sync-complete", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenBrokerSyncError<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("broker:sync-error", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export async function listenNavigateToRoute<T>(handler: EventCallback<T>): Promise<UnlistenFn> {
  const unlisten = await listen<T>("navigate-to-route", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}

export const listenDeepLink = async <T>(handler: EventCallback<T>): Promise<UnlistenFn> => {
  const unlisten = await listen<T>("deep-link-received", adaptCallback(handler));
  return adaptUnlisten(unlisten);
};

/**
 * Notify-5: scheduler emits this whenever new notifications land in the
 * SQLite table so the bell badge can refresh instantly instead of waiting
 * for the next polling tick. Payload = count of new rows in this batch.
 */
export async function listenNotificationsNew<T>(
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  const unlisten = await listen<T>("notifications:new", adaptCallback(handler));
  return adaptUnlisten(unlisten);
}
