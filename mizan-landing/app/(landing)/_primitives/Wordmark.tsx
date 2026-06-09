/**
 * Mizan wordmark — Merriweather serif bold, with the dot of the "i"
 * replaced by a gold-primary circle so the brand mark carries the
 * gold accent at every size.
 *
 * Pattern mirrors the desktop sidebar's wordmark at
 * `mizan-4/apps/frontend/src/pages/layouts/navigation/app-sidebar.tsx`
 * (lines 62–72) — `font-serif font-bold tracking-tight` + gold tittle.
 */
import { cn } from "@/lib/cn";

type WordmarkSize = "sm" | "lg";

interface WordmarkProps {
  size?: WordmarkSize;
  className?: string;
}

const SIZE_TO_CLASS: Record<WordmarkSize, string> = {
  // Header bar — 18px (Tailwind text-lg = 18px).
  sm: "text-lg",
  // Hero / footer accent — fluid clamp, lands at 56–72px on desktop.
  lg: "text-[clamp(40px,6vw,72px)] leading-[1.05]",
};

const DOT_TO_CLASS: Record<WordmarkSize, string> = {
  // 5px dot for the 18px size.
  sm: "h-[5px] w-[5px] top-[6px] right-[1px]",
  // ~14px dot for the clamp size — sits above the "i" stem visually.
  lg: "h-[clamp(8px,1.1vw,14px)] w-[clamp(8px,1.1vw,14px)] top-[clamp(6px,0.85vw,10px)] right-[clamp(1px,0.2vw,3px)]",
};

export function Wordmark({ size = "sm", className }: WordmarkProps) {
  return (
    <span
      aria-label="Mizan"
      className={cn(
        "relative inline-flex items-baseline font-serif font-bold tracking-tight text-foreground/95",
        SIZE_TO_CLASS[size],
        className,
      )}
    >
      <span aria-hidden="true">M</span>
      <span aria-hidden="true" className="relative">
        i
        {/* The tittle: a gold-primary circle positioned where the dot
            of the "i" naturally sits. Sized + offset proportionally per
            wordmark size so it tracks the glyph. */}
        <span
          aria-hidden="true"
          className={cn(
            "absolute rounded-full bg-gold-primary",
            DOT_TO_CLASS[size],
          )}
        />
      </span>
      <span aria-hidden="true">zan</span>
    </span>
  );
}
