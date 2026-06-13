import { cn } from "@/lib/cn";

/**
 * Section wrapper enforcing the landing's vertical rhythm:
 *   - 64px top / 80px bottom mobile
 *   - 96px top / 128px bottom desktop
 * Matches the desktop's PR-DENSITY-7 8-pt spacing grid.
 */
export function Section({
  children,
  id,
  className,
  background = "page",
  topBorder = false,
  bottomBorder = false,
}: {
  children: React.ReactNode;
  id?: string;
  className?: string;
  background?: "page" | "container" | "card";
  topBorder?: boolean;
  bottomBorder?: boolean;
}) {
  const bgClass = {
    page: "bg-depth-page",
    container: "bg-depth-container",
    card: "bg-depth-card",
  }[background];

  return (
    <section
      id={id}
      className={cn(
        "relative w-full py-16 md:py-32",
        bgClass,
        topBorder && "border-t border-depth-border",
        bottomBorder && "border-b border-depth-border",
        className,
      )}
    >
      {children}
    </section>
  );
}
