"use client";

import { motion, useReducedMotion } from "framer-motion";
import { ChevronDown } from "lucide-react";

/**
 * Hero scroll indicator — gentle 2s y-bounce, opacity 0.4. Collapses
 * to a static chevron when the user prefers reduced motion.
 */
export function ScrollIndicator() {
  const reduce = useReducedMotion();
  return (
    <motion.div
      aria-hidden="true"
      className="text-foreground/40"
      initial={{ opacity: 0 }}
      animate={reduce ? { opacity: 0.4 } : { opacity: 0.4, y: [0, 6, 0] }}
      transition={
        reduce
          ? { duration: 0 }
          : { duration: 2, repeat: Infinity, ease: "easeInOut" }
      }
    >
      <ChevronDown className="h-5 w-5" />
    </motion.div>
  );
}
