import { Button } from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Component, type ErrorInfo, type ReactNode } from "react";

import { logger } from "@/adapters";

/**
 * Per-page error boundary — caught errors render an inline recovery
 * card without taking down the rest of the app shell.
 *
 * Why we need both this AND `RootErrorBoundary`:
 *
 * - The root boundary catches anything that escapes a page and shows a
 *   full-screen recovery surface. Good for catastrophic failures (the
 *   webview itself broke), bad for a single page's render bug — the
 *   user loses access to the sidebar nav and top bar and can't even
 *   browse to a different page.
 *
 * - This page-scoped boundary catches a single page's render throw and
 *   leaves the surrounding `<AppLayout>` chrome (sidebar, top bar,
 *   notification bell) fully interactive. The user can navigate to
 *   another page and try again, or stay and reload just this page.
 *
 * Pages that depend on networked data, query-cached responses, or
 * intricate render trees (Goals, Retire planner, holdings, news) should
 * wrap their content in this boundary so a single bad row doesn't
 * black out the whole surface.
 */
interface PageErrorBoundaryProps {
  /** Friendly name shown in the recovery card (e.g. "Goals", "Retire planner"). */
  pageName: string;
  children: ReactNode;
}

interface PageErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class PageErrorBoundary extends Component<PageErrorBoundaryProps, PageErrorBoundaryState> {
  override state: PageErrorBoundaryState = {
    hasError: false,
    error: null,
    errorInfo: null,
  };

  static getDerivedStateFromError(error: Error): Partial<PageErrorBoundaryState> {
    return { hasError: true, error };
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    logger.error(
      `${this.props.pageName} page error: ${error.name}: ${error.message}\nstack: ${
        error.stack ?? "<none>"
      }\ncomponent stack:${errorInfo.componentStack ?? "<none>"}`,
    );
    this.setState({ errorInfo });
  }

  private handleRetry = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  private handleReload = () => {
    window.location.reload();
  };

  override render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const { error } = this.state;
    const { pageName } = this.props;

    return (
      <div
        role="alert"
        className="flex min-h-[60vh] items-center justify-center px-4 py-12"
      >
        <div className="bg-card text-card-foreground w-full max-w-md rounded-2xl border p-6 shadow-sm">
          <div className="bg-destructive/10 text-destructive mb-4 inline-flex h-10 w-10 items-center justify-center rounded-full">
            <Icons.AlertTriangle className="h-5 w-5" />
          </div>
          <h2 className="text-foreground text-lg font-semibold">
            {pageName} hit an unexpected error
          </h2>
          <p className="text-muted-foreground mt-2 text-sm leading-relaxed">
            Your data is safe — only the {pageName.toLowerCase()} view crashed. Try
            again, or jump back to another section.
          </p>
          {error ? (
            <pre className="bg-muted/40 text-muted-foreground mt-3 max-h-32 overflow-auto rounded-lg p-3 font-mono text-xs">
              {error.name}: {error.message}
            </pre>
          ) : null}
          <div className="mt-5 flex flex-wrap gap-2">
            <Button size="sm" onClick={this.handleRetry}>
              Try again
            </Button>
            <Button size="sm" variant="outline" onClick={this.handleReload}>
              Reload Mizan
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
