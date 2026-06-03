/**
 * Mizan Badge — provenance + modifier stack per ADR 0023.
 *
 * Track E PR-E2: the foundational visual surface for every figure
 * shown in the app. ADR 0023 §"10 origin variants" and §"8 modifier
 * badges" define the variant set; this primitive renders the origin
 * chip plus the modifier stack in the fixed severity order.
 *
 * Severity order (verbatim from ADR 0023):
 *   'stale' > 'pending-reconciliation' > 'ai-estimated'
 *   > 'mixed-compliance' > 'halal-screened' > 'agent-modified'
 *   > 'audit-trail' > 'advisor-reviewed' > 'mcp'
 *
 * Rendering cap: 3 visible modifiers; the rest collapse into a
 * `+N more` chip that the parent surface can wire to an expand
 * popover (PR-E3 ships the actual popover renderers — this primitive
 * stays headless on hover behaviour).
 *
 * Why a new component, not an overload on `<Badge>`:
 *   - Keeps the shadcn-ish base Badge primitive untouched + reusable
 *     for unrelated chips (toggle states, tags, etc.).
 *   - `<MizanBadge>` is the domain-specific composition; the base
 *     Badge is the foundation it draws.
 *
 * Semantic color tokens (dark/light parity is achieved by referring
 * to the `@mizan/ui` theme tokens rather than literal Tailwind
 * colors — `bg-success-foreground/...` etc.).
 */
import * as React from "react";

import { cn } from "../../lib/utils";
import { Badge } from "./badge";

/** Origin variants per ADR 0023 §"10 origin variants". */
export type MizanBadgeOrigin =
  | "manual"
  | "synced"
  | "imported"
  | "setu"
  | "sgfindex"
  | "tink"
  | "basiq"
  | "lean"
  | "ccxt"
  | "chain_reader"
  | "twelve_data"
  | "metalprice_api"
  | "bondevalue";

/** Modifier variants per ADR 0023 §"8 modifier badges" (+ MCP from Track K). */
export type MizanBadgeModifier =
  | "stale"
  | "pending-reconciliation"
  | "ai-estimated"
  | "mixed-compliance"
  | "halal-screened"
  | "agent-modified"
  | "audit-trail"
  | "advisor-reviewed"
  | "mcp";

/** Fixed severity order — index = rank, lower = higher severity. */
const MODIFIER_SEVERITY_ORDER: MizanBadgeModifier[] = [
  "stale",
  "pending-reconciliation",
  "ai-estimated",
  "mixed-compliance",
  "halal-screened",
  "agent-modified",
  "audit-trail",
  "advisor-reviewed",
  "mcp",
];

/** Visible modifier cap per ADR 0023; rest collapse into a `+N more` chip. */
export const MAX_VISIBLE_MODIFIERS = 3;

/**
 * Sort modifiers into ADR 0023's fixed severity order. Exported for
 * direct testing — also useful to any caller that wants to inspect
 * the ordering without rendering.
 *
 * Stable: duplicate modifiers (caller bug, but tolerated) preserve
 * relative order; unknown modifiers (forward-compat for future
 * additions) sort to the end.
 */
export function sortModifiersBySeverity(
  modifiers: readonly MizanBadgeModifier[],
): MizanBadgeModifier[] {
  const rank = (m: MizanBadgeModifier): number => {
    const idx = MODIFIER_SEVERITY_ORDER.indexOf(m);
    return idx === -1 ? MODIFIER_SEVERITY_ORDER.length : idx;
  };
  return [...modifiers].sort((a, b) => rank(a) - rank(b));
}

const ORIGIN_DISPLAY: Record<MizanBadgeOrigin, string> = {
  manual: "Manual",
  synced: "Synced",
  imported: "Imported",
  setu: "Setu",
  sgfindex: "SGFinDex",
  tink: "Tink",
  basiq: "Basiq",
  lean: "Lean",
  ccxt: "CCXT",
  chain_reader: "Chain reader",
  twelve_data: "Twelve Data",
  metalprice_api: "Metalprice",
  bondevalue: "Bondevalue",
};

const MODIFIER_DISPLAY: Record<MizanBadgeModifier, string> = {
  stale: "Stale",
  "pending-reconciliation": "Pending review",
  "ai-estimated": "AI estimate",
  "mixed-compliance": "Mixed compliance",
  "halal-screened": "Halal",
  "agent-modified": "Agent",
  "audit-trail": "Audit trail",
  "advisor-reviewed": "Advisor",
  mcp: "MCP",
};

/**
 * Modifier → base Badge variant. Maps each modifier into the
 * existing semantic-token variants (success/warning/destructive/info/
 * outline/secondary) so dark/light parity comes from the theme.
 */
function variantForModifier(
  m: MizanBadgeModifier,
): "success" | "warning" | "destructive" | "info" | "outline" | "secondary" {
  switch (m) {
    case "stale":
      return "destructive";
    case "pending-reconciliation":
      return "warning";
    case "ai-estimated":
      return "info";
    case "mixed-compliance":
      return "warning";
    case "halal-screened":
      return "success";
    case "agent-modified":
      return "secondary";
    case "audit-trail":
      return "outline";
    case "advisor-reviewed":
      return "info";
    case "mcp":
      return "outline";
  }
}

export interface MizanBadgeProps {
  /** Provenance — every figure on screen has an origin per ADR 0023. */
  origin: MizanBadgeOrigin;
  /**
   * Modifier stack. Caller passes them in any order; the primitive
   * sorts to ADR 0023's severity ranking before rendering.
   */
  modifiers?: readonly MizanBadgeModifier[];
  /** Optional override of the visible modifier cap. Defaults to 3. */
  maxVisibleModifiers?: number;
  /** Optional aria-label override; otherwise composed from origin + count. */
  ariaLabel?: string;
  /** Tailwind classes to append on the wrapper. */
  className?: string;
  /**
   * Called when the `+N more` rollup chip is activated. PR-E3 wires
   * this to a popover that expands the full modifier stack.
   */
  onOverflowActivate?: (hidden: readonly MizanBadgeModifier[]) => void;
}

/**
 * Render the Mizan Badge: origin chip + modifier stack (sorted by
 * severity, capped at `maxVisibleModifiers`, overflow into `+N more`).
 */
export function MizanBadge({
  origin,
  modifiers = [],
  maxVisibleModifiers = MAX_VISIBLE_MODIFIERS,
  ariaLabel,
  className,
  onOverflowActivate,
}: MizanBadgeProps) {
  const sorted = React.useMemo(() => sortModifiersBySeverity(modifiers), [modifiers]);
  const visible = sorted.slice(0, maxVisibleModifiers);
  const hidden = sorted.slice(maxVisibleModifiers);
  const overflowCount = hidden.length;

  const composedAria =
    ariaLabel ??
    `Origin ${ORIGIN_DISPLAY[origin]}${
      sorted.length > 0 ? `; ${sorted.length} modifier${sorted.length === 1 ? "" : "s"}` : ""
    }`;

  return (
    <span
      className={cn("inline-flex items-center gap-1", className)}
      role="group"
      aria-label={composedAria}
      data-testid="mizan-badge"
      data-origin={origin}
    >
      <Badge variant="secondary" data-testid="mizan-badge-origin">
        {ORIGIN_DISPLAY[origin]}
      </Badge>
      {visible.map((m) => (
        <Badge
          key={m}
          variant={variantForModifier(m)}
          data-testid={`mizan-badge-modifier-${m}`}
          aria-label={MODIFIER_DISPLAY[m]}
        >
          {MODIFIER_DISPLAY[m]}
        </Badge>
      ))}
      {overflowCount > 0 && (
        <Badge
          variant="outline"
          asChild
          data-testid="mizan-badge-overflow"
        >
          <button
            type="button"
            onClick={() => onOverflowActivate?.(hidden)}
            aria-label={`Show ${overflowCount} more modifier${overflowCount === 1 ? "" : "s"}`}
            data-testid="mizan-badge-overflow-button"
          >
            +{overflowCount} more
          </button>
        </Badge>
      )}
    </span>
  );
}
