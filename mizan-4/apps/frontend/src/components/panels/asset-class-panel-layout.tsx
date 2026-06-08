/**
 * Shared layout for the 12 asset class detail pages per Spec §6 (Per-
 * asset-class detail view universal pattern) + ADR 0018.
 *
 * Every panel detail page (brokerage / equities / sukuks / real estate /
 * crypto / ...) gets the same chrome:
 *
 *   ┌────────────────────────────────────────────────────────────┐
 *   │ ←  Page title · icon                                       │
 *   │    summary line (total, count, etc.)                       │
 *   ├────────────────────────────────────────────────────────────┤
 *   │                                                            │
 *   │ <chart slot>                                               │
 *   │                                                            │
 *   ├────────────────────────────────────────────────────────────┤
 *   │ <list slot>                                                │
 *   ├────────────────────────────────────────────────────────────┤
 *   │ <insights slot — optional>                                 │
 *   ├────────────────────────────────────────────────────────────┤
 *   │ <actions slot — optional, floating CTAs>                   │
 *   └────────────────────────────────────────────────────────────┘
 *
 * Each panel page passes the chart it cares about (donut / bar /
 * heatmap from ADR 0019), the holdings list, and optional insights /
 * actions. The chrome lives here so all 12 pages stay visually
 * consistent and the §13 density bar holds without per-page drift.
 *
 * History tab + tier-gated "Sync account" CTA are not yet plumbed
 * here — they ship as PR-UI-4.b once each panel's history surface and
 * sync entrypoint are exercised; the layout reserves the slots so
 * follow-ups are additive, not restructural.
 */
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import type { ComponentType } from "react";

export interface AssetClassPanelLayoutProps {
  /** Page title (matches the `AssetClassPanelDescriptor.label`). */
  title: string;
  /** Icon at the top of the page (matches the descriptor's iconKey). */
  IconComponent: ComponentType<{ className?: string }>;
  /** One-line summary under the title (total + count etc.). */
  summary: ReactNode;
  /** The asset-class allocation chart for this page (donut / bar / heatmap). */
  chart?: ReactNode;
  /** The holdings / accounts list for this page. */
  list?: ReactNode;
  /**
   * Optional AI insights strip — 2-3 agent bullets specific to this
   * asset class. Wired separately per panel as the insights surface
   * supplies asset-class-specific rules.
   */
  insights?: ReactNode;
  /**
   * Optional floating action bar (e.g. "Add holding" + "Sync account").
   * Pages without an action bar can omit; the layout's footer back
   * button is always rendered.
   */
  actions?: ReactNode;
  /** Optional override for the back-to-dashboard button label. */
  backLabel?: string;
}

export function AssetClassPanelLayout({
  title,
  IconComponent,
  summary,
  chart,
  list,
  insights,
  actions,
  backLabel = "Back to dashboard",
}: AssetClassPanelLayoutProps) {
  const navigate = useNavigate();

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <IconComponent className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        </div>
        <div className="text-muted-foreground text-sm">{summary}</div>
      </header>

      {chart}

      {list}

      {insights ? (
        <section
          aria-label={`AI insights for ${title}`}
          className="bg-muted/30 rounded-2xl border p-4"
        >
          <div className="text-muted-foreground mb-2 flex items-center gap-2 text-xs font-medium uppercase tracking-wider">
            <Icons.Sparkles className="h-3.5 w-3.5" />
            <span>AI insights</span>
          </div>
          {insights}
        </section>
      ) : null}

      {actions ? (
        <div
          aria-label={`${title} actions`}
          className="flex flex-wrap gap-2 pt-2"
        >
          {actions}
        </div>
      ) : null}

      <div className="pt-2">
        <Button variant="outline" size="sm" onClick={() => navigate("/")}>
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          {backLabel}
        </Button>
      </div>
    </div>
  );
}
