import type { GatedFeature, PlanId } from "../types";

export interface UpgradeCopy {
  title: string;
  body: string;
  /** Suggested plan slug to highlight in the CTA. */
  suggestedTier: PlanId;
}

/**
 * Contextual upsell copy keyed by gated feature (product manual §15). Used by
 * the upgrade modal so every paywall speaks to the specific action the user
 * just attempted, not a generic "upgrade now".
 */
const COPY: Record<GatedFeature, UpgradeCopy> = {
  max_portfolios: {
    title: "Add more portfolios",
    body: "Silver is designed for private AI tracking. Gold adds live sync and background monitoring for larger connected wealth pictures.",
    suggestedTier: "gold",
  },
  max_asset_classes: {
    title: "Track your full wealth",
    body: "Silver tracks all core asset classes through chat and files. Gold adds continuous account sync and monitoring.",
    suggestedTier: "gold",
  },
  max_holdings: {
    title: "Add more holdings",
    body: "Gold is built for continuously synced portfolios with broad holdings and account coverage.",
    suggestedTier: "gold",
  },
  managed_ai: {
    title: "Meet Mizan AI",
    body: "Sign in with Mizan to enable the AI assistant — 50 messages a day on Silver, autonomous agent mode on Gold. Power users can plug in their own provider under Settings → AI → Advanced.",
    suggestedTier: "silver",
  },
  device_sync: {
    title: "Sync across your devices",
    body: "Gold unlocks continuous sync and monitoring while preserving the encrypted local boundary.",
    suggestedTier: "gold",
  },
  broker_sync: {
    title: "Connect live accounts",
    body: "Gold uses Plaid to keep bank accounts, cards, liabilities, investments, transactions, balances, and holdings updated.",
    suggestedTier: "gold",
  },
  csv_imports: {
    title: "Import more statements",
    body: "CSV and file ingestion are core Silver features. Configure AI if importing is unavailable.",
    suggestedTier: "silver",
  },
  advanced_reports: {
    title: "Unlock deep reports",
    body: "Gold unlocks weekly AI wealth summaries, portfolio health, drift, cash drag, and liability monitoring.",
    suggestedTier: "gold",
  },
  zakat_engine: {
    title: "Compute your Zakat",
    body: "Gold unlocks the Zakat & purification engine — full portfolio assessment with deductions for short-term debts, gold/silver nisab handling, and the full audit trail.",
    suggestedTier: "gold",
  },
};

const FALLBACK: UpgradeCopy = {
  title: "Upgrade Mizan",
  body: "This capability is part of Gold live wealth monitoring.",
  suggestedTier: "gold",
};

export function upgradeCopyFor(feature: string): UpgradeCopy {
  return COPY[feature as GatedFeature] ?? FALLBACK;
}
