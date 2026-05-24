import { Component, type ErrorInfo, type ReactNode } from "react";

import { logger } from "@/adapters";
import { Button } from "@mizan/ui";

/**
 * Top-level error boundary wrapping the entire application.
 *
 * The previous behaviour was: any uncaught React render error
 * surfaced as a white screen with no recovery path — the user had to
 * force-quit the desktop app and reopen. This component renders a
 * branded recovery screen with two clear paths:
 *
 *   * **Reload** — calls `window.location.reload()` to discard the
 *     bad render tree and try again. Works for transient errors
 *     (data fetched in a weird order, race condition during state
 *     update, etc.).
 *   * **Report** — copies a structured diagnostic blob to the
 *     clipboard so the user can paste it into a support email. The
 *     blob includes error name/message/stack, component stack, and
 *     a timestamp, but **not** any portfolio data — diagnostic only.
 *
 * Errors are also dispatched to `logger.error` so they land in the
 * Tauri log file (which itself goes through the redaction filter on
 * the Rust side).
 *
 * `componentDidCatch` is a React class-only API; the boundary has to
 * be a class. The rest of the codebase is functional/hooks-based;
 * this is the one defensible exception.
 */
interface RootErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class RootErrorBoundary extends Component<{ children: ReactNode }, RootErrorBoundaryState> {
  override state: RootErrorBoundaryState = {
    hasError: false,
    error: null,
    errorInfo: null,
  };

  static getDerivedStateFromError(error: Error): Partial<RootErrorBoundaryState> {
    return { hasError: true, error };
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    // Send to the Tauri log file via the adapter logger. The Rust-side
    // log_redaction filter scrubs any sensitive substrings that might
    // sneak into the stack trace before persisting.
    logger.error(
      `Unhandled React error: ${error.name}: ${error.message}\nstack: ${
        error.stack ?? "<none>"
      }\ncomponent stack:${errorInfo.componentStack ?? "<none>"}`,
    );
    this.setState({ errorInfo });
  }

  private handleReload = () => {
    window.location.reload();
  };

  private handleCopyDiagnostic = async () => {
    const { error, errorInfo } = this.state;
    const blob = [
      "Mizan diagnostic report",
      `Timestamp: ${new Date().toISOString()}`,
      `Error: ${error?.name ?? "Unknown"}: ${error?.message ?? "<no message>"}`,
      "",
      "Stack:",
      error?.stack ?? "<no stack>",
      "",
      "Component stack:",
      errorInfo?.componentStack ?? "<no component stack>",
    ].join("\n");
    try {
      await navigator.clipboard.writeText(blob);
    } catch {
      // Clipboard API can be unavailable in some Tauri webviews; fall
      // back to a textarea + select.
      const ta = document.createElement("textarea");
      ta.value = blob;
      document.body.appendChild(ta);
      ta.select();
      try {
        document.execCommand("copy");
      } catch {
        /* swallow — user can still see the error onscreen */
      }
      document.body.removeChild(ta);
    }
  };

  override render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const { error } = this.state;

    return (
      <div
        role="alert"
        className="bg-background text-foreground flex min-h-screen items-center justify-center p-6"
      >
        <div className="bg-card text-card-foreground w-full max-w-lg rounded-2xl border p-6 shadow-sm">
          <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
            Mizan ran into a problem
          </p>
          <h1 className="mt-2 text-2xl font-semibold">Something went wrong.</h1>
          <p className="text-muted-foreground mt-3 text-sm leading-relaxed">
            The current page hit an unexpected error and couldn&apos;t finish rendering. Your data
            is safe — only the in-memory view crashed. Reload the app to recover. If this keeps
            happening, copy the diagnostic report below and send it over.
          </p>
          {error ? (
            <pre className="bg-muted/40 text-muted-foreground mt-4 max-h-40 overflow-auto rounded-lg p-3 font-mono text-xs">
              {error.name}: {error.message}
            </pre>
          ) : null}
          <div className="mt-6 flex flex-wrap gap-2">
            <Button onClick={this.handleReload}>Reload Mizan</Button>
            <Button variant="outline" onClick={this.handleCopyDiagnostic}>
              Copy diagnostic
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
