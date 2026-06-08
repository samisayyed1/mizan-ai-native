import { useEffect } from "react";

/**
 * Set the browser/OS chrome title to `Mizan — {pageName}` while the
 * caller is mounted. Restores the previous title on unmount so back
 * navigation doesn't strand a stale title.
 *
 * Spec §13 / PR-POLISH-6: every meaningful page in Mizan owns its
 * title slot so the dock / taskbar / tab bar consistently reads
 * "Mizan — Dashboard", "Mizan — Net Worth", "Mizan — Goals", etc.
 *
 * Usage:
 *
 *   useDocumentTitle("Dashboard");
 *   useDocumentTitle("Net Worth");
 *   useDocumentTitle(`Goal: ${goal.name}`);
 *
 * When `pageName` is empty/null/undefined, the title falls back to
 * the bare "Mizan" — useful for the splash / unauthenticated
 * states.
 */
export function useDocumentTitle(pageName: string | null | undefined): void {
  useEffect(() => {
    const previous = document.title;
    document.title = pageName ? `Mizan — ${pageName}` : "Mizan";
    return () => {
      document.title = previous;
    };
  }, [pageName]);
}
