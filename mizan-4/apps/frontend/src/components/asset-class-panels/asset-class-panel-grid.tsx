/**
 * Twelve-panel dashboard grid — Track A PR-A4.
 *
 * Renders the fixed §3(e) twelve in dashboard order. `'other'` appends
 * only when non-empty per [`rollupHoldingsByPanel`].
 *
 * Skeleton only — header + total + 24h + count + chevron per Goal §V.A4.
 * Sparkline + 30d delta wire in PR-A6 alongside the price-history feed.
 *
 * This is the §23 step "He taps the Sukuks panel" surface — the tap target
 * is wired even before the Track B dedicated panel pages land; the route
 * upgrades panel-by-panel as PR-B1..B7 ship.
 *
 * PR-POLISH-5 — staggered entry on dashboard load (fade-in + 8px
 * translateY, 30ms each). Honors `prefers-reduced-motion`.
 */
import { useMemo } from "react";
import { motion, useReducedMotion } from "motion/react";

import type { Holding } from "@/lib/types";
import { fadeInUp, staggerContainer } from "@/lib/motion";

import { AssetClassPanelCard } from "./asset-class-panel-card";
import {
  ASSET_CLASS_PANELS,
  getPanelDescriptor,
  rollupHoldingsByPanel,
  type AssetClassPanelRollup,
} from "./taxonomy";

export interface AssetClassPanelGridProps {
  holdings: readonly Holding[];
  baseCurrency: string;
  isPrivacyMode?: boolean;
  /**
   * When true, panels with zero holdings render in a dimmed state.
   * When false (the default), they still render at full opacity so
   * the user discovers what's available.
   */
  dimEmptyPanels?: boolean;
}

export function AssetClassPanelGrid({
  holdings,
  baseCurrency,
  isPrivacyMode = false,
  dimEmptyPanels = false,
}: AssetClassPanelGridProps) {
  const rollups = useMemo<AssetClassPanelRollup[]>(
    () => rollupHoldingsByPanel(holdings, baseCurrency),
    [holdings, baseCurrency],
  );

  // Respect OS-level "reduce motion" — when on, the staggered entry
  // collapses to a no-op so tiles render instantly without
  // translation or fade.
  const shouldReduceMotion = useReducedMotion();

  return (
    // PR-DENSITY-1 — auto-fill columns (min 240px / max 320px) so
    // wide displays pack 5 columns, standard 4, medium 3, narrow 2,
    // mobile 1. Fixed `grid-cols-*` breakpoints would either feel
    // cramped at one width or sparse at the next.
    //
    // Gap 8px (was 12) so 12 tiles read as one dense composition
    // rather than 12 separate cards.
    <motion.section
      aria-label="Asset class panels"
      className="grid gap-2"
      style={{ gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))" }}
      variants={shouldReduceMotion ? undefined : staggerContainer}
      initial="initial"
      animate="animate"
    >
      {rollups.map((rollup) => {
        const descriptor = getPanelDescriptor(rollup.panelId);
        const dimmed = dimEmptyPanels && rollup.holdingsCount === 0;
        return (
          <motion.div
            key={descriptor.id}
            variants={shouldReduceMotion ? undefined : fadeInUp}
            className={dimmed ? "opacity-50 transition-opacity" : "transition-opacity"}
          >
            <AssetClassPanelCard
              descriptor={descriptor}
              rollup={rollup}
              isPrivacyMode={isPrivacyMode}
            />
          </motion.div>
        );
      })}
    </motion.section>
  );
}

export { ASSET_CLASS_PANELS };
