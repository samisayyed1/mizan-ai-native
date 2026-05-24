import type { UserInfo } from "../types";
import { canUseCapability, normalizeAccountTier } from "@/domain/account/capabilities";

export function hasBrokerSync(userInfo: UserInfo | null): boolean {
  const team = userInfo?.team;
  const isActive =
    team?.subscription_status === "active" || team?.subscription_status === "trialing";
  return isActive && canUseCapability(normalizeAccountTier(team?.plan), "plaidSync");
}
