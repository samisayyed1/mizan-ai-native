import {
  deleteSecret,
  isDesktop,
  listenDeepLink,
  logger,
  openUrlInBrowser,
  setSecret,
} from "@/adapters";
import { useAuth } from "@/context/auth-context";
import { getPlatform } from "@/hooks/use-platform";
import { resolveConnectConfig, type ResolvedConnectConfig } from "@/lib/connect-config";
import { QueryKeys } from "@/lib/query-keys";
import { createClient, Session, SupabaseClient, User } from "@supabase/supabase-js";
import { useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { authenticate as authenticateWithASWebAuth } from "tauri-plugin-web-auth-api";
import { clearSyncSession, restoreSyncSession, storeSyncSession } from "../services/auth-service";
import { getUserInfo } from "../services/broker-service";
import type { UserInfo } from "../types";

// Auth configuration — must come from .env at build time.
// Empty strings keep the CONNECT_ENABLED gate honest: an offline-only build
// is exactly an empty .env. The provider never reaches createSupabaseClient
// when CONNECT_ENABLED is false, so the empty fallback is unreachable in
// practice. We keep `as string` for the type cast; the gate (connect-config.ts)
// is what actually decides whether the Connect provider runs.
// Both `CONNECT_AUTH_URL` and `CONNECT_AUTH_PUBLISHABLE_KEY` are now
// resolved at runtime via `resolveConnectConfig()` — this lets the
// desktop self-discover the cloud's Supabase config from
// `/api/v1/config/public` instead of requiring an installer rebuild
// on every key rotation. The build-time .env path still wins when
// set (see `lib/connect-config.ts`).

// Key for storing refresh token in keyring/localStorage (for session restoration)
// Note: For keyring (Tauri), the "mizan_" prefix is added automatically by SecretStore
const REFRESH_TOKEN_KEY = "sync_refresh_token";

// Deep-link URL for desktop callbacks (custom URL scheme)
const DESKTOP_DEEP_LINK_URL = "mizan://auth/callback";

// Web redirect URL for OAuth and magic link
const getWebRedirectUrl = () => {
  return `${window.location.origin}/auth/callback`;
};

// For OAuth on desktop, we use a hosted callback page that redirects to the deep link
// This is necessary because browsers block direct navigation to custom URL schemes.
// Must come from .env; an empty string disables the OAuth-via-browser path
// while leaving email/password sign-in intact.
const HOSTED_OAUTH_CALLBACK_URL = (import.meta.env.CONNECT_OAUTH_CALLBACK_URL as string) || "";

type AuthCallbackPayload = { type: "code"; code: string } | { type: "error"; message: string };

function parseAuthCallbackUrl(url: string): AuthCallbackPayload | null {
  try {
    const urlObj = new URL(url);
    const hashParams = new URLSearchParams(urlObj.hash.substring(1));

    const error =
      urlObj.searchParams.get("error_description") ??
      urlObj.searchParams.get("error") ??
      hashParams.get("error_description") ??
      hashParams.get("error");
    if (error) {
      return { type: "error", message: error };
    }

    const code = urlObj.searchParams.get("code");
    if (code) {
      return { type: "code", code };
    }

    const hasAccessToken =
      hashParams.has("access_token") || urlObj.searchParams.has("access_token") || false;
    if (hasAccessToken) {
      return {
        type: "error",
        message:
          "Unexpected token callback (access_token). This app expects Auth Code + PKCE; ensure Supabase is configured for PKCE and your hosted callback forwards the ?code=... parameter.",
      };
    }

    return null;
  } catch {
    return null;
  }
}

interface MizanConnectContextValue {
  isEnabled: boolean;
  isConnected: boolean;
  isInitializing: boolean;
  isLoading: boolean;
  isLoadingUserInfo: boolean;
  user: User | null;
  session: Session | null;
  teamId: string | null;
  userInfo: UserInfo | null;
  error: string | null;
  signInWithEmail: (email: string, password: string) => Promise<void>;
  signUpWithEmail: (email: string, password: string) => Promise<void>;
  signInWithOAuth: (provider: "google" | "apple" | "github") => Promise<void>;
  signInWithMagicLink: (email: string) => Promise<void>;
  verifyOtp: (email: string, token: string) => Promise<void>;
  resendConfirmation: (email: string) => Promise<void>;
  signOut: () => Promise<void>;
  clearError: () => void;
  refetchUserInfo: () => Promise<void>;
}

const MizanConnectContext = createContext<MizanConnectContextValue | undefined>(undefined);

// Disabled context value - used when CONNECT_ENABLED is false
// All methods are no-ops that return resolved promises
const disabledContextValue: MizanConnectContextValue = {
  isEnabled: false,
  isConnected: false,
  isInitializing: false,
  isLoading: false,
  isLoadingUserInfo: false,
  user: null,
  session: null,
  teamId: null,
  userInfo: null,
  error: null,
  // Default context — provider not mounted. All methods are intentional
  // no-ops; the real implementations land in the provider's value object.
  signInWithEmail: async () => {
    /* no-op: provider not mounted */
  },
  signUpWithEmail: async () => {
    /* no-op: provider not mounted */
  },
  signInWithOAuth: async () => {
    /* no-op: provider not mounted */
  },
  signInWithMagicLink: async () => {
    /* no-op: provider not mounted */
  },
  verifyOtp: async () => {
    /* no-op: provider not mounted */
  },
  resendConfirmation: async () => {
    /* no-op: provider not mounted */
  },
  signOut: async () => {
    /* no-op: provider not mounted */
  },
  clearError: () => {
    /* no-op: provider not mounted */
  },
  refetchUserInfo: async () => {
    /* no-op: provider not mounted */
  },
};

function getAuthStorageKey(supabaseUrl: string): string {
  try {
    const hostname = new URL(supabaseUrl).hostname;
    const projectRef = hostname.split(".")[0];
    return `sb-${projectRef}-auth-token`;
  } catch {
    return "sb-auth-token";
  }
}

function createHybridPkceStorage(storageKey: string) {
  const inMemory = new Map<string, string>();
  const pkceKey = `${storageKey}-code-verifier`;

  const safeLocalStorageGet = (key: string) => {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  };

  const safeLocalStorageSet = (key: string, value: string) => {
    try {
      localStorage.setItem(key, value);
    } catch {
      // ignore - PKCE exchange will fail after a full redirect without persistence
    }
  };

  const safeLocalStorageRemove = (key: string) => {
    try {
      localStorage.removeItem(key);
    } catch {
      // ignore
    }
  };

  return {
    getItem: (key: string) => {
      if (key === pkceKey) return safeLocalStorageGet(key);
      return inMemory.get(key) ?? null;
    },
    setItem: (key: string, value: string) => {
      if (key === pkceKey) {
        safeLocalStorageSet(key, value);
        return;
      }
      inMemory.set(key, value);
    },
    removeItem: (key: string) => {
      if (key === pkceKey) {
        safeLocalStorageRemove(key);
        return;
      }
      inMemory.delete(key);
    },
  };
}

// Create a Supabase client with custom storage for persistent auth.
// Accepts the resolved config so the same code path works for both
// build-time-baked credentials and runtime-discovered ones (the
// cloud's /api/v1/config/public endpoint).
const createSupabaseClient = (cfg: ResolvedConnectConfig) => {
  const storageKey = getAuthStorageKey(cfg.supabaseUrl);
  return createClient(cfg.supabaseUrl, cfg.supabasePublishableKey, {
    auth: {
      storageKey,
      storage: createHybridPkceStorage(storageKey),
      flowType: "pkce",
      autoRefreshToken: false,
      // Must be true for auth-js to use the provided `storage` (PKCE code_verifier lives there).
      // Our custom storage keeps sessions in-memory (non-persistent) while allowing PKCE to work
      // across full-page redirects.
      persistSession: true,
      detectSessionInUrl: false, // We handle URL parsing manually
    },
  });
};

// Internal provider used when Connect is enabled
function EnabledMizanConnectProvider({
  children,
  resolvedConfig,
}: {
  children: ReactNode;
  resolvedConfig: ResolvedConnectConfig;
}) {
  const { isAuthenticated } = useAuth();
  const queryClient = useQueryClient();
  const [isInitializing, setIsInitializing] = useState(true);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingUserInfo, setIsLoadingUserInfo] = useState(false);
  const [user, setUser] = useState<User | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const supabaseRef = useRef<SupabaseClient | null>(null);

  // Initialize Supabase client — pass the resolved config so we work
  // whether the credentials came from build-time .env or runtime
  // self-discovery via the cloud's /api/v1/config/public endpoint.
  supabaseRef.current ??= createSupabaseClient(resolvedConfig);

  const supabase = supabaseRef.current;

  // Store tokens: refresh token goes to backend (for cloud API calls) and locally (for session restoration)
  const storeTokens = useCallback(async (session: Session | null) => {
    logger.debug(`storeTokens called, isDesktop=${isDesktop}, hasSession=${!!session}`);

    if (!session) {
      // Clear from backend - throw on failure so signOut properly reports errors
      await clearSyncSession();

      // Clear session restoration token from secret store (keyring on desktop, FileSecretStore on web)
      await deleteSecret(REFRESH_TOKEN_KEY).catch((err) => {
        logger.warn(`Failed to delete refresh token: ${err}`);
      });
      return;
    }

    // Store tokens in backend's encrypted secret store (backend can mint fresh access tokens if needed)
    if (session.refresh_token) {
      try {
        await storeSyncSession(session.refresh_token);
      } catch (err) {
        logger.error(`Failed to store tokens in backend: ${err}`);
      }
    }

    // Also store refresh token in secret store for session restoration on app restart
    // Desktop: OS keyring, Web: backend FileSecretStore (both via setSecret command)
    if (session.refresh_token) {
      try {
        await setSecret(REFRESH_TOKEN_KEY, session.refresh_token);
      } catch (err) {
        logger.error(`setSecret failed: ${err}`);
        // Fallback to localStorage only on desktop where keyring might fail
        if (isDesktop) {
          localStorage.setItem(REFRESH_TOKEN_KEY, session.refresh_token);
        }
      }
    }
  }, []);

  // Handle auth callback from URL (deep link or web redirect)
  const handleAuthCallback = useCallback(
    async (url: string) => {
      const payload = parseAuthCallbackUrl(url);

      if (!payload) {
        logger.error("Failed to parse auth callback URL - no payload");
        return;
      }

      if (payload.type === "error") {
        logger.error(`Auth callback error: ${payload.message}`);
        setError(payload.message);
        return;
      }

      try {
        const { data, error: exchangeError } = await supabase.auth.exchangeCodeForSession(
          payload.code,
        );

        if (exchangeError) {
          logger.error(`Failed to exchange auth code: ${exchangeError.message}`);
          setError(exchangeError.message);
          return;
        }

        if (!data.session) {
          logger.error("No session returned after code exchange");
          setError("No session returned after completing sign-in.");
          return;
        }

        // Store tokens BEFORE setting session to avoid race condition
        await storeTokens(data.session);
        setSession(data.session);
        setUser(data.session.user);
        logger.info("Auth callback completed successfully");
      } catch (err) {
        logger.error(`Error in handleAuthCallback: ${err instanceof Error ? err.message : err}`);
        setError(err instanceof Error ? err.message : "Failed to complete sign in");
      }
    },
    [supabase, storeTokens],
  );

  // Restore session from stored tokens on mount
  useEffect(() => {
    let cancelled = false;

    const restoreSession = async () => {
      try {
        // Ask the backend for fresh tokens. The backend is the single owner of
        // the refresh token and will rotate it via Supabase when needed, avoiding
        // the race condition where both the JS client and backend independently
        // rotate the same refresh token.
        try {
          const { accessToken, refreshToken } = await restoreSyncSession();
          const { data, error: setErr } = await supabase.auth.setSession({
            access_token: accessToken,
            refresh_token: refreshToken,
          });
          if (setErr) {
            logger.debug("Failed to set session from backend tokens.");
            await storeTokens(null);
          } else if (data.session && !cancelled) {
            setSession(data.session);
            setUser(data.session.user);
          }
        } catch (_err) {
          // No backend session (not logged in or backend unreachable) — that's fine
          logger.debug("No backend session to restore.");
        }
      } catch (_err) {
        logger.error("Error restoring session.");
      } finally {
        if (!cancelled) {
          setIsInitializing(false);
        }
      }
    };

    void restoreSession();

    // Listen for auth state changes
    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange(async (event, newSession) => {
      if (cancelled) return;

      if (event === "SIGNED_IN" || event === "TOKEN_REFRESHED") {
        // Store tokens BEFORE setting session to avoid race condition
        // where isConnected becomes true before token is in keyring
        await storeTokens(newSession);
        setSession(newSession);
        setUser(newSession?.user ?? null);
      } else if (event === "SIGNED_OUT") {
        setSession(null);
        setUser(null);
        await storeTokens(null);
      }
    });

    return () => {
      cancelled = true;
      subscription.unsubscribe();
    };
  }, [supabase, storeTokens, isAuthenticated]);

  // Listen for deep link events on desktop
  useEffect(() => {
    if (!isDesktop) return;

    let unlistenFn: (() => Promise<void>) | undefined;

    const setupDeepLinkListener = async () => {
      try {
        unlistenFn = await listenDeepLink<string>((event) => {
          const url = event.payload;

          const authPayload = parseAuthCallbackUrl(url);
          if (authPayload) {
            void handleAuthCallback(url);
          }
        });
      } catch (_err) {
        logger.error("Failed to set up deep link listener.");
      }
    };

    void setupDeepLinkListener();

    return () => {
      void unlistenFn?.();
    };
  }, [handleAuthCallback]);

  // Handle auth callback on mount (webview redirect callback)
  // This works for both web and desktop (when OAuth happens in webview)
  useEffect(() => {
    const currentUrl = window.location.href;
    if (parseAuthCallbackUrl(currentUrl)) {
      void handleAuthCallback(currentUrl);
      // Clean up URL after handling
      window.history.replaceState({}, document.title, window.location.pathname);
    }
  }, [handleAuthCallback]);

  // When Plaid Link completes, refresh connection + account data the moment
  // the Tauri window regains focus. Backstop for the in-hook polling loop:
  // catches users who took longer than the 5-minute polling window, or who
  // navigated away from the page that hosted the polling hook.
  // Gated on signed-in: no point invalidating queries that aren't enabled.
  useEffect(() => {
    if (!session) return;
    const onFocus = () => {
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.BROKER_CONNECTIONS] });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.BROKER_ACCOUNTS] });
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [session, queryClient]);

  const signInWithEmail = useCallback(
    async (email: string, password: string) => {
      setIsLoading(true);
      setError(null);

      try {
        const { data, error: signInError } = await supabase.auth.signInWithPassword({
          email,
          password,
        });

        if (signInError) {
          throw signInError;
        }

        if (data.session) {
          // Store tokens BEFORE setting session to avoid race condition
          await storeTokens(data.session);
          setSession(data.session);
          setUser(data.session.user);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Sign in failed";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase, storeTokens],
  );

  const signUpWithEmail = useCallback(
    async (email: string, password: string) => {
      setIsLoading(true);
      setError(null);

      try {
        const { data, error: signUpError } = await supabase.auth.signUp({
          email,
          password,
        });

        if (signUpError) {
          throw signUpError;
        }

        // If email confirmation is not required, user will be signed in
        if (data.session) {
          // Store tokens BEFORE setting session to avoid race condition
          await storeTokens(data.session);
          setSession(data.session);
          setUser(data.session.user);
        } else if (data.user && !data.session) {
          // Email confirmation required
          setError("Please check your email to confirm your account.");
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Sign up failed";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase, storeTokens],
  );

  const signInWithOAuth = useCallback(
    async (provider: "google" | "apple" | "github") => {
      setIsLoading(true);
      setError(null);

      try {
        const isTauri = isDesktop;
        const platform = isTauri ? await getPlatform() : null;
        const isMobile = platform?.is_mobile ?? false;
        const isIOS = platform?.os === "ios";

        // iOS mobile: Use ASWebAuthenticationSession with deep link callback
        // This is required because Google blocks OAuth from embedded webviews (WKWebView)
        // ASWebAuthenticationSession opens a secure Safari sheet that Google accepts
        // Note: This is needed in both dev and prod modes on iOS
        const useASWebAuth = isTauri && isMobile && isIOS;

        // Determine redirect URL based on platform
        // iOS ASWebAuth always needs deep link URL (works in dev and prod)
        // Desktop prod uses hosted callback → deep link (can't use in dev - URL scheme not registered)
        // Dev mode uses webview redirect (simpler, no deep link registration needed)
        const redirectUrl = useASWebAuth
          ? DESKTOP_DEEP_LINK_URL // iOS: direct custom scheme, captured by ASWebAuth
          : isTauri && import.meta.env.PROD
            ? HOSTED_OAUTH_CALLBACK_URL // Desktop & Android: bounce page → mizan://
            : getWebRedirectUrl(); // Web or dev mode

        const useSystemBrowser = isTauri && import.meta.env.PROD && !useASWebAuth;
        const queryParams =
          provider === "google"
            ? {
                // Forces the account chooser instead of silently reusing the last Google session.
                prompt: "select_account",
              }
            : undefined;

        const { data, error: oauthError } = await supabase.auth.signInWithOAuth({
          provider,
          options: {
            skipBrowserRedirect: useSystemBrowser || useASWebAuth,
            redirectTo: redirectUrl,
            queryParams,
          },
        });

        if (oauthError) {
          throw oauthError;
        }

        // iOS mobile: Use ASWebAuthenticationSession plugin
        // This opens a secure Safari sheet that Google accepts for OAuth
        if (useASWebAuth && data.url) {
          try {
            const result = await authenticateWithASWebAuth({
              url: data.url,
              callbackScheme: "mizan",
            });

            // The plugin returns the full callback URL with the auth code
            if (result?.callbackUrl) {
              await handleAuthCallback(result.callbackUrl);
            } else {
              logger.error("No callbackUrl in ASWebAuth result");
            }
          } catch (authErr) {
            // User cancelled or auth failed
            const message =
              authErr instanceof Error ? authErr.message : "Authentication was cancelled";
            logger.error(`ASWebAuth error: ${message}`);
            // Don't throw if user just cancelled
            if (!message.toLowerCase().includes("cancel")) {
              throw authErr;
            }
            logger.info("OAuth authentication was cancelled by user");
          }
          return;
        }

        // Desktop: Open the OAuth URL in the system browser
        if (useSystemBrowser && data.url) {
          await openUrlInBrowser(data.url);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "OAuth sign in failed";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase, handleAuthCallback],
  );

  const signInWithMagicLink = useCallback(
    async (email: string) => {
      setIsLoading(true);
      setError(null);

      try {
        const isTauri = isDesktop;
        const platform = isTauri ? await getPlatform() : null;
        const isMobile = platform?.is_mobile ?? false;

        const redirectUrl =
          isTauri && import.meta.env.PROD
            ? isMobile
              ? HOSTED_OAUTH_CALLBACK_URL // Mobile: bounce page → mizan://
              : DESKTOP_DEEP_LINK_URL // Desktop: direct mizan:// from email client
            : getWebRedirectUrl();

        const { error: otpError } = await supabase.auth.signInWithOtp({
          email,
          options: {
            // Redirect URL for when user clicks the magic link
            emailRedirectTo: redirectUrl,
          },
        });

        if (otpError) {
          throw otpError;
        }

        // Don't throw error - magic link sent successfully
        // The UI will handle showing success message
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to send magic link";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase],
  );

  const verifyOtp = useCallback(
    async (email: string, token: string) => {
      setIsLoading(true);
      setError(null);

      try {
        const { data, error: verifyError } = await supabase.auth.verifyOtp({
          email,
          token,
          type: "email",
        });

        if (verifyError) {
          throw verifyError;
        }

        if (data.session) {
          // Store tokens BEFORE setting session to avoid race condition
          await storeTokens(data.session);
          setSession(data.session);
          setUser(data.session.user);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Invalid verification code";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase, storeTokens],
  );

  /**
   * Resend the confirmation email for an unconfirmed signup.
   *
   * Used by the LoginForm when sign-in fails with `email_not_confirmed`:
   * the user gets an inline "Resend confirmation" action that calls this.
   * Mirrors `signInWithMagicLink` shape — surfaces Supabase's error
   * message via `setError` and re-throws so the form can render its
   * own success state.
   */
  const resendConfirmation = useCallback(
    async (email: string) => {
      setIsLoading(true);
      setError(null);

      try {
        const { error: resendError } = await supabase.auth.resend({
          type: "signup",
          email,
        });
        if (resendError) {
          throw resendError;
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to resend confirmation email";
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [supabase],
  );

  const signOut = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Server-side invalidation may fail if the access token expired (since
      // autoRefreshToken is off). That's acceptable — the session will expire
      // naturally on the server. Always clear local state regardless.
      const { error: signOutError } = await supabase.auth.signOut();
      if (signOutError) {
        logger.warn(`Server-side sign out failed (token may be expired): ${signOutError.message}`);
      }

      setSession(null);
      setUser(null);
      await storeTokens(null);
    } catch (err) {
      // Still clear local state even on unexpected errors
      setSession(null);
      setUser(null);
      await storeTokens(null).catch((tokenErr) => {
        // Best-effort: we're already in the error path. Surface this
        // failure separately so it doesn't mask the original sign-out
        // error (preserved via setError/throw below).
        console.error("storeTokens cleanup after sign-out failure also failed:", tokenErr);
      });
      const message = err instanceof Error ? err.message : "Sign out failed";
      setError(message);
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [supabase, storeTokens]);

  const clearError = useCallback(() => setError(null), []);

  // Fetch user info from the cloud API
  const refetchUserInfo = useCallback(async () => {
    if (!session) {
      setUserInfo(null);
      setIsLoadingUserInfo(false);
      return;
    }

    setIsLoadingUserInfo(true);
    setError(null);

    try {
      const info = await getUserInfo();
      setUserInfo(info);
    } catch (err) {
      logger.error("Failed to fetch user info from API.");
      setUserInfo(null);
      const message = err instanceof Error ? err.message : "Failed to fetch user info";
      setError(message);
    } finally {
      setIsLoadingUserInfo(false);
    }
  }, [session]);

  // Fetch user info when session changes
  useEffect(() => {
    if (session) {
      void refetchUserInfo();
    } else {
      setUserInfo(null);
    }
  }, [session, refetchUserInfo]);

  // Extract team_id from user's app_metadata
  const teamId = useMemo(() => {
    return (user?.app_metadata?.team_id as string | undefined) ?? null;
  }, [user]);

  const value = useMemo<MizanConnectContextValue>(
    () => ({
      isEnabled: true,
      isConnected: !!session,
      isInitializing,
      isLoading,
      isLoadingUserInfo,
      user,
      session,
      teamId,
      userInfo,
      error,
      signInWithEmail,
      signUpWithEmail,
      signInWithOAuth,
      signInWithMagicLink,
      verifyOtp,
      resendConfirmation,
      signOut,
      clearError,
      refetchUserInfo,
    }),
    [
      session,
      isInitializing,
      isLoading,
      isLoadingUserInfo,
      user,
      teamId,
      userInfo,
      error,
      signInWithEmail,
      signUpWithEmail,
      signInWithOAuth,
      signInWithMagicLink,
      verifyOtp,
      resendConfirmation,
      signOut,
      clearError,
      refetchUserInfo,
    ],
  );

  return <MizanConnectContext.Provider value={value}>{children}</MizanConnectContext.Provider>;
}

// Main provider that chooses enabled/disabled path based on the
// resolved Connect config + the platform's cloud_sync capability.
//
// Resolution order:
//   1. Await resolveConnectConfig() — build-time wins; runtime fallback
//      hits /api/v1/config/public when build-time creds are missing.
//   2. Await getPlatform() — some platforms (web build, headless test
//      runners) opt out of cloud sync entirely.
//   3. If both succeed → mount EnabledMizanConnectProvider with the
//      resolved credentials. Otherwise → mount the disabled context.
//
// During resolution we render the disabled context with
// isInitializing=true so no Connect UI flashes a misleading state
// (showing "sign in" then yanking it away after the fetch).
export function MizanConnectProvider({ children }: { children: ReactNode }) {
  const [resolvedConfig, setResolvedConfig] = useState<ResolvedConnectConfig | null>(null);
  const [resolveComplete, setResolveComplete] = useState(false);
  const [isCloudSyncAvailable, setIsCloudSyncAvailable] = useState(false);
  const [isCapabilityCheckComplete, setIsCapabilityCheckComplete] = useState(false);

  // Step 1: resolve the Connect config (build-time or runtime).
  useEffect(() => {
    let cancelled = false;
    void resolveConnectConfig()
      .then((state) => {
        if (cancelled) return;
        if (state.enabled) {
          setResolvedConfig(state.config);
        } else {
          // Log the reason so support can diagnose without log-diving.
          console.info(`[MizanConnect] disabled: ${state.reason}`);
        }
      })
      .finally(() => {
        if (cancelled) return;
        setResolveComplete(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Step 2: capability check — only once config has resolved to an
  // enabled state. No point asking the platform if the config can't
  // even produce a Supabase client.
  useEffect(() => {
    if (!resolveComplete) return;
    if (!resolvedConfig) {
      setIsCapabilityCheckComplete(true);
      return;
    }

    let cancelled = false;
    void getPlatform()
      .then((platform) => {
        if (cancelled) return;
        setIsCloudSyncAvailable(
          platform.capabilities?.cloud_sync ?? platform.capabilities?.connect_sync ?? true,
        );
      })
      .catch(() => {
        if (cancelled) return;
        setIsCloudSyncAvailable(true);
      })
      .finally(() => {
        if (cancelled) return;
        setIsCapabilityCheckComplete(true);
      });

    return () => {
      cancelled = true;
    };
  }, [resolveComplete, resolvedConfig]);

  // Initial loading state: spinner instead of "sign in" until both
  // resolution + capability check finish.
  if (!resolveComplete || !isCapabilityCheckComplete) {
    return (
      <MizanConnectContext.Provider
        value={{
          ...disabledContextValue,
          isEnabled: true,
          isInitializing: true,
        }}
      >
        {children}
      </MizanConnectContext.Provider>
    );
  }

  if (!resolvedConfig || !isCloudSyncAvailable) {
    return (
      <MizanConnectContext.Provider value={disabledContextValue}>
        {children}
      </MizanConnectContext.Provider>
    );
  }

  return (
    <EnabledMizanConnectProvider resolvedConfig={resolvedConfig}>
      {children}
    </EnabledMizanConnectProvider>
  );
}

export const useMizanConnect = () => {
  const ctx = useContext(MizanConnectContext);
  if (!ctx) {
    throw new Error("useMizanConnect must be used within a MizanConnectProvider");
  }
  return ctx;
};
