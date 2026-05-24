import { isWeb, logger } from "@/adapters";
import { RootErrorBoundary } from "@/components/root-error-boundary";
import { AuthGate, AuthProvider } from "@/context/auth-context";
import { MizanConnectProvider } from "@/features/mizan-connect";
import { emitGatedError } from "@/features/mizan-connect/lib/gated-error-bus";
import { SettingsProvider } from "@/lib/settings-provider";
import { MutationCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@mizan/ui";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { useState } from "react";
import { PrivacyProvider } from "./context/privacy-context";
import { LoginPage } from "./pages/auth/login-page";
import { AppRoutes } from "./routes";

function App() {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            refetchOnWindowFocus: false,
            staleTime: 5 * 60 * 1000,
            retry: false,
          },
        },
        // Safety net: any mutation that hasn't supplied its own onError
        // handler will surface failures via a destructive toast + log
        // instead of silently swallowing them. Mutations that DO supply
        // their own onError keep their existing behaviour (we check
        // `mutation.options.onError` and bail so we don't double-toast).
        //
        // A second escape hatch is `meta.suppressDefaultError: true` —
        // for mutations whose caller handles errors via a `try/catch`
        // around `mutateAsync` and shows its own toast. Without the
        // opt-out the user would see *two* destructive toasts on the
        // same failure (one from the cache here, one from the caller).
        //
        // When we do toast, we surface the actual backend message when
        // it looks useful — so "Sync engine not configured" or
        // "Encryption key missing" reaches the user verbatim, not as
        // a generic "please try again".
        mutationCache: new MutationCache({
          onError: (error, _variables, _context, mutation) => {
            // Premium gate failures raise the contextual upgrade modal instead
            // of a destructive error toast.
            if (emitGatedError(error)) return;
            if (mutation.options.onError) return;
            const meta = mutation.options.meta as { suppressDefaultError?: boolean } | undefined;
            if (meta?.suppressDefaultError) return;
            const rawMessage = error instanceof Error ? error.message : String(error);
            logger.error(`Unhandled mutation failure: ${rawMessage}`);
            // Heuristic for "useful" — non-empty, not a stack-trace
            // wall, not the Tauri invoke-failed envelope. Anything else
            // came from a deliberate backend message and is worth showing.
            const trimmed = rawMessage.trim();
            const looksUseful =
              trimmed.length > 0 &&
              trimmed.length < 200 &&
              !trimmed.startsWith("Error invoking") &&
              !trimmed.includes("\n    at ");
            toast({
              title: "Something went wrong",
              description: looksUseful
                ? trimmed
                : "Your action couldn't be completed. Please try again.",
              variant: "destructive",
            });
          },
        }),
      }),
  );

  const isWebEnv = isWeb;

  // Make QueryClient available globally for addons
  window.__mizan_query_client__ = queryClient;

  const routedContent = isWebEnv ? (
    <AuthGate fallback={<LoginPage />}>
      <AppRoutes />
    </AuthGate>
  ) : (
    <AppRoutes />
  );

  return (
    // Top-level error boundary OUTSIDE every provider so even a
    // provider that throws during initialisation (auth, query client,
    // settings) renders the recovery screen instead of a white page.
    <RootErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <MizanConnectProvider>
            <PrivacyProvider>
              <SettingsProvider>
                <TooltipProvider>{routedContent}</TooltipProvider>
              </SettingsProvider>
            </PrivacyProvider>
          </MizanConnectProvider>
        </AuthProvider>
      </QueryClientProvider>
    </RootErrorBoundary>
  );
}

export default App;
