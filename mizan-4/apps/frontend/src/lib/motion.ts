/**
 * Centralised motion constants — PR-POLISH-5.
 *
 * Linear / Stripe / Arc / Robinhood feel: motion is purposeful, fast,
 * and never distracts. Durations stay in the 150–400ms band; easings
 * favour `easeOut` so animations decelerate into rest.
 *
 * Every motion-consuming component must respect
 * `prefers-reduced-motion: reduce` — the user-supplied OS-level
 * accessibility flag. The `useReducedMotion()` hook from `motion/react`
 * (or `framer-motion`) returns true when the user has the setting on;
 * call sites should fall back to 0-duration transitions when so.
 */

import type { Easing, Transition, Variants } from "motion/react";

export const MOTION_DURATIONS = {
  /** Card hover background tint. */
  hover: 0.15,
  /** Tap-press feedback (touch + click). */
  tap: 0.1,
  /** Tile entry / page transition. */
  enter: 0.25,
  /** Number value cross-fade. */
  numberChange: 0.4,
} as const;

/** easeOutQuad / cubic-bezier(.25,.46,.45,.94) — softens at the end. */
export const MOTION_EASE: Easing = [0.25, 0.46, 0.45, 0.94];

/**
 * Stagger delay between sibling entries (e.g. tiles in the asset
 * class grid). 30ms each → ~360ms total for 12 tiles, well under the
 * §A19 cold-start budget and well above flicker.
 */
export const MOTION_STAGGER = 0.03;

/**
 * Reusable variants for a fade-in + small upward translate. Used by
 * the asset class panel grid's entry animation. Each item starts
 * 8px below its resting position and at opacity 0, then settles
 * into place.
 */
export const fadeInUp: Variants = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
};

/**
 * Container variants that drive a staggered reveal of children using
 * the `fadeInUp` item variants above. Use:
 *
 *   <motion.div variants={staggerContainer} initial="initial" animate="animate">
 *     {items.map(it => (
 *       <motion.div key={it.id} variants={fadeInUp}>...</motion.div>
 *     ))}
 *   </motion.div>
 */
export const staggerContainer: Variants = {
  initial: {},
  animate: {
    transition: {
      staggerChildren: MOTION_STAGGER,
    },
  },
};

/** Standard transition for any one-shot animation. */
export const standardTransition: Transition = {
  duration: MOTION_DURATIONS.enter,
  ease: MOTION_EASE,
};
