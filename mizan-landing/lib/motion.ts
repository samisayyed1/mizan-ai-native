/**
 * Motion constants — verbatim port from
 * `mizan-4/apps/frontend/src/lib/motion.ts` lines 17–36.
 *
 * Keeping these synchronized means landing-page motion feels
 * indistinguishable from the desktop's motion.
 */
import type { Easing, Transition, Variants } from "framer-motion";

export const MOTION_DURATIONS = {
  hover: 0.15,
  tap: 0.1,
  enter: 0.25,
  numberChange: 0.4,
} as const;

export const MOTION_EASE: Easing = [0.25, 0.46, 0.45, 0.94];
export const MOTION_STAGGER = 0.03;

export const fadeInUp: Variants = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
};

export const staggerContainer: Variants = {
  initial: {},
  animate: {
    transition: {
      staggerChildren: MOTION_STAGGER,
    },
  },
};

export const standardTransition: Transition = {
  duration: MOTION_DURATIONS.enter,
  ease: MOTION_EASE,
};
