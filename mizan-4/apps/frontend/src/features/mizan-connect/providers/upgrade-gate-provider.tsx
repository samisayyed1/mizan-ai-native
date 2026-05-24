import { openCheckout } from "@/adapters";
import { logger } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@mizan/ui/components/ui/dialog";
import { Icons } from "@mizan/ui/components/ui/icons";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useNavigate } from "react-router-dom";

import { parseGatedError } from "../lib/gated-error";
import { setGatedErrorHandler } from "../lib/gated-error-bus";
import { upgradeCopyFor, type UpgradeCopy } from "../lib/upgrade-copy";

/** Open a URL in the user's default browser. Uses Tauri's shell plugin on
 *  desktop, `window.open` in web. Lazy-imported so the shell-plugin is only
 *  loaded when actually needed (and absent in web builds). */
async function openExternalUrl(url: string): Promise<void> {
  try {
    const mod = await import("@tauri-apps/plugin-shell");
    await mod.open(url);
  } catch {
    // Web fallback — same-tab navigation would lose state, so use a new tab.
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

interface UpgradeGateContextValue {
  /** Open the contextual upgrade modal for a gated feature. */
  requestUpgrade: (feature: string) => void;
  /**
   * If `error` is a backend GatedError, open the matching upgrade modal and
   * return `true` (handled). Otherwise return `false` so the caller can show a
   * normal error. Use in command/mutation `onError` handlers.
   */
  handleGatedError: (error: unknown) => boolean;
}

const UpgradeGateContext = createContext<UpgradeGateContextValue | null>(null);

/**
 * App-level provider exposing a single contextual upgrade modal. Wrap the app
 * once; trigger via `useUpgradeGate().requestUpgrade(feature)` or by funneling
 * caught errors through `handleGatedError`.
 */
export function UpgradeGateProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [copy, setCopy] = useState<UpgradeCopy | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [checkoutOpened, setCheckoutOpened] = useState(false);

  const requestUpgrade = useCallback((feature: string) => {
    setCopy(upgradeCopyFor(feature));
    setOpen(true);
  }, []);

  // Bridge the module-level bus (used by the global mutation-cache error
  // handler, which lives above this provider) to this modal.
  useEffect(() => {
    setGatedErrorHandler(requestUpgrade);
    return () => setGatedErrorHandler(null);
  }, [requestUpgrade]);

  // When the user returns to the app after a Stripe Checkout hop, refresh
  // the entitlements + user queries so the new plan unlocks without a full
  // reload. Only attached while we know they're mid-checkout to avoid
  // hammering the cloud on every focus change.
  useEffect(() => {
    if (!checkoutOpened) return;
    const onFocus = () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.ENTITLEMENTS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.USER_INFO] });
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [checkoutOpened, queryClient]);

  const handleGatedError = useCallback(
    (error: unknown) => {
      const gated = parseGatedError(error);
      if (!gated) return false;
      requestUpgrade(gated.feature);
      return true;
    },
    [requestUpgrade],
  );

  const handleUpgradeCta = useCallback(async () => {
    if (!copy) return;
    setSubmitting(true);
    try {
      const url = await openCheckout(copy.suggestedTier, "monthly");
      setOpen(false);
      setCheckoutOpened(true);
      await openExternalUrl(url);
    } catch (err) {
      // Cloud not reachable / not configured — fall back to the in-app plans
      // page rather than leaving the user stuck on a busy spinner.
      logger.error(`openCheckout failed: ${err instanceof Error ? err.message : String(err)}`);
      setOpen(false);
      navigate("/connect");
    } finally {
      setSubmitting(false);
    }
  }, [copy, navigate]);

  const value = useMemo(
    () => ({ requestUpgrade, handleGatedError }),
    [requestUpgrade, handleGatedError],
  );

  return (
    <UpgradeGateContext.Provider value={value}>
      {children}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <div className="bg-primary/10 text-primary mb-2 flex h-10 w-10 items-center justify-center rounded-full">
              <Icons.Sparkles className="h-5 w-5" />
            </div>
            <DialogTitle>{copy?.title ?? "Upgrade Mizan"}</DialogTitle>
            <DialogDescription className="leading-relaxed">{copy?.body}</DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2 sm:gap-2">
            <Button variant="ghost" onClick={() => setOpen(false)} disabled={submitting}>
              Maybe later
            </Button>
            <Button onClick={handleUpgradeCta} disabled={submitting}>
              {submitting ? "Opening checkout…" : "Upgrade now"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </UpgradeGateContext.Provider>
  );
}

export function useUpgradeGate(): UpgradeGateContextValue {
  const ctx = useContext(UpgradeGateContext);
  if (!ctx) {
    throw new Error("useUpgradeGate must be used within an UpgradeGateProvider");
  }
  return ctx;
}
