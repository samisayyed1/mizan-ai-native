import { cn } from "@/lib/cn";

/**
 * Eyebrow — t-micro caps, gold-deep by default.
 *
 * A11y note: gold-deep (#8B6F47) on depth-page (#0B0B0B) sits at
 * ~4.1:1 contrast — passes WCAG AA at ≥14px bold but NOT at 11px
 * regular. The component swaps to gold-primary (8.4:1) when small,
 * keeping AA across surfaces without per-caller plumbing.
 *
 * The `tone="primary"` prop forces the gold-primary variant for the
 * Zakat section eyebrow + any spot where the eyebrow needs to read
 * louder than the surrounding chrome.
 */
export function Eyebrow({
  children,
  className,
  tone = "deep",
  as: As = "div",
}: {
  children: React.ReactNode;
  className?: string;
  tone?: "deep" | "primary";
  as?: "div" | "span" | "p" | "h2" | "h3";
}) {
  return (
    <As
      className={cn(
        "t-micro",
        // Force gold-primary when small (t-micro = 11px) per the
        // contrast rule above, even when caller asked for "deep".
        // Override only on opt-in card surfaces by passing
        // `text-gold-deep` in className.
        tone === "primary" ? "text-gold-primary" : "text-gold-primary",
        className,
      )}
    >
      {children}
    </As>
  );
}
