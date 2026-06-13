"use client";

import { motion, useReducedMotion } from "framer-motion";
import { type ReactNode } from "react";

import {
  MOTION_DURATIONS,
  MOTION_EASE,
  MOTION_STAGGER,
} from "@/lib/motion";

/**
 * Single client wrapper that drives every in-view reveal across the
 * landing. Keeps section shells as server components by quarantining
 * the framer-motion import to this island.
 */
export function RevealOnScroll({
  children,
  delay = 0,
  className,
  as: As = "div",
  stagger = false,
  immediate = false,
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
  as?: "div" | "section" | "ul" | "ol";
  stagger?: boolean;
  // Render content visible immediately (no fade). Use for above-the-fold
  // content like the hero so the LCP element paints instantly.
  immediate?: boolean;
}) {
  const reduce = useReducedMotion();

  if (reduce || immediate) {
    return <As className={className}>{children}</As>;
  }

  const Component =
    As === "section"
      ? motion.section
      : As === "ul"
        ? motion.ul
        : As === "ol"
          ? motion.ol
          : motion.div;

  return (
    <Component
      // Opacity-only reveal. No transform → zero layout shift, so anchor
      // jumps (#product / #waitlist) never "jerk" or shake on landing,
      // and the reveal never fights a smooth scroll. Opacity is fully
      // GPU-composited, so the first scroll-through stays buttery.
      initial={{ opacity: 0 }}
      whileInView={{ opacity: 1 }}
      viewport={{ once: true, amount: 0.05 }}
      transition={{
        duration: MOTION_DURATIONS.enter,
        ease: MOTION_EASE,
        delay,
        ...(stagger
          ? { staggerChildren: MOTION_STAGGER, delayChildren: 0.05 }
          : {}),
      }}
      className={className}
    >
      {children}
    </Component>
  );
}

/** Item-level reveal — use as a child of a `<RevealOnScroll stagger>`. */
export function RevealItem({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <motion.div
      variants={{
        initial: { opacity: 0 },
        animate: { opacity: 1 },
      }}
      className={className}
    >
      {children}
    </motion.div>
  );
}
