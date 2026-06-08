import * as React from "react";

import { cn } from "../../lib/utils";

/**
 * Unified empty-state primitive — PR-POLISH-7 / Spec §13.
 *
 * Every empty surface in Mizan adopts this shape so the
 * "ready when you are" feel reads coherent across pages:
 *
 *   ┌─────────────────────────────────────────┐
 *   │                                         │
 *   │           [illustration]                │
 *   │                                         │
 *   │           Headline                      │
 *   │           Subdued body text.            │
 *   │                                         │
 *   │      [Primary CTA]  [Secondary CTA]     │
 *   │                                         │
 *   └─────────────────────────────────────────┘
 *
 * The illustration slot accepts any ReactNode — usually a
 * monochrome line-art glyph rendered at 64–96px, drawn in
 * `text-muted-foreground/40` so it whispers instead of shouting.
 *
 * `actions` slot accepts any number of buttons; the layout caller
 * decides which is primary vs secondary. Per Spec §13 the
 * primary CTA should lead with the AI agent path
 * ("Tell Mizan: 'I bought 50 shares...'") and the secondary
 * with the manual form.
 *
 * Loading + error are NOT empty states — use `<PageSkeleton>`
 * and `<PageErrorBoundary>` respectively. This component is
 * specifically for the "no data yet, here's how to get started"
 * surface.
 */
interface PageEmptyStateProps {
  /** Optional illustration (typically a monochrome SVG glyph). */
  illustration?: React.ReactNode;
  /** Bold headline — 1 short sentence. */
  headline: string;
  /** Subdued body — 1-3 sentences explaining how to populate the surface. */
  body?: React.ReactNode;
  /** Action row at the bottom. Pass <Button>s; primary first, secondary after. */
  actions?: React.ReactNode;
  /** Optional max-width override (default `max-w-md`). */
  className?: string;
}

export function PageEmptyState({
  illustration,
  headline,
  body,
  actions,
  className,
}: PageEmptyStateProps) {
  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "mx-auto flex w-full flex-col items-center justify-center gap-4 py-12 text-center",
        className ?? "max-w-md",
      )}
    >
      {illustration ? (
        <div className="text-muted-foreground/40 [&_svg]:size-16 sm:[&_svg]:size-20" aria-hidden="true">
          {illustration}
        </div>
      ) : null}
      <div className="space-y-2">
        <h2 className="text-foreground text-base font-semibold tracking-tight sm:text-lg">
          {headline}
        </h2>
        {body ? (
          <p className="text-muted-foreground text-sm leading-relaxed">{body}</p>
        ) : null}
      </div>
      {actions ? (
        <div className="mt-2 flex flex-wrap items-center justify-center gap-2">
          {actions}
        </div>
      ) : null}
    </div>
  );
}
