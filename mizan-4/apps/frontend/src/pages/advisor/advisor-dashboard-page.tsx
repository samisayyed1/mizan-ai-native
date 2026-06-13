import { Page, PageContent, PageHeader } from "@mizan/ui";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { useQuery } from "@tanstack/react-query";

import { listMyTeams } from "@/adapters";
// Relative path to bypass the BUILD_TARGET-conditional `@/adapters`
// alias in vite.config — same rationale as broker-service.ts.
import type { MyTeamsResponse } from "../../adapters/shared/teams";
import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";
import { QueryKeys } from "@/lib/query-keys";

/**
 * Advisor dashboard (M5.2).
 *
 * Renders every team the caller is a member of as a tile. Tapping a tile
 * opens the team's portfolio view (which we'll wire up in M5.2b — for now
 * each tile just shows the team summary).
 *
 * Pro/Enterprise only via `entitlements.advisorMode`. Free/Basic see a
 * locked card and tap to open the UpgradeGate.
 */
export default function AdvisorDashboardPage() {
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();
  const enabled = entitlements.advisorMode;

  const teamsQuery = useQuery<MyTeamsResponse>({
    queryKey: [QueryKeys.MY_TEAMS],
    queryFn: listMyTeams,
    enabled,
    staleTime: 60_000,
  });

  if (!enabled) {
    return (
      <Page>
        <PageHeader heading="Advisor dashboard" />
        <PageContent>
          <Card
            className="bg-linear-to-br to-card mx-auto max-w-2xl p-8 text-center"
            style={{
              borderColor: "hsl(31 38% 46% / 0.25)",
              backgroundImage:
                "linear-gradient(to bottom right, hsl(40 67% 87% / 0.18), transparent)",
            }}
          >
            <Icons.Users
              className="mx-auto mb-3 h-10 w-10"
              style={{ color: "hsl(31 38% 46%)" }}
            />
            <h2 className="text-lg font-semibold">Advisor dashboard</h2>
            <p className="text-muted-foreground mx-auto mt-2 max-w-md text-sm leading-relaxed">
              Manage multiple client portfolios from one place. Read-only drill-down by default;
              mutating clients' data requires owner role on the team. Included with the Mizan
              Enterprise plan.
            </p>
            <button
              type="button"
              className="bg-foreground text-background mt-4 rounded-md px-4 py-2 text-sm font-medium transition-colors hover:opacity-90"
              onClick={() => requestUpgrade("advisor_mode")}
            >
              Upgrade to Enterprise
            </button>
          </Card>
        </PageContent>
      </Page>
    );
  }

  const teams = teamsQuery.data?.teams ?? [];

  return (
    <Page>
      <PageHeader heading="Advisor dashboard" />
      <PageContent>
        <p className="text-muted-foreground mb-4 text-sm">
          Teams you can act on — your own and every team you've been added to as an advisor or
          viewer.
        </p>

        {teamsQuery.isLoading ? (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Skeleton className="h-32" />
            <Skeleton className="h-32" />
            <Skeleton className="h-32" />
          </div>
        ) : teams.length === 0 ? (
          <Card className="p-8 text-center">
            <p className="text-muted-foreground text-sm">
              No teams yet. Owners can invite you from their team page; once accepted you'll see the
              team here.
            </p>
          </Card>
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {teams.map((team) => (
              <Card
                key={team.id}
                style={{ borderColor: "hsl(31 38% 46% / 0.15)" }}
              >
                <CardHeader>
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <CardTitle className="text-base">{team.name}</CardTitle>
                      <CardDescription className="text-xs">
                        Your role:{" "}
                        <span className="font-medium capitalize">{team.myRole ?? "—"}</span>
                      </CardDescription>
                    </div>
                    {team.brandingLogoUrl && (
                      <img
                        src={team.brandingLogoUrl}
                        alt=""
                        className="h-8 w-8 rounded object-contain"
                      />
                    )}
                  </div>
                </CardHeader>
                <CardContent>
                  {/* Drill-down view is on the next sprint. Until it
                      ships, the team tile is informational — we render
                      a polished "in progress" line instead of leaking
                      the internal sprint identifier ("M5.2b") into the
                      UI. The card's gold border + brand styling
                      signals that this is a real surface, not a
                      crashed render. */}
                  <div className="text-muted-foreground flex items-center gap-2 text-xs">
                    <span
                      aria-hidden="true"
                      className="inline-block h-1.5 w-1.5 rounded-full"
                      style={{ backgroundColor: "hsl(31 38% 46%)" }}
                    />
                    <span>Read-only portfolio view in active development.</span>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </PageContent>
    </Page>
  );
}
