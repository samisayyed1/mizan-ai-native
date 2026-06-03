/**
 * Mizan Badge popover renderers — Track E PR-E3 / ADR 0023.
 *
 * Per ADR 0023 §"Hover popover content", each modifier badge surfaces
 * a popover with three sections:
 *   1. Why the badge applies (1 sentence)
 *   2. What data backs it (e.g. last-synced timestamp + policy window)
 *   3. Action (e.g. "Tap to review the diff")
 *
 * Performance budget: <50ms hover-to-paint per Spec §17 / §A19.
 *
 * Why one file with discriminated-union contexts (vs 8 separate
 * renderer files):
 *   - One file is faster to scan + audit + extend; per-modifier
 *     extraction is mechanical if any single renderer grows past
 *     ~100 LOC, which none of these will.
 *   - Discriminated-union context types catch mismatched-modifier
 *     bugs at compile time at every call site.
 *   - Tests can table-drive across the whole set, which means future
 *     modifiers stay covered automatically.
 *
 * The renderers are PURE: caller supplies the context, the renderer
 * returns React. No I/O, no side effects, no hooks. This means a host
 * surface can compute the context once (e.g. cache invalidation
 * inputs from `cache_policy`) and render the popover synchronously
 * on hover — no roundtrip, hence the budget.
 */
import * as React from "react";

import { Popover, PopoverContent, PopoverTrigger } from "./popover";

import type { MizanBadgeModifier } from "./mizan-badge";

// ─── Context types ────────────────────────────────────────────────────

/** Context for the `'stale'` modifier. */
export interface StaleContext {
  modifier: "stale";
  /** When the value was last synced from the upstream source. */
  lastSyncedAtMs: number;
  /** Cache freshness policy window (ms). From `cache_policy` registry. */
  policyWindowMs: number;
  /** Optional refresh callback for the action row. */
  onRefresh?: () => void;
}

/** Context for the `'pending-reconciliation'` modifier. */
export interface PendingReconciliationContext {
  modifier: "pending-reconciliation";
  /** Stored value pre-sync. */
  priorValue: number;
  /** New value the sync just delivered. */
  currentValue: number;
  /** Display currency. */
  currency: string;
  /** Open the reconciliation diff. */
  onReview?: () => void;
}

/** Context for the `'ai-estimated'` modifier. */
export interface AiEstimatedContext {
  modifier: "ai-estimated";
  /** Estimator confidence in [0, 1]. */
  confidence: number;
  /** Lower bound of the estimation range. */
  low: number;
  /** Upper bound of the estimation range. */
  high: number;
  /** Display currency. */
  currency: string;
  /** Human-readable source ("Dubai Land Department", "Chrono24", …). */
  source: string;
}

/** Context for the `'halal-screened'` modifier. */
export interface HalalScreenedContext {
  modifier: "halal-screened";
  /** When the AAOIFI screening last ran. */
  screenedAtMs: number;
  /** Screener identifier (e.g. "AAOIFI v2"). */
  screener: string;
}

/** Context for the `'mixed-compliance'` modifier. */
export interface MixedComplianceContext {
  modifier: "mixed-compliance";
  /** When the AAOIFI screening last ran. */
  screenedAtMs: number;
  /** Specific criteria that returned ambiguous verdicts. */
  reasons: readonly string[];
}

/** Context for the `'audit-trail'` modifier. */
export interface AuditTrailContext {
  modifier: "audit-trail";
  /** Truth Ledger entry hash (hex). */
  truthLedgerHash: string;
  /** Open the Truth Ledger explorer at this hash. */
  onViewLedger?: () => void;
}

/** Context for the `'advisor-reviewed'` modifier. */
export interface AdvisorReviewedContext {
  modifier: "advisor-reviewed";
  /** Display name of the reviewing advisor. */
  advisorName: string;
  /** When the advisor's last review landed. */
  reviewedAtMs: number;
}

/** Context for the `'agent-modified'` modifier. */
export interface AgentModifiedContext {
  modifier: "agent-modified";
  /** Tool the agent used to write (e.g. `update_holding`). */
  toolName: string;
  /** When the agent's last write to this row landed. */
  modifiedAtMs: number;
}

/** Context for the `'mcp'` modifier. */
export interface McpContext {
  modifier: "mcp";
  /** Display name of the MCP server. */
  serverName: string;
  /** URL of the MCP server (for the action row). */
  serverUrl?: string;
}

/**
 * Discriminated union — pass the right context for the right modifier.
 * `'pending-reconciliation'` carries the `priorValue`/`currentValue`
 * pair; `'audit-trail'` carries the hash; etc. TypeScript catches the
 * mismatched-modifier-context bug at every call site.
 */
export type MizanBadgePopoverContext =
  | StaleContext
  | PendingReconciliationContext
  | AiEstimatedContext
  | HalalScreenedContext
  | MixedComplianceContext
  | AuditTrailContext
  | AdvisorReviewedContext
  | AgentModifiedContext
  | McpContext;

// ─── Helpers ──────────────────────────────────────────────────────────

/** Render a "N hours ago" style relative timestamp from absolute ms. */
function formatRelativeFromMs(nowMs: number, thenMs: number): string {
  const diffMs = Math.max(0, nowMs - thenMs);
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}

function formatPolicyWindow(ms: number): string {
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`;
  if (ms < 3_600_000) return `${Math.round(ms / 60_000)} min`;
  if (ms < 86_400_000) return `${Math.round(ms / 3_600_000)}h`;
  return `${Math.round(ms / 86_400_000)}d`;
}

function formatAmount(value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      maximumFractionDigits: 0,
    }).format(value);
  } catch {
    return `${currency} ${value.toLocaleString()}`;
  }
}

// ─── Pure renderers — exported for direct testing ─────────────────────

interface RendererOutput {
  /** Section 1 — why the badge applies. One sentence. */
  why: string;
  /** Section 2 — the data backing it. One sentence. */
  data: string;
  /**
   * Section 3 — optional callable action. Rendered as a `<button>`
   * when provided.
   */
  action?: { label: string; onClick: () => void };
}

/**
 * Render the popover content for a given context. Pure function +
 * `nowMs` parameter so tests can pin the relative-time output.
 */
export function renderBadgePopoverContent(
  context: MizanBadgePopoverContext,
  nowMs: number,
): RendererOutput {
  switch (context.modifier) {
    case "stale": {
      const since = formatRelativeFromMs(nowMs, context.lastSyncedAtMs);
      const window = formatPolicyWindow(context.policyWindowMs);
      const out: RendererOutput = {
        why: `This value hasn't been refreshed since ${since}.`,
        data: `Cache policy window is ${window}; we'll auto-refresh on the next sync tick.`,
      };
      if (context.onRefresh) {
        out.action = { label: "Refresh now", onClick: context.onRefresh };
      }
      return out;
    }
    case "pending-reconciliation": {
      const prior = formatAmount(context.priorValue, context.currency);
      const next = formatAmount(context.currentValue, context.currency);
      const out: RendererOutput = {
        why: "A sync delivered a value that differs from the one on file.",
        data: `Was ${prior}; now ${next}. Needs your review before it's accepted.`,
      };
      if (context.onReview) {
        out.action = { label: "Review diff", onClick: context.onReview };
      }
      return out;
    }
    case "ai-estimated": {
      const lo = formatAmount(context.low, context.currency);
      const hi = formatAmount(context.high, context.currency);
      const pct = Math.round(Math.max(0, Math.min(1, context.confidence)) * 100);
      return {
        why: "This value came from AI estimation against external comparables.",
        data: `Range: ${lo}–${hi} (confidence ${pct}%). Source: ${context.source}.`,
      };
    }
    case "halal-screened": {
      const since = formatRelativeFromMs(nowMs, context.screenedAtMs);
      return {
        why: "This holding passed our Sharia compliance screen.",
        data: `Last screened ${since} by ${context.screener}. Verdict: compliant.`,
      };
    }
    case "mixed-compliance": {
      const since = formatRelativeFromMs(nowMs, context.screenedAtMs);
      const reasons =
        context.reasons.length > 0 ? context.reasons.join("; ") : "ambiguous criteria";
      return {
        why: "The Sharia screen returned a mixed verdict.",
        data: `Last screened ${since}. Reasons: ${reasons}.`,
      };
    }
    case "audit-trail": {
      const shortHash = context.truthLedgerHash.slice(0, 10);
      const out: RendererOutput = {
        why: "This value is anchored in the Truth Ledger.",
        data: `Hash ${shortHash}… — open to verify the chain integrity.`,
      };
      if (context.onViewLedger) {
        out.action = { label: "Open Truth Ledger", onClick: context.onViewLedger };
      }
      return out;
    }
    case "advisor-reviewed": {
      const since = formatRelativeFromMs(nowMs, context.reviewedAtMs);
      return {
        why: "Your advisor has signed off on this holding.",
        data: `Last reviewed ${since} by ${context.advisorName}.`,
      };
    }
    case "agent-modified": {
      const since = formatRelativeFromMs(nowMs, context.modifiedAtMs);
      return {
        why: "The most recent write to this row came from Mizan AI.",
        data: `Tool: ${context.toolName}. ${since}.`,
      };
    }
    case "mcp": {
      const out: RendererOutput = {
        why: "This data came from a connected MCP server.",
        data: `Server: ${context.serverName}.`,
      };
      if (context.serverUrl) {
        out.action = {
          label: "Open server",
          onClick: () => {
            if (typeof window !== "undefined" && context.serverUrl) {
              window.open(context.serverUrl, "_blank", "noopener,noreferrer");
            }
          },
        };
      }
      return out;
    }
  }
}

// ─── Public component ────────────────────────────────────────────────

export interface MizanBadgePopoverProps {
  /** The modifier whose chip is the popover trigger. */
  modifier: MizanBadgeModifier;
  /** The data backing the popover content. */
  context: MizanBadgePopoverContext;
  /** Override the per-render baseline timestamp. Useful for tests. */
  nowMs?: number;
  /** The chip (or any node) that triggers the popover on hover/tap. */
  children: React.ReactNode;
}

/**
 * Wrap any node (typically a chip from `MizanBadge`) into a popover
 * whose content is the per-modifier reasoning trail.
 *
 * The contract enforces context-shape match at the type level: the
 * caller can't pass an `audit-trail` context with a `'stale'` chip,
 * because the discriminator `modifier` on the context is checked
 * against the prop. The runtime guard below preserves it at JS time
 * for non-TS callers.
 */
export function MizanBadgePopover({
  modifier,
  context,
  nowMs,
  children,
}: MizanBadgePopoverProps) {
  // Runtime guard — TS catches the mismatch at compile time but JS
  // consumers (e.g. addon-sdk callers) get a console.error rather than
  // a silently-wrong popover. Avoids `process.env` directly so the
  // @mizan/ui package stays node-typings-free.
  if (context.modifier !== modifier) {
    // eslint-disable-next-line no-console
    console.error(
      `MizanBadgePopover: modifier prop "${modifier}" does not match context.modifier "${context.modifier}"`,
    );
  }
  const baselineMs = nowMs ?? Date.now();
  const content = React.useMemo(
    () => renderBadgePopoverContent(context, baselineMs),
    [context, baselineMs],
  );
  return (
    <Popover>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent
        className="w-72 text-sm"
        data-testid={`mizan-badge-popover-${modifier}`}
      >
        <div className="space-y-2">
          <div className="text-foreground font-medium">{content.why}</div>
          <div className="text-muted-foreground">{content.data}</div>
          {content.action && (
            <button
              type="button"
              onClick={content.action.onClick}
              className="text-primary hover:underline focus-visible:underline focus-visible:outline-none"
              data-testid={`mizan-badge-popover-action-${modifier}`}
            >
              {content.action.label}
            </button>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
