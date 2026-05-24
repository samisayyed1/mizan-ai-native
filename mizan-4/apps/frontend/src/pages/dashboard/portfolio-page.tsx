import { HealthStatusIndicator } from "@/components/health-status-icon";
import { SwipablePage, SwipablePageView } from "@/components/page";
import { PrivacyToggle } from "@/components/privacy-toggle";
import { useAddAsset } from "@/features/add-asset";
import { useNavigationMode } from "@/pages/layouts/navigation/navigation-mode-context";
import { Button, Icons } from "@mizan/ui";
import { Card, CardContent, CardHeader } from "@mizan/ui/components/ui/card";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Suspense, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { NetWorthContent } from "../net-worth/net-worth-content";
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
  const navigate = useNavigate();

  const handleAddAsset = useCallback(() => {
    addAsset.open();
  }, [addAsset]);

  const handleAddLiability = useCallback(() => {
    const prompt =
      "Tell me about the liability you want to track — for example: my mortgage balance, a student loan, a credit card balance, or a car loan.";
    navigate(`/assistant?intent=add-asset&prompt=${encodeURIComponent(prompt)}`);
  }, [navigate]);

  const commonActions = (
    <>
      <HealthStatusIndicator />
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
  );

  const investmentActions = (
    <>
      {commonActions}
      <DashboardActions />
    </>
  );

  const netWorthActions = (
    <>
      {commonActions}
      <DashboardActions onAddAsset={handleAddAsset} onAddLiability={handleAddLiability} />
    </>
  );

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
      {
        value: "net-worth",
        label: "Net Worth",
        icon: Icons.Wallet,
        content: (
          <Suspense fallback={<PageLoader />}>
            <NetWorthContent onAddAsset={handleAddAsset} onAddLiability={handleAddLiability} />
          </Suspense>
        ),
        actions: netWorthActions,
      },
    ],
    [investmentActions, netWorthActions, handleAddAsset, handleAddLiability],
  );

  return <SwipablePage className="pt-0" views={views} defaultView="investments" withPadding={false} />;
}
