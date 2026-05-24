import { isDesktop } from "@/adapters";
import { ExternalLink } from "@/components/external-link";
import { getPreferredProvider, savePreferredProvider } from "@/lib/cookie-utils";
import { Alert, AlertDescription } from "@mizan/ui/components/ui/alert";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@mizan/ui/components/ui/collapsible";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Input } from "@mizan/ui/components/ui/input";
import { InputOTP, InputOTPGroup, InputOTPSlot } from "@mizan/ui/components/ui/input-otp";
import { Label } from "@mizan/ui/components/ui/label";
import { Separator } from "@mizan/ui/components/ui/separator";
import { useEffect, useState } from "react";
import { useMizanConnect } from "../providers/mizan-connect-provider";
import { ProviderButton } from "./provider-button";

// OAuth is only available on desktop/mobile (Tauri) where we can handle deep links
// Web (self-hosted) uses email-based auth only since we can't register all
// possible redirect URLs.
const isNativeApp = isDesktop;

type Provider = "google" | "email";
type AuthMode = "signIn" | "signUp";
type EmailFlow = "password" | "magicLink";

// ─────────────────────────────────────────────────────────────────────────────
// Features Section
// ─────────────────────────────────────────────────────────────────────────────

const features = [
  {
    icon: Icons.CloudSync2,
    title: "Broker Sync",
    description: "Auto-sync transactions daily",
    color: "orange",
  },
  {
    icon: Icons.Devices,
    title: "Device Sync",
    description: "End-to-end encrypted sync",
    color: "green",
  },
  {
    icon: Icons.Users,
    title: "Household",
    description: "Shared family portfolio view",
    color: "blue",
  },
];

const featureColors = {
  orange: {
    bg: "bg-orange-100 dark:bg-orange-900/30",
    icon: "text-orange-600 dark:text-orange-400",
  },
  green: {
    bg: "bg-green-100 dark:bg-green-900/30",
    icon: "text-green-600 dark:text-green-400",
  },
  blue: {
    bg: "bg-blue-100 dark:bg-blue-900/30",
    icon: "text-blue-600 dark:text-blue-400",
  },
};

function FeaturesSection() {
  return (
    <div className="grid grid-cols-1 gap-2 sm:grid-cols-3 sm:gap-3">
      {features.map((feature) => {
        const colors = featureColors[feature.color as keyof typeof featureColors];
        return (
          <div
            key={feature.title}
            className="bg-muted/30 flex items-center gap-3 rounded-lg border p-3 sm:flex-col sm:gap-2 sm:text-center"
          >
            <div
              className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${colors.bg}`}
            >
              <feature.icon className={`h-4 w-4 ${colors.icon}`} />
            </div>
            <div>
              <p className="text-xs font-medium">{feature.title}</p>
              <p className="text-muted-foreground text-[10px]">{feature.description}</p>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error message translation
// ─────────────────────────────────────────────────────────────────────────────

interface FriendlyError {
  message: string;
  /** When set, render an inline action under the alert. */
  action?: "resendConfirmation";
}

/** Translate Supabase / network errors into user-facing copy. */
function translateAuthError(raw: string | null | undefined, mode: AuthMode): FriendlyError | null {
  if (!raw) return null;
  const m = raw.toLowerCase();

  if (m.includes("invalid login credentials") || m.includes("invalid_credentials")) {
    return { message: "Wrong email or password." };
  }
  if (m.includes("email not confirmed") || m.includes("email_not_confirmed")) {
    return {
      message: "Confirm your email first. Check your inbox for the confirmation link.",
      action: "resendConfirmation",
    };
  }
  if (m.includes("user already registered") || m.includes("user_already_exists")) {
    return { message: "An account with that email already exists. Try signing in instead." };
  }
  if (m.includes("password should be at least") || m.includes("weak_password")) {
    return { message: raw }; // Supabase's own message is already clear here
  }
  if (m.includes("rate") && (m.includes("limit") || m.includes("429"))) {
    return { message: "Too many attempts. Try again in a minute." };
  }
  if (m.includes("network") || m.includes("failed to fetch")) {
    return { message: "Couldn't reach Supabase. Check your network and try again." };
  }
  // Pass anything else through verbatim — better to surface Supabase's text
  // than to silently swallow it.
  return { message: mode === "signUp" ? `Sign up failed: ${raw}` : `Sign in failed: ${raw}` };
}

// ─────────────────────────────────────────────────────────────────────────────
// LoginForm
// ─────────────────────────────────────────────────────────────────────────────

export function LoginForm() {
  const {
    signInWithEmail,
    signUpWithEmail,
    signInWithOAuth,
    signInWithMagicLink,
    verifyOtp,
    resendConfirmation,
    error,
    clearError,
    isLoading,
  } = useMizanConnect();

  // Mode + flow state
  const [authMode, setAuthMode] = useState<AuthMode>("signIn");
  const [emailFlow, setEmailFlow] = useState<EmailFlow>("password");

  // Form fields
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  // UI state
  const [localError, setLocalError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [loadingProvider, setLoadingProvider] = useState<Provider | null>(null);
  const [preferredProvider, setPreferredProvider] = useState<Provider | null>(null);
  const [isMoreOptionsOpen, setIsMoreOptionsOpen] = useState(false);

  // OTP / magic-link verification state
  const [showOtpInput, setShowOtpInput] = useState(false);
  const [pendingEmail, setPendingEmail] = useState("");
  const [otpCode, setOtpCode] = useState("");

  // Load preferred provider from cookie on mount
  useEffect(() => {
    const savedProvider = getPreferredProvider();
    setPreferredProvider(savedProvider);
  }, []);

  // ─────────────────────────────────────────────────────────────────────────
  // Provider visibility (unchanged from prior version — Google on desktop,
  // email always shown)
  // ─────────────────────────────────────────────────────────────────────────

  const getTopProviders = (): Provider[] => {
    if (!isNativeApp) {
      return ["email"];
    }
    if (preferredProvider) {
      return [preferredProvider];
    }
    return ["google"];
  };

  const getMoreOptionsProviders = (): Provider[] => {
    if (!isNativeApp) return [];
    const topProviders = getTopProviders();
    const allProviders: Provider[] = ["google", "email"];
    return allProviders.filter((p) => !topProviders.includes(p));
  };

  const topProviders = getTopProviders();
  const moreOptionsProviders = getMoreOptionsProviders();

  // ─────────────────────────────────────────────────────────────────────────
  // Helpers
  // ─────────────────────────────────────────────────────────────────────────

  const resetTransients = () => {
    setLocalError(null);
    setSuccessMessage(null);
    clearError();
  };

  const isValidEmail = (value: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);

  const handleSwitchMode = (mode: AuthMode) => {
    setAuthMode(mode);
    setEmailFlow("password"); // reset to password whenever we switch tabs
    resetTransients();
  };

  // ─────────────────────────────────────────────────────────────────────────
  // OAuth (Google) handler — unchanged
  // ─────────────────────────────────────────────────────────────────────────

  const handleOAuthSignIn = async (provider: "google") => {
    resetTransients();
    setLoadingProvider(provider);
    try {
      await signInWithOAuth(provider);
      savePreferredProvider(provider);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Sign in failed. Please try again.";
      setLocalError(message);
    } finally {
      setLoadingProvider(null);
    }
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Password sign-in / sign-up (PRIMARY flow)
  // ─────────────────────────────────────────────────────────────────────────

  const handlePasswordSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    resetTransients();

    if (!email || !isValidEmail(email)) {
      setLocalError("Please enter a valid email address.");
      return;
    }
    if (!password) {
      setLocalError("Please enter your password.");
      return;
    }
    if (authMode === "signUp" && password.length < 8) {
      setLocalError("Password must be at least 8 characters.");
      return;
    }

    setLoadingProvider("email");
    try {
      if (authMode === "signIn") {
        await signInWithEmail(email, password);
        savePreferredProvider("email");
        // Provider sets the session — ConnectedView takes over from here.
      } else {
        await signUpWithEmail(email, password);
        savePreferredProvider("email");
        // If Supabase auto-confirms the user, the provider will already
        // have set a session; otherwise we surface the confirm-email
        // message it stored on `error`. Either way, give the user clear
        // visual feedback.
        setSuccessMessage(
          "Account created. If your project requires email confirmation, check your inbox.",
        );
        setPendingEmail(email);
      }
    } catch {
      // Provider's `error` state already captures the Supabase message —
      // `translateAuthError(error)` will render it below. We don't
      // double-set localError here.
    } finally {
      setLoadingProvider(null);
    }
  };

  const handleResendConfirmation = async () => {
    const target = pendingEmail || email;
    if (!target || !isValidEmail(target)) {
      setLocalError("Enter your email address first.");
      return;
    }
    resetTransients();
    setLoadingProvider("email");
    try {
      await resendConfirmation(target);
      setSuccessMessage(`Confirmation email re-sent to ${target}.`);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Couldn't resend the confirmation email.";
      setLocalError(message);
    } finally {
      setLoadingProvider(null);
    }
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Magic-link / OTP flow (SECONDARY — kept for users without passwords)
  // ─────────────────────────────────────────────────────────────────────────

  const handleMagicLinkSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    resetTransients();

    if (!email || !isValidEmail(email)) {
      setLocalError("Please enter a valid email address.");
      return;
    }

    setLoadingProvider("email");
    try {
      await signInWithMagicLink(email);
      savePreferredProvider("email");
      setPendingEmail(email);
      setShowOtpInput(true);
      setEmail("");
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to send magic link. Please try again.";
      setLocalError(message);
    } finally {
      setLoadingProvider(null);
    }
  };

  const handleOtpVerify = async () => {
    if (otpCode.length !== 6) {
      setLocalError("Please enter the complete 6-digit code");
      return;
    }
    resetTransients();
    setLoadingProvider("email");
    try {
      await verifyOtp(pendingEmail, otpCode);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "";
      const isOtpExpired =
        errorMessage.toLowerCase().includes("expired") ||
        errorMessage.toLowerCase().includes("invalid");
      const message = isOtpExpired
        ? "Code expired or invalid. Please request a new code."
        : errorMessage || "Invalid code. Please try again.";
      setLocalError(message);
      setOtpCode("");
    } finally {
      setLoadingProvider(null);
    }
  };

  const handleResendCode = async () => {
    resetTransients();
    setLoadingProvider("email");
    try {
      await signInWithMagicLink(pendingEmail);
      setSuccessMessage("A new code has been sent to your email.");
      setOtpCode("");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to resend code.";
      setLocalError(message);
    } finally {
      setLoadingProvider(null);
    }
  };

  const handleBackToEmail = () => {
    setShowOtpInput(false);
    setPendingEmail("");
    setOtpCode("");
    setEmailFlow("password");
    resetTransients();
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Display state
  // ─────────────────────────────────────────────────────────────────────────

  const friendly = translateAuthError(localError ?? error, authMode);

  // ─────────────────────────────────────────────────────────────────────────
  // Email block — renders the password form by default, magic-link inline
  // when the user clicks "Use magic link instead".
  // ─────────────────────────────────────────────────────────────────────────

  const emailBlock = (
    <div className="w-full max-w-sm space-y-3">
      {/* Sign in / Sign up toggle */}
      <div
        role="tablist"
        aria-label="Authentication mode"
        className="bg-muted/40 grid grid-cols-2 rounded-full p-0.5 text-xs"
      >
        <button
          type="button"
          role="tab"
          aria-selected={authMode === "signIn"}
          onClick={() => handleSwitchMode("signIn")}
          className={`rounded-full px-3 py-1.5 font-medium transition-colors ${
            authMode === "signIn"
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Sign in
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={authMode === "signUp"}
          onClick={() => handleSwitchMode("signUp")}
          className={`rounded-full px-3 py-1.5 font-medium transition-colors ${
            authMode === "signUp"
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Sign up
        </button>
      </div>

      {emailFlow === "password" ? (
        <form onSubmit={handlePasswordSubmit} className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="email">Email</Label>
            <Input
              id="email"
              type="email"
              placeholder="you@example.com"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
              className="rounded-full"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              placeholder={authMode === "signUp" ? "At least 8 characters" : ""}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete={authMode === "signUp" ? "new-password" : "current-password"}
              className="rounded-full"
            />
          </div>
          <Button
            type="submit"
            disabled={loadingProvider === "email" || isLoading}
            className="from-primary to-primary/90 bg-linear-to-r w-full"
          >
            {loadingProvider === "email" ? (
              <>
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                {authMode === "signIn" ? "Signing in..." : "Creating account..."}
              </>
            ) : authMode === "signIn" ? (
              "Sign in"
            ) : (
              "Create account"
            )}
          </Button>
          <div className="text-center">
            <button
              type="button"
              onClick={() => {
                setEmailFlow("magicLink");
                resetTransients();
              }}
              className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
            >
              Use magic link instead
            </button>
          </div>
        </form>
      ) : (
        <form onSubmit={handleMagicLinkSubmit} className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="email-magic">Email</Label>
            <Input
              id="email-magic"
              type="email"
              placeholder="you@example.com"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
              className="rounded-full"
            />
          </div>
          <Button
            type="submit"
            disabled={loadingProvider === "email" || isLoading}
            variant="default"
            className="w-full"
          >
            {loadingProvider === "email" ? (
              <>
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                Sending magic link...
              </>
            ) : (
              "Send magic link"
            )}
          </Button>
          <div className="text-center">
            <button
              type="button"
              onClick={() => {
                setEmailFlow("password");
                resetTransients();
              }}
              className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
            >
              ← Back to password
            </button>
          </div>
        </form>
      )}
    </div>
  );

  // ─────────────────────────────────────────────────────────────────────────
  // Render
  // ─────────────────────────────────────────────────────────────────────────

  return (
    <div className="space-y-6">
      <FeaturesSection />

      <Card>
        <CardHeader className="pb-4">
          <CardTitle className="text-center text-base font-medium">
            {showOtpInput ? "Verify your email" : "Get started"}
          </CardTitle>
          <CardDescription className="text-center">
            {showOtpInput
              ? `We sent a 6-digit code to ${pendingEmail}.`
              : "Sign in to connect your broker accounts"}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Error Alert */}
          {friendly && (
            <Alert variant="destructive">
              <Icons.AlertCircle className="h-4 w-4" />
              <AlertDescription className="space-y-2">
                <div>{friendly.message}</div>
                {friendly.action === "resendConfirmation" && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleResendConfirmation}
                    disabled={loadingProvider === "email" || isLoading}
                  >
                    {loadingProvider === "email" ? (
                      <>
                        <Icons.Spinner className="mr-2 h-3 w-3 animate-spin" />
                        Sending...
                      </>
                    ) : (
                      "Resend confirmation email"
                    )}
                  </Button>
                )}
              </AlertDescription>
            </Alert>
          )}

          {/* Success Alert */}
          {successMessage && (
            <Alert>
              <Icons.CheckCircle className="h-4 w-4" />
              <AlertDescription>{successMessage}</AlertDescription>
            </Alert>
          )}

          {/* OTP verification UI (only after a magic-link send) */}
          {showOtpInput ? (
            <div className="flex flex-col items-center space-y-4">
              <InputOTP
                maxLength={6}
                value={otpCode}
                onChange={setOtpCode}
                onComplete={handleOtpVerify}
              >
                <InputOTPGroup>
                  <InputOTPSlot index={0} />
                  <InputOTPSlot index={1} />
                  <InputOTPSlot index={2} />
                  <InputOTPSlot index={3} />
                  <InputOTPSlot index={4} />
                  <InputOTPSlot index={5} />
                </InputOTPGroup>
              </InputOTP>

              <div className="flex flex-col items-center gap-2">
                <Button
                  variant="default"
                  onClick={handleOtpVerify}
                  disabled={otpCode.length !== 6 || loadingProvider === "email"}
                  className="w-full max-w-sm"
                >
                  {loadingProvider === "email" ? (
                    <>
                      <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                      Verifying...
                    </>
                  ) : (
                    "Verify"
                  )}
                </Button>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={handleBackToEmail}
                    className="text-muted-foreground"
                  >
                    <Icons.ArrowLeft className="mr-2 h-4 w-4" />
                    Back to sign in
                  </Button>
                  <span className="text-muted-foreground">•</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={handleResendCode}
                    disabled={loadingProvider === "email"}
                    className="text-muted-foreground"
                  >
                    Resend code
                  </Button>
                </div>
              </div>
            </div>
          ) : (
            <>
              {/* Top providers — Google (desktop) above email block */}
              <div className="flex flex-col items-center space-y-3">
                {topProviders.includes("google") && (
                  <ProviderButton
                    provider="google"
                    onClick={() => handleOAuthSignIn("google")}
                    isLoading={loadingProvider === "google"}
                    isLastUsed={topProviders.length > 1 && preferredProvider === "google"}
                  />
                )}
                {topProviders.includes("email") && emailBlock}
              </div>

              {/* Divider when there are alternate providers */}
              {moreOptionsProviders.length > 0 && (
                <div className="relative">
                  <div className="absolute inset-0 flex items-center">
                    <Separator className="mx-auto max-w-sm" />
                  </div>
                  <div className="relative flex justify-center text-xs uppercase">
                    <span className="bg-background text-muted-foreground px-2">or</span>
                  </div>
                </div>
              )}
            </>
          )}

          {/* Alternate sign-in options (collapsible) — hidden during OTP */}
          {!showOtpInput && moreOptionsProviders.length > 0 && (
            <Collapsible open={isMoreOptionsOpen} onOpenChange={setIsMoreOptionsOpen}>
              <div className="flex justify-center">
                <CollapsibleTrigger asChild>
                  <Button type="button" variant="ghost" className="gap-2" disabled={isLoading}>
                    <span className="text-muted-foreground text-sm">More sign-in options</span>
                    <Icons.ChevronDown
                      className={`h-4 w-4 transition-transform ${
                        isMoreOptionsOpen ? "rotate-180" : ""
                      }`}
                    />
                  </Button>
                </CollapsibleTrigger>
              </div>
              <CollapsibleContent className="flex flex-col items-center space-y-3 pt-3">
                {moreOptionsProviders.includes("google") && (
                  <ProviderButton
                    provider="google"
                    onClick={() => handleOAuthSignIn("google")}
                    isLoading={loadingProvider === "google"}
                  />
                )}
                {moreOptionsProviders.includes("email") && emailBlock}
              </CollapsibleContent>
            </Collapsible>
          )}

          {/* Terms & privacy footer */}
          {!showOtpInput && (
            <div className="pt-4">
              <p className="text-muted-foreground text-center text-xs">
                By continuing, you agree to our{" "}
                <ExternalLink
                  href="https://mizan.app/connect/legal/terms-of-use"
                  className="hover:text-foreground underline underline-offset-4"
                >
                  Terms of Use
                </ExternalLink>{" "}
                and{" "}
                <ExternalLink
                  href="https://mizan.app/connect/legal/privacy-policy"
                  className="hover:text-foreground underline underline-offset-4"
                >
                  Privacy Policy
                </ExternalLink>
                .
              </p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Privacy footnote */}
      <p className="text-muted-foreground text-center text-xs">
        Your portfolio data never leaves your device. Connect uses secure aggregators to sync
        transactions directly to your local database.
      </p>
    </div>
  );
}
