import { Card } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Link } from "react-router-dom";

import { ExternalLink } from "@/components/external-link";
import { useFinancialNews } from "@/hooks/use-financial-news";
import { relativeNewsTime } from "@/pages/news/components/news-utils";

/**
 * Compact news strip on the simplified Home (M3.4).
 *
 * Shows the top 3 articles tailored to the user's holdings ("For You"),
 * falling back to "Markets" when there are no holdings yet. Always renders
 * (even on Free) — the M3.4 daily-limit gate on the /news page handles the
 * upsell separately; here the widget always presents the user with at least
 * a few real headlines so news has a presence without owning a nav tab.
 */
export function NewsHomeWidget() {
  const { data: forYou = [], isLoading: isLoadingForYou, hasSymbols } = useFinancialNews("forYou");
  const fallbackEnabled = !hasSymbols;
  const { data: markets = [], isLoading: isLoadingMarkets } = useFinancialNews(
    fallbackEnabled ? "markets" : "forYou",
  );

  const isLoading = isLoadingForYou || (fallbackEnabled && isLoadingMarkets);
  const articles = (fallbackEnabled ? markets : forYou).slice(0, 3);

  if (isLoading) {
    return (
      <Card className="border-border/60 rounded-xl p-5">
        <Skeleton className="mb-3 h-4 w-24" />
        <div className="space-y-3">
          <Skeleton className="h-5 w-full" />
          <Skeleton className="h-5 w-full" />
          <Skeleton className="h-5 w-3/4" />
        </div>
      </Card>
    );
  }

  // ADR 0018b: the sidebar's News widget now lives at the top of the
  // column, so an empty array can NOT silently `return null` — the
  // first visible slot would just be a hole. Render the header + a
  // quiet empty state instead so the user always sees the surface
  // exists and where to go for more.

  // Shared header — same chrome for loading / empty / populated.
  const header = (
    <div className="mb-3 flex items-center justify-between gap-2">
      <div className="flex items-center gap-2">
        <Icons.Newspaper className="text-muted-foreground h-4 w-4" />
        <h2 className="text-sm font-semibold tracking-tight">
          {fallbackEnabled ? "Markets" : "For you"}
        </h2>
      </div>
      <Link
        to="/news"
        className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
      >
        See all →
      </Link>
    </div>
  );

  return (
    <Card className="border-border/60 rounded-xl p-5">
      {header}
      {articles.length === 0 ? (
        <p className="text-muted-foreground text-xs leading-relaxed">
          {hasSymbols
            ? "No headlines on your holdings today."
            : "Connect an account to see news tailored to what you own."}
        </p>
      ) : (
        <ul className="divide-border divide-y">
          {articles.map((article) => {
            const time = relativeNewsTime(article.published);
            return (
              <li key={article.id} className="py-2.5 first:pt-0 last:pb-0">
                <ExternalLink href={article.url} className="group block" rel="noopener noreferrer">
                  <p className="text-sm font-medium leading-snug group-hover:underline">
                    {article.title}
                  </p>
                  <p className="text-muted-foreground mt-1 text-xs">
                    {article.source && <span className="font-medium">{article.source}</span>}
                    {article.source && time && <span> · </span>}
                    {time}
                  </p>
                </ExternalLink>
              </li>
            );
          })}
        </ul>
      )}
    </Card>
  );
}
