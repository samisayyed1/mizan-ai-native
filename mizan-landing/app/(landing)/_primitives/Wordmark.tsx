/**
 * Mizan wordmark — refined version of the desktop app icon. A bold
 * gold-gradient "M" set inside a thin gold ring on a dark disc,
 * followed by "Mizan" in Merriweather serif bold.
 *
 * The mark is drawn inline as SVG so it stays crisp at every size,
 * doesn't trigger an extra request, and keeps perfect parity with the
 * desktop `apps/frontend/public/logo.png`.
 */
import { cn } from "@/lib/cn";

type WordmarkSize = "sm" | "lg";

interface WordmarkProps {
  size?: WordmarkSize;
  className?: string;
}

const TEXT_SIZE: Record<WordmarkSize, string> = {
  sm: "text-lg",
  lg: "text-[clamp(40px,6vw,72px)] leading-[1.05]",
};

const MARK_PX: Record<WordmarkSize, number> = {
  sm: 28,
  lg: 56,
};

function MizanMark({ size }: { size: WordmarkSize }) {
  const px = MARK_PX[size];
  return (
    <svg
      width={px}
      height={px}
      viewBox="0 0 64 64"
      fill="none"
      aria-hidden="true"
      className="shrink-0"
    >
      <defs>
        <linearGradient id="mizan-gold-grad" x1="0" y1="0" x2="64" y2="64" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="hsl(40 67% 87%)" />
          <stop offset="55%" stopColor="hsl(31 49% 64%)" />
          <stop offset="100%" stopColor="hsl(31 32% 41%)" />
        </linearGradient>
        <radialGradient id="mizan-disc-grad" cx="50%" cy="40%" r="60%">
          <stop offset="0%" stopColor="hsl(0 0% 11%)" />
          <stop offset="100%" stopColor="hsl(0 0% 5%)" />
        </radialGradient>
      </defs>
      {/* Dark disc with subtle gradient — matches desktop app icon. */}
      <rect x="1" y="1" width="62" height="62" rx="14" fill="url(#mizan-disc-grad)" stroke="hsl(31 32% 41% / 0.35)" strokeWidth="0.5" />
      {/* Thin ambient ring */}
      <circle cx="32" cy="32" r="22" fill="none" stroke="url(#mizan-gold-grad)" strokeWidth="0.6" opacity="0.4" />
      {/* The M — geometric, bold, gold-gradient */}
      <path
        d="M 18 46 L 18 18 L 24 18 L 32 32 L 40 18 L 46 18 L 46 46 L 41 46 L 41 27 L 34 39 L 30 39 L 23 27 L 23 46 Z"
        fill="url(#mizan-gold-grad)"
      />
    </svg>
  );
}

export function Wordmark({ size = "sm", className }: WordmarkProps) {
  const gap = size === "sm" ? "gap-2.5" : "gap-4";
  return (
    <span
      aria-label="Mizan"
      className={cn(
        "inline-flex items-center font-serif font-bold tracking-tight",
        gap,
        TEXT_SIZE[size],
        className,
      )}
    >
      <MizanMark size={size} />
      <span
        aria-hidden="true"
        className={cn(
          size === "lg"
            ? "bg-gradient-to-br from-gold-cream via-gold-primary to-gold-deep bg-clip-text text-transparent"
            : "text-foreground/95",
        )}
      >
        Mizan
      </span>
    </span>
  );
}
