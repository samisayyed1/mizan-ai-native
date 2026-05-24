// Mizan Connect Feature
// ============================

// Provider and hook
export { MizanConnectProvider, useMizanConnect } from "./providers/mizan-connect-provider";
export { UpgradeGateProvider, useUpgradeGate } from "./providers/upgrade-gate-provider";

// Entitlements
export { useEntitlements } from "./hooks/use-entitlements";
export { parseGatedError } from "./lib/gated-error";

// Components
export { ConnectedView } from "./components/connected-view";
export { LoginForm } from "./components/login-form";
export { SubscriptionPlans } from "./components/subscription-plans";
export { ProviderButton } from "./components/provider-button";

// Plan capabilities
export { hasBrokerSync } from "./lib/plan-capabilities";

// Services
export {
  syncBrokerData,
  getSyncedAccounts,
  getPlatforms,
  listBrokerConnections,
  getSubscriptionPlans,
  getSubscriptionPlansPublic,
  getUserInfo,
} from "./services/broker-service";

export { storeSyncSession, clearSyncSession } from "./services/auth-service";

// Types
export type {
  SyncConnectionsResponse,
  SyncAccountsResponse,
  SyncActivitiesResponse,
  SyncResult,
  BrokerConnectionBrokerage,
  BrokerConnection,
  PlanId,
  BillingPeriod,
  PlanPricing,
  PlanLimits,
  SubscriptionPlan,
  PlansResponse,
  UserTeam,
  DateFormat,
  UserInfo,
  Entitlements,
  GatedError,
  GatedFeature,
} from "./types";
export { UNLIMITED, withinLimit } from "./types";
