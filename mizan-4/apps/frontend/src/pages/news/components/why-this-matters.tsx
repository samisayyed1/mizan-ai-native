/**
 * "Why this matters to you" rationale renderer — Track D PR-D4.
 *
 * Surfaces personalization signals from the Mizan Connect news
 * ranking stack (PR-D2 lexical + PR-D3 pgvector similarity) as a
 * compact sub-label on news cards.
 *
 * # Inputs
 *
 * `rationale` is the `string[]` field on `NewsArticle.rationale`,
 * populated by the `/v1/news/feed` handler. Each string is one
 * matched signal — typically 1-3 strings per ranked article.
 *
 * Strings are pre-ordered by signal weight on the server side
 * (ticker > category > memory keyword > pgvector similarity) so the
 * first entry is the most salient reason. The component renders the
 * first 2 by default; "+N more" expands to show all.
 *
 * # Graceful absence
 *
 * When `rationale` is empty or undefined the component renders
 * nothing — news cards from non-personalized providers (the Global
 * tab) don't carry rationale and shouldn't show an empty sub-label.
 *
 * # Out of scope
 *
 * - The relevance-score chip (PR-D4.b — uses `NewsArticle.relevanceScore`)
 * - Per-rationale icons (PR-D4.c — adds origin icons for ticker /
 *   category / memory / vector signals)
 */
import { useState } from "react";

import { Icons } from "@mizan/ui/components/ui/icons";

export interface WhyThisMattersProps {
  /** Pre-ordered rationale strings from the news ranker. */
  rationale?: string[];
  /** Max items shown collapsed. Defaults to 2. */
  collapsedLimit?: number;
  /** Optional CSS class for outer wrapper. */
  className?: string;
}

export function WhyThisMatters({
  rationale,
  collapsedLimit = 2,
  className,
}: WhyThisMattersProps) {
  const [expanded, setExpanded] = useState(false);

  if (!rationale || rationale.length === 0) {
    return null;
  }

  const visible = expanded ? rationale : rationale.slice(0, collapsedLimit);
  const overflow = rationale.length - visible.length;

  return (
    <div
      className={[
        "border-border/60 bg-muted/30 rounded-md border-l-2 px-2 py-1.5 text-xs",
        className ?? "",
      ]
        .filter(Boolean)
        .join(" ")}
      aria-label="Why this matters to you"
    >
      <div className="text-muted-foreground mb-0.5 flex items-center gap-1 text-[10px] font-medium uppercase tracking-wide">
        <Icons.Sparkles className="h-3 w-3" />
        Why this matters to you
      </div>
      <ul className="text-foreground/90 space-y-0.5">
        {visible.map((line, i) => (
          <li key={`${i}-${line}`} className="leading-snug">
            {line}
          </li>
        ))}
      </ul>
      {overflow > 0 && !expanded && (
        <button
          type="button"
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setExpanded(true);
          }}
          className="text-muted-foreground hover:text-foreground mt-1 text-[10px]"
        >
          +{overflow} more
        </button>
      )}
    </div>
  );
}
