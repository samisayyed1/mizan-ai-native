/**
 * Twelve-panel asset class skeleton — Track A PR-A4 / ADR 0021.
 *
 * Public exports for the dashboard composition layer. Internal
 * helpers (descriptor lookup, rollup math) are re-exported so
 * Track B's per-panel surfaces can reuse the taxonomy without
 * duplicating the classifier logic.
 */
export { AssetClassPanelGrid } from "./asset-class-panel-grid";
export type { AssetClassPanelGridProps } from "./asset-class-panel-grid";
export { AssetClassPanelCard } from "./asset-class-panel-card";
export type { AssetClassPanelCardProps } from "./asset-class-panel-card";
export {
  ASSET_CLASS_PANELS,
  classifyHolding,
  getPanelDescriptor,
  rollupHoldingsByPanel,
} from "./taxonomy";
export type {
  AssetClassPanelDescriptor,
  AssetClassPanelId,
  AssetClassPanelRollup,
} from "./taxonomy";
