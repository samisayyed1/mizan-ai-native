import { Badge } from "@mizan/ui/components/ui/badge";
import { Card } from "@mizan/ui/components/ui/card";

import { ExternalLink } from "@/components/external-link";
import type { NewsArticle } from "@/lib/types";

import { relativeNewsTime } from "./news-utils";
import { WhyThisMatters } from "./why-this-matters";

/** A single headline card in the magazine grid. */
export function NewsCard({ article }: { article: NewsArticle }) {
  const time = relativeNewsTime(article.published);
  return (
    <ExternalLink href={article.url} className="group block h-full" rel="noopener noreferrer">
      <Card className="hover:bg-muted/30 flex h-full flex-col gap-2 p-4 transition-colors">
        <h3 className="line-clamp-2 text-sm font-semibold leading-snug group-hover:underline">
          {article.title}
        </h3>
        {article.summary && (
          <p className="text-muted-foreground line-clamp-2 text-xs leading-relaxed">
            {article.summary}
          </p>
        )}
        {/* PR-D4: "Why this matters to you" rationale from the
            Mizan Connect news ranker. Renders nothing when absent
            (Global tab / non-personalized providers). */}
        <WhyThisMatters rationale={article.rationale} />
        <div className="text-muted-foreground mt-auto flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
          {article.source && <span className="font-medium">{article.source}</span>}
          {time && <span>· {time}</span>}
        </div>
        {article.relatedSymbols.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {article.relatedSymbols.slice(0, 4).map((symbol) => (
              <Badge key={symbol} variant="outline" className="text-[10px] font-normal">
                {symbol}
              </Badge>
            ))}
            {article.relatedSymbols.length > 4 && (
              <span className="text-muted-foreground text-[10px]">
                +{article.relatedSymbols.length - 4}
              </span>
            )}
          </div>
        )}
      </Card>
    </ExternalLink>
  );
}
