export type MizanAccountTier = "free" | "silver" | "gold";

export interface AccountCapabilities {
  tier: MizanAccountTier;
  aiAssistant: true;
  encryptedLocalStorage: true;
  /** True for Silver+, false for Free. */
  csvIngestion: boolean;
  /** True everywhere — chat with BYO key works on Free. */
  conversationalAssets: true;
  /** Manual alt-asset entry — always allowed. */
  alternativeAssets: true;
  /** Gold-only (per Feroz 25 May 2026). */
  zakatEngine: boolean;
  balanceMasking: true;
  /** Bring-your-own AI key — always allowed (Free, Silver, Gold). */
  byoAi: true;
  /** Managed AI (Mizan-hosted) — Silver+. */
  managedAi: boolean;
  /** Yahoo + TradingView live quotes — always on. */
  liveQuotes: true;
  /** News feed — always on (Free has the daily limit gate). */
  news: true;
  plaidSync: boolean;
  liveLiabilityTracking: boolean;
  backgroundMonitoring: boolean;
  automatedPortfolioHealth: boolean;
  allocationDriftDetection: boolean;
  cashDragDetection: boolean;
  weeklyAiReports: boolean;
  proactiveAlerts: boolean;
}

export type AccountCapability = Exclude<keyof AccountCapabilities, "tier">;

/** Capabilities every tier (Free included) shares — privacy + AI-native core. */
const BASE_CAPABILITIES = {
  aiAssistant: true,
  encryptedLocalStorage: true,
  conversationalAssets: true,
  alternativeAssets: true,
  balanceMasking: true,
  byoAi: true,
  liveQuotes: true,
  news: true,
} as const;

const CAPABILITIES: Record<MizanAccountTier, AccountCapabilities> = {
  free: {
    tier: "free",
    ...BASE_CAPABILITIES,
    csvIngestion: false,
    zakatEngine: false,
    managedAi: false,
    plaidSync: false,
    liveLiabilityTracking: false,
    backgroundMonitoring: false,
    automatedPortfolioHealth: false,
    allocationDriftDetection: false,
    cashDragDetection: false,
    weeklyAiReports: false,
    proactiveAlerts: false,
  },
  silver: {
    tier: "silver",
    ...BASE_CAPABILITIES,
    csvIngestion: true,
    zakatEngine: false,
    managedAi: true,
    plaidSync: false,
    liveLiabilityTracking: false,
    backgroundMonitoring: false,
    automatedPortfolioHealth: false,
    allocationDriftDetection: false,
    cashDragDetection: false,
    weeklyAiReports: false,
    proactiveAlerts: false,
  },
  gold: {
    tier: "gold",
    ...BASE_CAPABILITIES,
    csvIngestion: true,
    zakatEngine: true,
    managedAi: true,
    plaidSync: true,
    liveLiabilityTracking: true,
    backgroundMonitoring: true,
    automatedPortfolioHealth: true,
    allocationDriftDetection: true,
    cashDragDetection: true,
    weeklyAiReports: true,
    proactiveAlerts: true,
  },
};

/**
 * Map a plan slug from the cloud to a local tier.
 *
 * - Unknown / unsigned-in / explicit "free" → `free`
 * - Legacy "basic" / "silver" → `silver` (paid existing customers don't downgrade)
 * - Anything else (paid) → `gold`
 */
export function normalizeAccountTier(plan?: string | null): MizanAccountTier {
  const slug = plan?.trim().toLowerCase();
  if (!slug || slug === "free") {
    return "free";
  }
  if (slug === "basic" || slug === "silver") {
    return "silver";
  }
  return "gold";
}

export function getAccountCapabilities(tier: MizanAccountTier): AccountCapabilities {
  return CAPABILITIES[tier];
}

export function canUseCapability(
  tier: MizanAccountTier,
  capability: AccountCapability,
): boolean {
  return getAccountCapabilities(tier)[capability] === true;
}

export function assertCapability(
  tier: MizanAccountTier,
  capability: AccountCapability,
): asserts tier is MizanAccountTier {
  if (!canUseCapability(tier, capability)) {
    throw new Error(`Mizan ${tier} cannot use capability: ${capability}`);
  }
}
