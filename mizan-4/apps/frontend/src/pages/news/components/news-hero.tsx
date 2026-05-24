import { Badge } from "@mizan/ui/components/ui/badge";
import { Card } from "@mizan/ui/components/ui/card";

import { ExternalLink } from "@/components/external-link";
import type { NewsArticle } from "@/lib/types";

import { relativeNewsTime } from "./news-utils";

/** The large featured story at the top of the magazine feed. */
export function NewsHero({ article }: { article: NewsArticle }) {
  const time = relativeNewsTime(article.published);
  return (
    <ExternalLink href={article.url} className="group block" rel="noopener noreferrer">
      <Card className="from-card to-card/70 bg-gradient-to-br p-6 shadow-md transition-shadow hover:shadow-lg md:p-8">
        <div className="flex flex-wrap items-center gap-2 text-xs">
          {article.source && <Badge variant="secondary">{article.source}</Badge>}
          {time && <span className="text-muted-foreground">{time}</span>}
        </div>
        <h2 className="font-heading mt-3 text-xl font-bold leading-tight tracking-tight group-hover:underline md:text-2xl">
          {article.title}
        </h2>
        {article.summary && (
          <p className="text-muted-foreground mt-2 line-clamp-3 text-sm leading-relaxed">
            {article.summary}
          </p>
        )}
        {article.relatedSymbols.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-1.5">
            {article.relatedSymbols.slice(0, 6).map((symbol) => (
              <Badge key={symbol} variant="outline" className="text-[10px] font-normal">
                {symbol}
              </Badge>
            ))}
          </div>
        )}
      </Card>
    </ExternalLink>
  );
}
