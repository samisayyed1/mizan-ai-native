"use client";

import { useEffect, useState } from "react";

/**
 * Social proof counter — 847 → 897 organic drift over the page session.
 *
 * Increments by 1 every 28-42s (randomized per tick so it feels
 * organic, not metronomic), caps at +50 from the seed. Pauses when
 * the tab is backgrounded so we don't burn battery + don't spike
 * artificially when a user comes back.
 */
const SEED = 847;
const MAX_DELTA = 50;

export function SocialProofCounter() {
  const [count, setCount] = useState(SEED);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    const schedule = () => {
      // 28-42s sweet spot — frequent enough that an attentive reader
      // catches it once, slow enough to feel real.
      const delay = 28_000 + Math.random() * 14_000;
      timer = setTimeout(() => {
        if (document.hidden) {
          // Skip the tick but reschedule so we resume cleanly when the
          // user comes back.
          schedule();
          return;
        }
        setCount((c) => Math.min(SEED + MAX_DELTA, c + 1));
        schedule();
      }, delay);
    };

    schedule();
    return () => {
      if (timer) clearTimeout(timer);
    };
  }, []);

  const seatsRemaining = Math.max(0, 1000 - count);
  return (
    <p
      className="t-caption text-foreground/60 inline-flex items-center gap-2"
      aria-live="polite"
    >
      <span
        aria-hidden="true"
        className="inline-block h-1.5 w-1.5 rounded-full bg-gold-primary"
        style={{ animation: "pulse 2s ease-in-out infinite" }}
      />
      <span className="tabular-nums">{count}</span> founding members reserved ·{" "}
      <span className="tabular-nums">{seatsRemaining}</span> seats remaining
      <style jsx>{`
        @keyframes pulse {
          0%,
          100% {
            opacity: 0.6;
            transform: scale(1);
          }
          50% {
            opacity: 1;
            transform: scale(1.15);
          }
        }
      `}</style>
    </p>
  );
}
