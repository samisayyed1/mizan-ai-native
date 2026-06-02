// Onboarding sign-in step — surfaces Sign in with Google + email
// magic-link, both optional. Skipping keeps the user on the local-only
// path; they can sign in later from Settings → Mizan Connect.
//
// AI-Native-3 — sign-in lands here so new users discover managed AI
// + cross-device sync during onboarding instead of finding it buried
// in settings days later.

import { useMizanConnect } from "@/features/mizan-connect/providers/mizan-connect-provider";
import { Button } from "@mizan/ui/components/ui/button";
import { Input } from "@mizan/ui/components/ui/input";
import { Label } from "@mizan/ui/components/ui/label";
import { toast } from "sonner";
import { useState } from "react";

interface Props {
  /** Called when the user opts to skip sign-in for now. */
  onSkip: () => void;
  /** Called once a session is established. */
  onSignedIn: () => void;
}

export function OnboardingSignIn({ onSkip, onSignedIn }: Props) {
  const connect = useMizanConnect();
  const [email, setEmail] = useState("");
  const [magicLinkLoading, setMagicLinkLoading] = useState(false);
  const [googleLoading, setGoogleLoading] = useState(false);

  // If the user signed in inside this step, fire the callback so the
  // parent advances the stepper.
  if (connect.isConnected) {
    // Defer the call so we don't trigger a re-render inside render.
    queueMicrotask(onSignedIn);
  }

  const handleGoogle = async () => {
    if (!connect.isEnabled) {
      toast.error(
        "This build of Mizan was shipped without Connect wired in. Sign-in is unavailable.",
      );
      return;
    }
    setGoogleLoading(true);
    try {
      await connect.signInWithOAuth("google");
      // The callback is handled by the deep-link listener; we'll see
      // `connect.isConnected` flip to true and the parent step will
      // advance via the queueMicrotask above.
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Sign-in failed";
      // Common case: user closed the browser window mid-OAuth.
      if (!msg.toLowerCase().includes("cancel")) {
        toast.error(msg);
      }
    } finally {
      setGoogleLoading(false);
    }
  };

  const handleMagicLink = async () => {
    if (!email.trim()) return;
    if (!connect.isEnabled) {
      toast.error(
        "This build of Mizan was shipped without Connect wired in. Sign-in is unavailable.",
      );
      return;
    }
    setMagicLinkLoading(true);
    try {
      await connect.signInWithMagicLink(email.trim());
      toast.success(
        `Check ${email.trim()} for a sign-in link. You can close this and pick up where you left off.`,
      );
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't send the link");
    } finally {
      setMagicLinkLoading(false);
    }
  };

  return (
    <div className="w-full max-w-md space-y-6 text-center">
      <div className="space-y-2">
        <h1 className="text-2xl font-bold tracking-tight md:text-3xl">
          Want managed AI and cross-device sync?
        </h1>
        <p className="text-muted-foreground mx-auto max-w-sm text-sm leading-relaxed md:text-base">
          Sign in to unlock Silver and Gold features. Or skip — Mizan works
          offline forever and you can sign in later from Settings.
        </p>
      </div>

      <div className="space-y-3">
        <Button
          type="button"
          size="lg"
          variant="outline"
          className="h-11 w-full justify-center gap-3"
          onClick={handleGoogle}
          disabled={googleLoading || !connect.isEnabled}
        >
          <GoogleIcon />
          {googleLoading ? "Opening Google…" : "Continue with Google"}
        </Button>

        <div className="flex items-center gap-3">
          <div className="bg-border h-px flex-1" />
          <span className="text-muted-foreground text-xs uppercase tracking-wider">
            or
          </span>
          <div className="bg-border h-px flex-1" />
        </div>

        <div className="space-y-2 text-left">
          <Label htmlFor="onboarding-email" className="text-xs">
            Email
          </Label>
          <Input
            id="onboarding-email"
            type="email"
            autoComplete="email"
            placeholder="you@example.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            disabled={magicLinkLoading || !connect.isEnabled}
            className="h-11"
          />
          <Button
            type="button"
            size="lg"
            className="from-primary to-primary/90 bg-linear-to-r h-11 w-full"
            onClick={handleMagicLink}
            disabled={!email.trim() || magicLinkLoading || !connect.isEnabled}
          >
            {magicLinkLoading ? "Sending link…" : "Email me a sign-in link"}
          </Button>
        </div>
      </div>

      <button
        type="button"
        className="text-muted-foreground hover:text-foreground text-sm underline-offset-4 transition-colors hover:underline focus-visible:outline-none focus-visible:underline"
        onClick={onSkip}
      >
        Skip for now — I'll set up locally
      </button>

      {!connect.isEnabled && (
        <p className="text-muted-foreground mx-auto max-w-xs text-xs leading-relaxed">
          Sign-in is unavailable in this build. You can still use Mizan locally —
          just press Skip.
        </p>
      )}

      {connect.error && (
        <p className="text-destructive text-xs leading-relaxed">
          {connect.error}{" "}
          <button
            type="button"
            className="underline"
            onClick={() => connect.clearError()}
          >
            Dismiss
          </button>
        </p>
      )}
    </div>
  );
}

/**
 * Inline Google "G" mark — kept as a component so it inherits dark-mode
 * tweaks and we don't pull in another icon dependency.
 */
function GoogleIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      aria-hidden="true"
      role="img"
    >
      <path
        d="M21.35 11.1H12v3.2h5.35c-.23 1.4-1.65 4.1-5.35 4.1-3.22 0-5.85-2.67-5.85-5.95s2.63-5.95 5.85-5.95c1.84 0 3.07.79 3.77 1.47l2.57-2.48C16.7 4.06 14.55 3 12 3 6.97 3 2.9 7.05 2.9 12.1s4.07 9.1 9.1 9.1c5.25 0 8.73-3.69 8.73-8.88 0-.6-.06-1.05-.13-1.42z"
        fill="currentColor"
      />
    </svg>
  );
}

