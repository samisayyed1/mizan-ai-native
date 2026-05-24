import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";

interface AddAssetContextValue {
  /** Route into the assistant-led asset ingestion surface. */
  open: (options?: { source?: "dashboard" | "portfolio" | "sidebar"; prompt?: string }) => void;
}

const AddAssetContext = createContext<AddAssetContextValue | null>(null);

/**
 * Provider kept as the stable app-wide Add Asset command surface. The old
 * manual wizard is no longer the product path; every Add entry point opens the
 * Assistant with an explicit add-asset intent so state-changing writes can flow
 * through structured actions, deterministic validation, and user review.
 */
export function AddAssetProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const open = useCallback((options?: { source?: "dashboard" | "portfolio" | "sidebar"; prompt?: string }) => {
    const prompt =
      options?.prompt ??
      "Tell me what you want to add — for example: 15.5 oz gold, a rental property in Toronto, my RRSP statement, or a CSV from my broker.";
    const params = new URLSearchParams({
      intent: "add-asset",
      prompt,
    });
    if (options?.source) {
      params.set("source", options.source);
    }
    navigate(`/assistant?${params.toString()}`);
  }, [navigate]);

  const value = useMemo<AddAssetContextValue>(() => ({ open }), [open]);

  return <AddAssetContext.Provider value={value}>{children}</AddAssetContext.Provider>;
}

export function useAddAsset(): AddAssetContextValue {
  const ctx = useContext(AddAssetContext);
  if (!ctx) {
    throw new Error("useAddAsset must be used within an AddAssetProvider");
  }
  return ctx;
}
