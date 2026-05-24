export type MizanAccountTier = "silver" | "gold";

export type AccountCapabilities = {
  tier: MizanAccountTier;
  aiAssistant: true;
  encryptedLocalStorage: true;
  csvIngestion: true;
  conversationalAssets: true;
  alternativeAssets: true;
  zakatEngine: boolean;
  balanceMasking: true;
  plaidSync: boolean;
  liveLiabilityTracking: boolean;
  backgroundMonitoring: boolean;
  automatedPortfolioHealth: boolean;
  allocationDriftDetection: boolean;
  cashDragDetection: boolean;
  weeklyAiReports: boolean;
  proactiveAlerts: boolean;
};

export type AccountCapability = Exclude<keyof AccountCapabilities, "tier">;

const BASE_CAPABILITIES = {
  aiAssistant: true,
  encryptedLocalStorage: true,
  csvIngestion: true,
  conversationalAssets: true,
  alternativeAssets: true,
  balanceMasking: true,
} as const;

const CAPABILITIES: Record<MizanAccountTier, AccountCapabilities> = {
  silver: {
    tier: "silver",
    ...BASE_CAPABILITIES,
    zakatEngine: false,
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
    zakatEngine: true,
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

export function normalizeAccountTier(plan?: string | null): MizanAccountTier {
  const slug = plan?.trim().toLowerCase();
  if (!slug || slug === "free" || slug === "basic" || slug === "silver") {
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
