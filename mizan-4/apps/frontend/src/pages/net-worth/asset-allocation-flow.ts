/**
 * Asset-allocation Sankey helper — Track NW PR-NW2 / Goal v3 §V Phase 7.
 *
 * Translates the user's portfolio into a Sankey flow:
 *
 *     "Net Worth"  ───────►  Equities
 *                  ────────►  Bonds & Sukuks
 *                  ────────►  Bank & Cash
 *                  ...etc (one link per non-empty asset class)
 *
 * The single root node "Net Worth" is the source; each non-empty
 * asset class panel is a target. Link values are the asset class's
 * total base-currency exposure.
 *
 * Pure-math; no IO. Tests pin every branch.
 *
 * # Out of scope (deferred)
 *
 * - The full income → spending → savings cash-flow Sankey (PR-NW2.b
 *   — needs the activities surface threaded through the frontend)
 * - Liabilities decomposition: a future "Liabilities → categories"
 *   Sankey lands as PR-NW3 alongside the liabilities integration
 */
import type { Holding } from "@/lib/types";

import {
  ASSET_CLASS_PANELS,
  classifyHolding,
  type AssetClassPanelId,
} from "@/components/asset-class-panels/taxonomy";
import type { SankeyDataset } from "@/components/charts/sankey";

/** Per-panel total in base currency. */
interface PanelTotal {
  panelId: AssetClassPanelId;
  label: string;
  totalBase: number;
}

/** Compute per-panel totals from holdings. Excludes vehicles (per Feroz #14). */
export function computePanelTotals(holdings: readonly Holding[]): PanelTotal[] {
  const totals = new Map<AssetClassPanelId, number>();
  for (const h of holdings) {
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const panelId = classifyHolding(h);
    if (panelId === "other") continue; // vehicles + unclassified excluded
    totals.set(panelId, (totals.get(panelId) ?? 0) + value);
  }
  const rows: PanelTotal[] = [];
  for (const panel of ASSET_CLASS_PANELS) {
    const total = totals.get(panel.id);
    if (total && total > 0) {
      rows.push({ panelId: panel.id, label: panel.label, totalBase: total });
    }
  }
  // Desc by total — widest flows render highest in the Sankey
  rows.sort((a, b) => b.totalBase - a.totalBase);
  return rows;
}

/**
 * Build a Sankey dataset that flows from "Net Worth" → each non-empty
 * asset-class panel. Returns null when there are no non-empty panels
 * (caller renders an empty-state instead of a degenerate single-node
 * graph).
 */
export function buildAssetAllocationFlow(
  holdings: readonly Holding[],
): SankeyDataset | null {
  const panels = computePanelTotals(holdings);
  if (panels.length === 0) return null;

  // Node 0 is the root "Net Worth"; nodes 1..N are the asset class panels.
  const nodes = [
    { name: "Net Worth" },
    ...panels.map((p) => ({ name: p.label })),
  ];
  const links = panels.map((p, i) => ({
    source: 0,
    target: i + 1,
    value: p.totalBase,
  }));

  return { nodes, links };
}

/** Total Net Worth in base currency across non-vehicle classes. */
export function totalNetWorthBase(holdings: readonly Holding[]): number {
  return computePanelTotals(holdings).reduce((acc, p) => acc + p.totalBase, 0);
}
