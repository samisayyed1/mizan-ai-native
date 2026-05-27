// Current plan card — rendered for users with an active/trialing
// Stripe subscription. Replaces the upgrade grid (`SubscriptionPlans`)
// once a paid plan is active, so a Silver user has a clear visual
// confirmation that their payment landed + a path to Gold (which is
// what unlocks broker sync) or to the Stripe Customer Portal (for
// downgrade / cancel / payment-method updates).
//
// Without this card, a Silver user looks identical to a Free user
// because the connected-view's plans grid was the only visible
// surface; this caused the QA loop where paid testers thought their
// upgrade hadn't worked.

import { openCheckout, openBillingPortal, logger } from "@/adapters";
import { Badge } from "@mizan/ui/components/ui/badge";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { useState } from "react";

// Lazy-load Tauri's shell plugin so web builds don't import it.
async function openExternalUrl(url: string): Promise<void> {
  try {
    const mod = await import("@tauri-apps/plugin-shell");
    await mod.open(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

interface Props {
  /** Current plan slug from /v1/me — e.g. "silver", "gold", "enterprise". */
  plan: string;
  /** Stripe subscription status — "active" or "trialing" when this card renders. */
  status: string;
  /**
   * True when the current plan does NOT include broker sync (i.e. Silver
   * or any legacy tier below Gold). Surfaces the "Upgrade to Gold" CTA
   * inline so Silver users have a one-click path to unlock Plaid +
   * SnapTrade without scrolling away to a separate billing page.
   */
  canUpgradeToGold: boolean;
  /** Re-fetch /v1/me after the user returns from Stripe portal/checkout. */
  onRefresh: () => void;
}

export function CurrentPlanCard({ plan, status, canUpgradeToGold, onRefresh }: Props) {
  const [busy, setBusy] = useState<"upgrade" | "portal" | null>(null);

  const planLabel = plan.charAt(0).toUpperCase() + plan.slice(1);
  const statusLabel = status === "trialing" ? "Trial active" : "Active";

  // Open the user's default browser to Stripe Customer Portal — for
  // downgrade, cancel, payment-method update, or invoice history.
  const handleManage = async () => {
    setBusy("portal");
    try {
      const url = await openBillingPortal();
      await openExternalUrl(url);
      const onFocus = () => {
        onRefresh();
        window.removeEventListener("focus", onFocus);
      };
      window.addEventListener("focus", onFocus);
    } catch (err) {
      const raw = err instanceof Error ? err.message : String(err);
      logger.error(`openBillingPortal failed: ${raw}`);
      const lower = raw.toLowerCase();
      // Common failure: the user has a subscription row but no Stripe
      // customer_id linked — usually because the subscription was
      // provisioned out-of-band (admin force-grant for QA, or legacy
      // migration). Stripe's Customer Portal API requires a real
      // customer_id; we can't synthesise one. Tell the user how to
      // formalise it instead of dumping "API error 404".
      const noCustomer =
        lower.includes("no stripe customer") ||
        lower.includes("not found") ||
        lower.includes("not_found");
      toast({
        title: noCustomer ? "No Stripe billing record yet" : "Couldn't open billing portal",
        description: noCustomer
          ? "This subscription was provisioned outside Stripe (QA grant or legacy migration). To get a Stripe billing record — required for the self-service portal — run a fresh checkout."
          : raw,
        variant: "destructive",
      });
    } finally {
      setBusy(null);
    }
  };

  const handleUpgradeToGold = async () => {
    setBusy("upgrade");
    try {
      const url = await openCheckout("gold", "monthly");
      await openExternalUrl(url);
      const onFocus = () => {
        onRefresh();
        window.removeEventListener("focus", onFocus);
      };
      window.addEventListener("focus", onFocus);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.error(`openCheckout(gold) failed: ${msg}`);
      toast({
        title: "Couldn't open Gold checkout",
        description: msg,
        variant: "destructive",
      });
    } finally {
      setBusy(null);
    }
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">Current plan</CardTitle>
          <Badge variant="default" className="text-xs">
            {planLabel}
          </Badge>
          <Badge variant="secondary" className="text-xs">
            {statusLabel}
          </Badge>
        </div>
        <CardDescription>
          {canUpgradeToGold
            ? `You're on ${planLabel}. Upgrade to Gold to add broker connections (Plaid, SnapTrade) and unlimited AI credits.`
            : `You're on ${planLabel}. Broker sync, advanced reports, and unlimited AI are all included.`}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap gap-2">
          {canUpgradeToGold && (
            <Button onClick={handleUpgradeToGold} disabled={busy !== null}>
              {busy === "upgrade" ? "Opening Gold checkout…" : "Upgrade to Gold"}
            </Button>
          )}
          <Button variant="outline" onClick={handleManage} disabled={busy !== null}>
            {busy === "portal" ? "Opening portal…" : "Manage subscription"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
