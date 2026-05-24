import { Tabs, TabsContent, TabsList, TabsTrigger } from "@mizan/ui/components/ui/tabs";
import { useState } from "react";

import type { NewsScope } from "@/lib/types";

import { NewsFeed } from "./components/news-feed";

export default function NewsPage() {
  const [scope, setScope] = useState<NewsScope>("markets");

  return (
    <div className="mx-auto w-full max-w-5xl px-4 py-6 md:px-6 lg:px-8">
      <div className="mb-6">
        <h1 className="font-heading text-2xl font-bold tracking-tight md:text-3xl">News</h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Live financial headlines, tailored to your portfolio and the markets.
        </p>
      </div>

      <Tabs value={scope} onValueChange={(value) => setScope(value as NewsScope)}>
        <TabsList>
          <TabsTrigger value="forYou">For You</TabsTrigger>
          <TabsTrigger value="markets">Markets</TabsTrigger>
        </TabsList>
        <TabsContent value="forYou" className="mt-6">
          <NewsFeed scope="forYou" />
        </TabsContent>
        <TabsContent value="markets" className="mt-6">
          <NewsFeed scope="markets" />
        </TabsContent>
      </Tabs>
    </div>
  );
}
