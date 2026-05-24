import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

import { AddAssetDialog } from "./add-asset-dialog";

interface AddAssetContextValue {
  /**
   * Open the inline Add dialog. The dialog renders in place — no
   * navigation, no context loss. Two options inside (Ask Mizan AI /
   * Add manually); the AI option expands an inline composer + agent
   * progress card; the manual option routes to /settings/accounts.
   */
  open: (options?: {
    source?: "dashboard" | "portfolio" | "sidebar";
    /** Optional seed text for the AI composer. */
    prompt?: string;
  }) => void;
  /** Programmatic close — most consumers don't need this; the dialog
   *  closes itself on Esc, on backdrop click, or after a successful
   *  agent run. */
  close: () => void;
}

const AddAssetContext = createContext<AddAssetContextValue | null>(null);

/**
 * Provider for the app-wide Add Asset command surface.
 *
 * Old behaviour (deprecated): clicking Add navigated to
 * /assistant?intent=add-asset&prompt=... and the user landed on the
 * full assistant page mid-thought. Jarring + lost wherever-you-were
 * context.
 *
 * New behaviour: clicking Add opens a dialog IN PLACE with two
 * options — "Ask Mizan AI" (inline chat + file drop + agent run)
 * and "Add manually" (links to /settings/accounts). The agent
 * progress card renders inside the same dialog, so the user
 * watches their portfolio get built without ever leaving the
 * surface they started on.
 */
export function AddAssetProvider({ children }: { children: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  const [initialPrompt, setInitialPrompt] = useState<string | undefined>(
    undefined,
  );

  const open = useCallback(
    (options?: { source?: "dashboard" | "portfolio" | "sidebar"; prompt?: string }) => {
      setInitialPrompt(options?.prompt);
      setIsOpen(true);
    },
    [],
  );

  const close = useCallback(() => setIsOpen(false), []);

  const value = useMemo<AddAssetContextValue>(
    () => ({ open, close }),
    [open, close],
  );

  return (
    <AddAssetContext.Provider value={value}>
      {children}
      <AddAssetDialog
        open={isOpen}
        onOpenChange={setIsOpen}
        initialPrompt={initialPrompt}
      />
    </AddAssetContext.Provider>
  );
}

export function useAddAsset(): AddAssetContextValue {
  const ctx = useContext(AddAssetContext);
  if (!ctx) {
    throw new Error("useAddAsset must be used within an AddAssetProvider");
  }
  return ctx;
}
