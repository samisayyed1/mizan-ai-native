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
    <motion.section
      aria-label="Asset class panels"
      className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-2 xl:grid-cols-3"
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
