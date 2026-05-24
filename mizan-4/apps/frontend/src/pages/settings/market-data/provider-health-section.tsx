import { Badge } from "@mizan/ui/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";

import { useProviderHealth } from "@/hooks/use-provider-health";

import { circuitStateVisual } from "./provider-health-helpers";

/** Read-only panel listing each market-data provider's live health (circuit
 * state + recent failures). Hidden entirely when health can't be reported
 * (e.g. web mode) or there are no providers, so it never shows a broken card. */
export function ProviderHealthSection() {
  const { data, isLoading, isError } = useProviderHealth();

  if (isError) return null;
  if (!isLoading && (!data || data.length === 0)) return null;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Data connections</CardTitle>
        <CardDescription>
          Live status of each market-data source. A source that keeps failing is paused
          automatically and skipped until it recovers.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-2">
        {isLoading ? (
          <>
            <Skeleton className="h-6 w-full" />
            <Skeleton className="h-6 w-full" />
          </>
        ) : (
          data!.map((provider) => {
            const visual = circuitStateVisual(provider.circuitState);
            return (
              <div key={provider.id} className="flex items-center justify-between gap-3 text-sm">
                <span className="font-medium">{provider.id}</span>
                <div className="flex items-center gap-3">
                  {provider.consecutiveFailures > 0 && (
                    <span className="text-muted-foreground text-xs tabular-nums">
                      {provider.consecutiveFailures} recent{" "}
                      {provider.consecutiveFailures === 1 ? "failure" : "failures"}
                    </span>
                  )}
                  <Badge variant={visual.variant} className="h-5 px-1.5 text-[10px] font-normal">
                    {visual.label}
                  </Badge>
                </div>
              </div>
            );
          })
        )}
      </CardContent>
    </Card>
  );
}
