import { openCheckout } from "@/adapters";
import { logger } from "@/adapters";
import { QueryKeys as Keys } from "@/lib/query-keys";
import { useQueryClient } from "@tanstack/react-query";
import { getSubscriptionPlans } from "../services/broker-service";
import { useMizanConnect } from "../providers/mizan-connect-provider";
import type { BillingPeriod, SubscriptionPlan } from "../types";

// Lazy-load Tauri's shell plugin so web builds don't try to import it.
async function openExternalUrl(url: string): Promise<void> {
  try {
    const mod = await import("@tauri-apps/plugin-shell");
    await mod.open(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
import { Badge } from "@mizan/ui/components/ui/badge";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Popover, PopoverContent, PopoverTrigger } from "@mizan/ui/components/ui/popover";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@mizan/ui/components/ui/toggle-group";
import { QueryKeys } from "@/lib/query-keys";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

// Helper to detect if error is an auth/token issue
function isAuthError(error: Error | null): boolean {
  if (!error) return false;
  const msg = error.message.toLowerCase();
  return (
    msg.includes("access token") ||
    msg.includes("refresh token") ||
    msg.includes("unauthorized") ||
    msg.includes("sign in") ||
    msg.includes("session expired")
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Custom Hooks
// ─────────────────────────────────────────────────────────────────────────────

function useSubscriptionPlans(enabled: boolean) {
  return useQuery({
    queryKey: [QueryKeys.SUBSCRIPTION_PLANS],
    queryFn: getSubscriptionPlans,
    enabled,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Sub-Components
// ─────────────────────────────────────────────────────────────────────────────

function PlanCardSkeleton() {
  return (
    <div className="border-border bg-muted/30 flex flex-col rounded-lg border p-4">
      {/* Title and description */}
      <div className="mb-3">
        <Skeleton className="h-5 w-28" />
        <Skeleton className="mt-1.5 h-3 w-full" />
      </div>
      {/* Price */}
      <div className="mb-4">
        <Skeleton className="h-7 w-20" />
      </div>
      {/* Features */}
      <div className="mb-4 flex-1 space-y-1.5">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="flex items-center gap-2">
            <Skeleton className="h-3 w-3 rounded-full" />
            <Skeleton className="h-3 w-full" />
          </div>
        ))}
      </div>
      {/* Button */}
      <Skeleton className="mt-auto h-8 w-full rounded-md" />
    </div>
  );
}

interface PlanCardProps {
  plan: SubscriptionPlan;
  billingPeriod: BillingPeriod;
  isDefault?: boolean;
  isComingSoon?: boolean;
}

function PlanCard({ plan, billingPeriod, isDefault, isComingSoon }: PlanCardProps) {
  const priceAmount = billingPeriod === "monthly" ? plan.pricing.monthly : plan.pricing.yearly;
  const yearlyPricing = plan.pricing.yearly;
  const monthlyPricing = plan.pricing.monthly;
  const queryClient = useQueryClient();
  const [submitting, setSubmitting] = useState(false);

  // Use yearlyDiscountPercent from API if available, otherwise calculate
  const yearlySavings =
    billingPeriod === "yearly"
      ? (plan.yearlyDiscountPercent ??
        Math.round(((monthlyPricing * 12 - yearlyPricing) / (monthlyPricing * 12)) * 100))
      : 0;

  const formatPrice = (amount: number) => {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "USD",
      minimumFractionDigits: 0,
      maximumFractionDigits: 2,
    }).format(amount);
  };

  // Wire the "Get Started" CTA to the real Stripe Checkout flow.
  // The cloud's /api/v1/billing/checkout-session mints a hosted Stripe
  // URL keyed off the user's Supabase JWT + the chosen plan slug +
  // interval; we open it in the user's default browser, then
  // invalidate the entitlements query so the new plan unlocks on
  // window-focus return without a full reload.
  //
  // When the user is on the Free tier (not signed in) the upstream
  // command returns a 401 — that surfaces via toast so they know
  // they need to sign into Mizan Connect first.
  const handleGetStarted = async () => {
    setSubmitting(true);
    try {
      const url = await openCheckout(plan.id, billingPeriod);
      await openExternalUrl(url);
      // Mark mid-checkout so a focus listener can refresh entitlements
      // once they come back. The provider-level upgrade-gate already
      // wires that listener; we re-invalidate here for the case the
      // user lands on the plans page directly.
      const onFocus = () => {
        queryClient.invalidateQueries({ queryKey: [Keys.ENTITLEMENTS] });
        queryClient.invalidateQueries({ queryKey: [Keys.USER_INFO] });
        window.removeEventListener("focus", onFocus);
      };
      window.addEventListener("focus", onFocus);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.error(`openCheckout failed: ${msg}`);
      const isAuth =
        msg.toLowerCase().includes("unauthorized") ||
        msg.toLowerCase().includes("sign in") ||
        msg.toLowerCase().includes("access token");
      toast({
        title: isAuth ? "Sign in to Mizan Connect first" : "Couldn't open checkout",
        description: isAuth
          ? "Mizan Connect sign-in is required before you can subscribe. Open Settings → Mizan Connect to sign in."
          : `Stripe checkout couldn't be opened: ${msg}`,
        variant: "destructive",
      });
    } finally {
      setSubmitting(false);
    }
  };

  // Use isComingSoon from API if available
  const showComingSoon = isComingSoon || plan.isComingSoon;

  return (
    <div
      className={`bg-muted/30 relative flex flex-col rounded-lg border p-4 ${
        isDefault ? "border-primary" : "border-border"
      } ${showComingSoon ? "opacity-60" : ""}`}
    >
      {showComingSoon && (
        <Badge className="absolute -top-2.5 left-1/2 -translate-x-1/2" variant="secondary">
          Coming Soon
        </Badge>
      )}

      <div className="mb-3">
        <h3 className="text-base font-semibold">{plan.name}</h3>
        <p className="text-muted-foreground mt-0.5 text-xs">{plan.tagline ?? plan.description}</p>
      </div>

      <div className="mb-4">
        <div className="flex items-baseline gap-1">
          <span className="text-2xl font-bold">{formatPrice(priceAmount)}</span>
          <span className="text-muted-foreground text-xs">
            /{billingPeriod === "monthly" ? "mo" : "yr"}
          </span>
        </div>
        {billingPeriod === "yearly" && yearlySavings > 0 && (
          <p className="mt-0.5 text-xs text-success dark:text-green-400">
            Save {yearlySavings}% vs monthly
          </p>
        )}
      </div>

      <ul className="mb-4 flex-1 space-y-1.5">
        {plan.features.map((feature, index) => (
          <li key={index} className="flex items-start gap-2 text-xs">
            <Icons.Check className="mt-0.5 h-3 w-3 shrink-0 text-success" />
            <span className="text-muted-foreground">{feature}</span>
          </li>
        ))}
      </ul>

      <Button
        className="mt-auto w-full"
        size="sm"
        variant={isDefault ? "default" : "outline"}
        onClick={handleGetStarted}
        disabled={showComingSoon || !plan.isAvailable || submitting}
      >
        {showComingSoon
          ? "Coming Soon"
          : submitting
            ? "Opening checkout…"
            : "Get Started"}
      </Button>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Component
// ─────────────────────────────────────────────────────────────────────────────

interface SubscriptionPlansProps {
  enabled?: boolean;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}

export function SubscriptionPlans({
  enabled = true,
  onRefresh,
  isRefreshing,
}: SubscriptionPlansProps) {
  const [billingPeriod, setBillingPeriod] = useState<BillingPeriod>("monthly");
  const { data, isLoading, error } = useSubscriptionPlans(enabled);
  const { signOut, isLoading: isSigningOut } = useMizanConnect();

  // Handle auth errors - session expired or token sync issue
  if (error && isAuthError(error)) {
    const handleReconnect = async () => {
      // Sign out to clear stale session, then user can sign in again
      await signOut();
    };

    const handleRefresh = () => {
      window.location.reload();
    };

    return (
      <Card className="border-warning/30 bg-warning/5">
        <CardContent className="py-8">
          <div className="flex flex-col items-center justify-center text-center">
            <div className="bg-warning/15 mb-4 rounded-full p-4">
              <Icons.AlertCircle className="text-warning h-8 w-8" />
            </div>
            <h3 className="text-foreground mb-2 text-base font-semibold">Connection Issue</h3>
            <p className="text-muted-foreground mb-4 max-w-sm text-sm">
              We&apos;re having trouble connecting to your account. This can happen if your session
              has expired.{" "}
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    className="text-muted-foreground hover:text-foreground underline underline-offset-2"
                  >
                    Contact support
                  </button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-3">
                  <div className="flex items-center gap-2">
                    <span className="text-sm">support@mizan.app</span>
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-7 w-7"
                      onClick={() => {
                        navigator.clipboard.writeText("support@mizan.app");
                        toast.success("Email copied to clipboard");
                      }}
                      aria-label="Copy support email"
                    >
                      <Icons.Copy className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </PopoverContent>
              </Popover>
            </p>
            <div className="flex items-center gap-2">
              <Button size="sm" variant="outline" onClick={handleRefresh}>
                <Icons.Refresh className="mr-2 h-4 w-4" />
                Refresh
              </Button>
              <Button size="sm" onClick={handleReconnect} disabled={isSigningOut}>
                {isSigningOut ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Reconnecting...
                  </>
                ) : (
                  <>
                    <Icons.LogOut className="mr-2 h-4 w-4" />
                    Reconnect
                  </>
                )}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  // Handle other errors (service unavailable, network issues, etc.)
  if (error) {
    return (
      <Card className="border-warning/30 bg-warning/5">
        <CardContent className="py-8">
          <div className="flex flex-col items-center justify-center text-center">
            <div className="bg-warning/15 mb-4 rounded-full p-4">
              <Icons.CloudOff className="text-warning h-8 w-8" />
            </div>
            <h3 className="text-foreground mb-2 text-base font-semibold">Unable to Load Plans</h3>
            <p className="text-muted-foreground mb-4 max-w-sm text-sm">
              We couldn&apos;t retrieve subscription plans right now. This is usually a temporary
              issue.{" "}
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    className="text-muted-foreground hover:text-foreground underline underline-offset-2"
                  >
                    Contact support
                  </button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-3">
                  <div className="flex items-center gap-2">
                    <span className="text-sm">support@mizan.app</span>
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-7 w-7"
                      onClick={() => {
                        navigator.clipboard.writeText("support@mizan.app");
                        toast.success("Email copied to clipboard");
                      }}
                      aria-label="Copy support email"
                    >
                      <Icons.Copy className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </PopoverContent>
              </Popover>
            </p>
            <Button variant="outline" size="sm" onClick={() => window.location.reload()}>
              <Icons.Refresh className="mr-2 h-4 w-4" />
              Refresh Page
            </Button>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="pb-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <CardTitle className="text-base font-semibold">Choose Your Plan</CardTitle>
            <CardDescription className="text-xs">
              Silver is private AI wealth tracking. Gold adds Plaid live sync and monitoring.
            </CardDescription>
          </div>
          <ToggleGroup
            type="single"
            value={billingPeriod}
            onValueChange={(value) => value && setBillingPeriod(value as BillingPeriod)}
            className="bg-muted h-8 rounded-md p-0.5"
          >
            <ToggleGroupItem
              value="monthly"
              aria-label="Monthly billing"
              className="data-[state=on]:bg-background h-7 rounded px-3 text-xs data-[state=on]:shadow-sm"
            >
              Monthly
            </ToggleGroupItem>
            <ToggleGroupItem
              value="yearly"
              aria-label="Yearly billing"
              className="data-[state=on]:bg-background h-7 rounded px-3 text-xs data-[state=on]:shadow-sm"
            >
              Yearly
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {isLoading ? (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <PlanCardSkeleton />
            <PlanCardSkeleton />
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {data?.plans.map((plan) => (
              <PlanCard
                key={plan.id}
                plan={plan}
                billingPeriod={billingPeriod}
                isDefault={plan.id === "gold" || plan.badge === "Popular"}
              />
            ))}
          </div>
        )}

        {/* Refresh hint after subscribing */}
        {onRefresh && (
          <div className="mt-4 flex items-center justify-center gap-2 rounded-md border border-dashed p-3">
            <p className="text-muted-foreground text-xs">Already subscribed?</p>
            <Button
              variant="ghost"
              size="sm"
              onClick={onRefresh}
              disabled={isRefreshing}
              className="h-7 px-2 text-xs"
            >
              {isRefreshing ? (
                <Icons.Spinner className="mr-1.5 h-3 w-3 animate-spin" />
              ) : (
                <Icons.Refresh className="mr-1.5 h-3 w-3" />
              )}
              Refresh
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
