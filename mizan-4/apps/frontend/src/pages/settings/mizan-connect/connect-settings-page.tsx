import { ComingSoonCard } from "@/components/coming-soon-card";
import { ConnectedView } from "@/features/mizan-connect/components/connected-view";
import { LoginForm } from "@/features/mizan-connect/components/login-form";
import { useMizanConnect } from "@/features/mizan-connect/providers/mizan-connect-provider";
import { Separator } from "@mizan/ui/components/ui/separator";
import { SettingsHeader } from "../settings-header";

/**
 * Mizan Connect settings tab.
 *
 * Thin wrapper around existing components — no auth or fetch logic of its
 * own. Conditionally renders {@link LoginForm} when signed out and
 * {@link ConnectedView} when signed in. When the Connect feature flag is off
 * (no `.env` configuration), falls back to a placeholder so the tab is still
 * reachable but doesn't expose disabled controls.
 */
export default function ConnectSettingsPage() {
  const { isEnabled, isConnected, isInitializing } = useMizanConnect();

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Mizan Connect"
        text="Sign in to enable cross-device sync and the upcoming brokerage integrations."
      />
      <Separator />
      {!isEnabled ? (
        <ComingSoonCard
          title="Mizan Connect not yet configured"
          message="Your portfolio data is fully usable offline — manual entries, CSV imports, and on-device AI all work without signing in. Mizan Connect unlocks cross-device sync, broker connections via Plaid, and cloud-grade AI."
          detail="Operators: set SUPABASE_PUBLISHABLE_KEY on the Mizan Connect Fly app — the desktop auto-discovers it via GET /api/v1/config/public on the next launch. No installer rebuild needed."
        />
      ) : isInitializing ? null : isConnected ? (
        <ConnectedView />
      ) : (
        <LoginForm />
      )}
    </div>
  );
}
