import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";

interface AddAssetContextValue {
  /**
   * Open the inline Add dialog. The dialog renders in place — no
   * navigation, no context loss. Two options inside (Ask Mizan AI /
   * Add manually); the AI option expands an inline composer + agent
   * progress card; the manual option routes to /activities/manage with
   * the account pre-selected when called from an account context.
   */
  open: (options?: {
    source?: "dashboard" | "portfolio" | "sidebar";
    /** Optional seed text for the AI composer. */
    prompt?: string;
    /**
     * When the dialog is opened from an account-scoped surface
     * (e.g. the account-detail page's "Add stock" button), this is the
     * account the new asset/activity should belong to. Passed straight
     * through to the manual form so the user is NOT asked to pick a
     * portfolio again — the very repetition the user flagged as broken.
     */
    accountId?: string;
    /** Display name of the pre-selected account (shown in the AI tab). */
    accountName?: string;
  }) => void;
  /** Programmatic close — most consumers don't need this; the dialog
   *  closes itself on Esc, on backdrop click, or after a successful
   *  agent run. */
  close: () => void;
}

const AddAssetContext = createContext<AddAssetContextValue | null>(null);

/**
 * Provider for the app-wide Add command surface.
 *
 * Current behaviour: clicking Add navigates straight to
 * `/assistant?intent=add-asset&prompt=…`. The user lands in the
 * standalone AI page where the full chat history, the full tool
 * surface (research_asset, create_account, add_alternative_asset,
 * record_activity, …), and the streaming runtime all live. One
 * coherent place to plan, mutate, and verify.
 *
 * Previous iteration (deprecated 2026-06-21) was an inline modal
 * with two options ("Ask Mizan AI" / "Add manually"). It split the
 * mental model — the modal said "describe it or drop a file" but
 * the actual write tools live on /assistant, so the user ended up
 * bounced anyway. Routing directly removes the extra step + keeps
 * the assistant as the single source of truth for any state
 * mutation.
 */
export function AddAssetProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();

  const open = useCallback(
    (options?: {
      source?: "dashboard" | "portfolio" | "sidebar";
      prompt?: string;
      accountId?: string;
      accountName?: string;
    }) => {
      // Build the search params so the assistant page can pre-fill its
      // composer + scope the agent's tool calls to the right account.
      const params = new URLSearchParams({ intent: "add-asset" });
      if (options?.prompt) params.set("prompt", options.prompt);
      if (options?.accountId) params.set("accountId", options.accountId);
      if (options?.accountName) params.set("accountName", options.accountName);
      navigate(`/assistant?${params.toString()}`);
    },
    [navigate],
  );

  const close = useCallback(() => {
    // No modal anymore — close is a no-op kept on the context for
    // backwards-compat with callers that opened-then-closed
    // programmatically. Safe to delete once those callers migrate.
  }, []);

  const value = useMemo<AddAssetContextValue>(
    () => ({ open, close }),
    [open, close],
  );

  return <AddAssetContext.Provider value={value}>{children}</AddAssetContext.Provider>;
}

export function useAddAsset(): AddAssetContextValue {
  const ctx = useContext(AddAssetContext);
  if (!ctx) {
    throw new Error("useAddAsset must be used within an AddAssetProvider");
  }
  return ctx;
}
