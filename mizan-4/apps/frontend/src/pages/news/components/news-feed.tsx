import { Card } from "@mizan/ui/components/ui/card";
import { Icons, type IconName } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";

import { useFinancialNews } from "@/hooks/use-financial-news";
import type { NewsScope } from "@/lib/types";

import { NewsCard } from "./news-card";
import { NewsHero } from "./news-hero";

function EmptyState({ icon, title, message }: { icon: IconName; title: string; message: string }) {
  const Icon = Icons[icon];
  return (
    <Card className="flex flex-col items-center justify-center gap-3 px-6 py-16 text-center">
      <Icon className="text-muted-foreground/60 size-10" />
      <div>
        <p className="font-medium">{title}</p>
        <p className="text-muted-foreground mt-1 text-sm">{message}</p>
      </div>
    </Card>
  );
}

function NewsFeedSkeleton() {
  return (
    <div className="space-y-6">
      <Skeleton className="h-40 w-full rounded-lg" />
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-28 w-full rounded-lg" />
        ))}
      </div>
    </div>
  );
}

export function NewsFeed({ scope }: { scope: NewsScope }) {
  const { data: articles = [], isLoading, isError, hasSymbols } = useFinancialNews(scope);

  if (scope === "forYou" && !hasSymbols) {
    return (
      <EmptyState
        icon="TrendingUp"
        title="No holdings yet"
        message="Add holdings to your portfolio to see news tailored to what you own."
      />
    );
  }

  if (isLoading) return <NewsFeedSkeleton />;

  if (isError || articles.length === 0) {
    return (
      <EmptyState
        icon="Newspaper"
        title="No headlines right now"
        message="We couldn't load any news. Check back in a moment."
      />
    );
  }

  const [hero, ...rest] = articles;
  return (
    <div className="space-y-6">
      <NewsHero article={hero} />
      {rest.length > 0 && (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {rest.map((article) => (
            <NewsCard key={article.id} article={article} />
          ))}
        </div>
      )}
    </div>
  );
}
