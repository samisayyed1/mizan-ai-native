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
          title="Mizan Connect is offline in this build"
          message="This copy of Mizan ships without the cloud sign-in flow wired in. Everything else works offline-first — your data stays on this device."
          detail="If you built this from source, drop CONNECT_AUTH_URL + CONNECT_AUTH_PUBLISHABLE_KEY into .env and rebuild. Production binaries from the Mizan site come pre-wired."
        />
      ) : isInitializing ? null : isConnected ? (
        <ConnectedView />
      ) : (
        <LoginForm />
      )}
    </div>
  );
}
