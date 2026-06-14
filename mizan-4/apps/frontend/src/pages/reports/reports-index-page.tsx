import { Page, PageContent, PageHeader } from "@mizan/ui";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";

import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";

/** Assistant-native wealth reports grounded in computed local state. */
export default function ReportsIndexPage() {
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();
  const locked = !entitlements.advancedReports;

  const onAction = (path: string) => {
    if (locked) {
      requestUpgrade("advanced_reports");
      return;
    }
    window.location.assign(path);
  };

  return (
    <Page>
      <PageHeader heading="Reports" />
      <PageContent>
        <p className="text-muted-foreground mb-4 text-sm">
          Dynamic summaries grounded in your current encrypted wealth state.
        </p>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {/*
            Each tile points at the actual page that backs the
            report. Previously these all pointed at /reports/{slug}
            paths that don't exist in the router (only /reports
            itself + /reports/monthly are registered), producing
            blank pages. The income + health surfaces have shipped
            full pages (/income, /health); rental + payoff are
            served by the Real Estate panel + the Holdings list with
            a liabilities filter until dedicated report views ship.
          */}
          <ReportTile
            title="Income report"
            description="Year-to-date dividends, interest, rental income, and a monthly trend chart. Top 10 income-producing assets and YoY change."
            icon={<Icons.TrendingUp className="h-6 w-6"
                style={{ color: "hsl(31 38% 46%)" }} />}
            ctaLabel={locked ? "Upgrade to unlock" : "Open income report"}
            onClick={() => onAction("/income")}
            locked={locked}
          />
          <ReportTile
            title="Rental income"
            description="One section per rented property — tenant, monthly rent, lease dates, projected annual income."
            icon={<Icons.Home className="h-6 w-6"
                style={{ color: "hsl(31 38% 46%)" }} />}
            ctaLabel={locked ? "Upgrade to unlock" : "Open rental report"}
            onClick={() => onAction("/panels/real-estate")}
            locked={locked}
          />
          <ReportTile
            title="Liability payoff"
            description="Amortization schedule for any fixed-rate liability. Total interest cost, projected payoff date, and a balance trajectory chart."
            icon={<Icons.Activity2 className="h-6 w-6"
                style={{ color: "hsl(31 38% 46%)" }} />}
            ctaLabel={locked ? "Upgrade to unlock" : "Open payoff projection"}
            onClick={() => onAction("/holdings?tab=liabilities")}
            locked={locked}
          />
          <ReportTile
            title="Portfolio health"
            description="0–100 composite score across concentration, FX exposure, cash drag, and allocation drift. Highlights the weakest driver."
            icon={<Icons.PieChart className="h-6 w-6"
                style={{ color: "hsl(31 38% 46%)" }} />}
            ctaLabel={locked ? "Upgrade to unlock" : "Open health report"}
            onClick={() => onAction("/health")}
            locked={locked}
          />
        </div>
        {locked && (
          <p className="text-muted-foreground mt-6 text-center text-xs">
            Gold unlocks scheduled health, drift, cash drag, and weekly AI wealth summaries.
          </p>
        )}
      </PageContent>
    </Page>
  );
}

function ReportTile({
  title,
  description,
  icon,
  ctaLabel,
  onClick,
  locked,
}: {
  title: string;
  description: string;
  icon: React.ReactNode;
  ctaLabel: string;
  onClick: () => void;
  locked: boolean;
}) {
  return (
    <Card
      className={locked ? "border-muted" : ""}
      style={locked ? undefined : { borderColor: "hsl(31 38% 46% / 0.25)" }}
    >
      <CardHeader>
        <div className="flex items-start gap-3">
          {icon}
          <div className="flex-1">
            <CardTitle className="text-base">{title}</CardTitle>
          </div>
        </div>
        <CardDescription className="mt-2">{description}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button variant={locked ? "outline" : "default"} size="sm" onClick={onClick}>
          {ctaLabel}
        </Button>
      </CardContent>
    </Card>
  );
}
