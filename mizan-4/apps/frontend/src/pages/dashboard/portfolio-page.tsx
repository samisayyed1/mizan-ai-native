import { SwipablePage, SwipablePageView } from "@/components/page";
import { PrivacyToggle } from "@/components/privacy-toggle";
import { useAddAsset } from "@/features/add-asset";
import { useNavigationMode } from "@/pages/layouts/navigation/navigation-mode-context";
import { Button, Icons } from "@mizan/ui";
import { Card, CardContent, CardHeader } from "@mizan/ui/components/ui/card";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Suspense, useCallback, useMemo } from "react";
import { DashboardActions } from "./dashboard-actions";
import { DashboardContent } from "./dashboard-content";

// Loading skeleton
const PageLoader = () => (
  <div className="flex h-full w-full flex-col space-y-4 p-4">
    <Card>
      <CardHeader className="space-y-2">
        <Skeleton className="h-8 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 md:grid-cols-3">
          <Skeleton className="h-32 w-full" />
          <Skeleton className="h-32 w-full" />
          <Skeleton className="h-32 w-full" />
        </div>
        <Skeleton className="h-64 w-full" />
      </CardContent>
    </Card>
  </div>
);

export default function PortfolioPage() {
  const { isFocusMode, toggleFocusMode } = useNavigationMode();
  const addAsset = useAddAsset();

  const handleAddAsset = useCallback(() => {
    addAsset.open();
  }, [addAsset]);

  // Net-worth liabilities flow through the same inline AddAssetDialog so
  // users never get yanked into a separate assistant page (see UX-1).
  // We seed a liability-specific prompt so the dialog's Mizan AI tab
  // opens already pointing at the right capability.
  const handleAddLiability = useCallback(() => {
    addAsset.open({
      source: "portfolio",
      prompt:
        "Tell me about the liability you want to track — for example: my mortgage balance, a student loan, a credit card balance, or a car loan.",
    });
  }, [addAsset]);

  // UX-10: HealthStatusIndicator removed from the customer-facing
  // dashboard per user request — health warnings were leaking
  // diagnostic-grade detail (FX gaps, missing quotes, classification
  // drift) into the headline action row. The backend health checks
  // still run and self-correct via the existing scheduler; the /health
  // route remains accessible for support, just no longer linked from
  // the dashboard chrome.
  const commonActions = useMemo(
    () => (
      <>
        {isFocusMode && (
          <Button
            variant="secondary"
            size="icon-xs"
            className="bg-secondary/50 rounded-full"
            onClick={toggleFocusMode}
          >
            <Icons.Fullscreen className="size-5" />
          </Button>
        )}
        <PrivacyToggle />
      </>
    ),
    [isFocusMode, toggleFocusMode],
  );

  // ADR 0018b: the dashboard absorbed the Net Worth tab, so the
  // Add Asset / Add Liability affordances live here too — the dropdown
  // routes both through the inline AddAssetDialog (UX-1) so users never
  // get yanked to a separate page.
  const investmentActions = useMemo(
    () => (
      <>
        {commonActions}
        <DashboardActions onAddAsset={handleAddAsset} onAddLiability={handleAddLiability} />
      </>
    ),
    [commonActions, handleAddAsset, handleAddLiability],
  );

  // ADR 0018b: the dashboard is a single composed surface — Investments
  // and Net Worth are no longer split into top-level tabs. Net Worth
  // detail is now a tap-through from the Net Worth strip inside the
  // dashboard (/net-worth route remains accessible directly). This
  // collapses the IA so the user lands on one canonical view.
  const views: SwipablePageView[] = useMemo(
    () => [
      {
        value: "investments",
        label: "Investments",
        icon: Icons.TrendingUp,
        content: (
          <Suspense fallback={<PageLoader />}>
            <DashboardContent />
          </Suspense>
        ),
        actions: investmentActions,
      },
    ],
    [investmentActions],
  );

  return <SwipablePage className="pt-0" views={views} defaultView="investments" withPadding={false} />;
}
