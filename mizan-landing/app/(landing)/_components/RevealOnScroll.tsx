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
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
  as?: "div" | "section" | "ul" | "ol";
  stagger?: boolean;
}) {
  const reduce = useReducedMotion();

  if (reduce) {
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
      // Reveal is a transform-only lift — content starts at opacity 1
      // so any failure mode (JS off, slow hydration, IntersectionObserver
      // not firing fast enough during programmatic scroll) still shows
      // text. The y-translate is a small bit of polish, not a gate.
      initial={{ opacity: 1, y: 12 }}
      whileInView={{ opacity: 1, y: 0 }}
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
        initial: { opacity: 1, y: 8 },
        animate: { opacity: 1, y: 0 },
      }}
      className={className}
    >
      {children}
    </motion.div>
  );
}
