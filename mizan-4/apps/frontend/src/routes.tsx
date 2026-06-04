import { Suspense, useEffect, useState } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";

import { AppLayout } from "@/pages/layouts/app-layout";
import { OnboardingLayout } from "@/pages/layouts/onboarding-layout";
import SettingsLayout from "@/pages/settings/settings-layout";

import { getDynamicRoutes, subscribeToNavigationUpdates } from "@/addons/addons-runtime-context";
import { AddAssetProvider } from "@/features/add-asset";
import { UpgradeGateProvider } from "@/features/mizan-connect/providers/upgrade-gate-provider";
import AuthCallbackPage from "@/features/mizan-connect/pages/auth-callback-page";
import ConnectPage from "@/features/mizan-connect/pages/connect-page";
import ActivityManagerPage from "@/pages/activity/activity-manager-page";
import ActivityPage from "@/pages/activity/activity-page";
import ActivityImportPage from "@/pages/activity/import/activity-import-page";
import AssetsPage from "@/pages/asset/assets-page";
import PortfolioPage from "@/pages/dashboard/portfolio-page";
import PortfolioListPage from "@/pages/portfolio/portfolio-list-page";
import AdvisorDashboardPage from "@/pages/advisor/advisor-dashboard-page";
import MonthlyReportsPage from "@/pages/reports/monthly-reports-page";
import ReportsIndexPage from "@/pages/reports/reports-index-page";
import ZakatPage from "@/pages/zakat/zakat-page";
import HoldingsPage from "@/pages/holdings/holdings-page";
import EquitiesPanelPage from "@/pages/panels/equities/equities-panel";
import SukuksPanelPage from "@/pages/panels/sukuks/sukuks-panel";
import IncomePage from "@/pages/income/income-page";
import NewsPage from "@/pages/news/news-page";
import PortfolioInsightsPage from "@/pages/insights/portfolio-insights";
import NotFoundPage from "@/pages/not-found";
import PerformancePage from "@/pages/performance/performance-page";
import SettingsAccountsPage from "@/pages/settings/accounts/accounts-page";
import SettingsAppearancePage from "@/pages/settings/appearance/appearance-page";
import AccountPage from "./pages/account/account-page";
import AiAssistantPage from "./pages/ai-assistant/ai-assistant-page";
import AssetProfilePage from "./pages/asset/asset-profile-page";
import HealthPage from "./pages/health/health-page";
import HoldingsInsightsPage from "./pages/holdings/holdings-insights-page";
import OnboardingPage from "./pages/onboarding/onboarding-page";
import AboutSettingsPage from "./pages/settings/about/about-page";
import AddonSettingsPage from "./pages/settings/addons/addon-settings";
import AiProvidersPage from "./pages/settings/ai-providers/ai-providers-page";
import ContributionLimitPage from "./pages/settings/contribution-limits/contribution-limits-page";
import ExportSettingsPage from "./pages/settings/exports/exports-page";
import GeneralSettingsPage from "./pages/settings/general/general-page";
import MarketDataImportPage from "./pages/settings/market-data/market-data-import-page";
import MarketDataSettingsPage from "./pages/settings/market-data/market-data-settings";
import TaxonomiesPage from "./pages/settings/taxonomies/taxonomies-page";
import ConnectSettingsPage from "./pages/settings/mizan-connect/connect-settings-page";
import GoalsDashboardPage from "@/features/goals/pages/goals-dashboard-page";
import GoalNewPage from "@/features/goals/pages/goal-new-page";
import GoalDetailPage from "@/features/goals/pages/goal-detail-page";
import GoalRetirementGuidePage from "@/features/goals/pages/goal-retirement-guide-page";

export function AppRoutes() {
  const [dynamicRoutes, setDynamicRoutes] = useState<
    { path: string; component: React.LazyExoticComponent<React.ComponentType<unknown>> }[]
  >([]);

  // Subscribe to dynamic route updates
  useEffect(() => {
    const updateRoutes = () => {
      setDynamicRoutes(getDynamicRoutes());
    };

    // Initial load
    updateRoutes();

    // Subscribe to updates
    const unsubscribe = subscribeToNavigationUpdates(updateRoutes);

    return () => {
      unsubscribe();
    };
  }, []);

  return (
    <BrowserRouter>
      <UpgradeGateProvider>
        <AddAssetProvider>
          <Routes>
            {/* QR Scanner - No layout for fullscreen camera access */}
            {/* <Route path="/qr-scanner" element={<QRScannerPage />} /> */}

            {/* Auth callback - PKCE code exchange handler for Mizan Connect */}
            <Route path="/auth/callback" element={<AuthCallbackPage />} />

            {/* Onboarding with dedicated layout */}
            <Route path="/onboarding" element={<OnboardingLayout />}>
              <Route index element={<OnboardingPage />} />
            </Route>

            {/* Main app with sidebar */}
            <Route path="/" element={<AppLayout />}>
              <Route index element={<PortfolioPage />} />
              <Route path="dashboard" element={<PortfolioPage />} />
              <Route path="portfolio" element={<PortfolioListPage />} />
              <Route path="zakat" element={<ZakatPage />} />
              <Route path="reports" element={<ReportsIndexPage />} />
              <Route path="reports/monthly" element={<MonthlyReportsPage />} />
              <Route path="advisor" element={<AdvisorDashboardPage />} />
              <Route path="activities" element={<ActivityPage />} />
              <Route path="activities/manage" element={<ActivityManagerPage />} />
              <Route path="holdings" element={<HoldingsPage />} />
              {/* Track B PR-B1 — Sukuks panel (§23 anchor surface). Track B
                  PR-B2..B7 add the rest of the twelve panels under
                  /panels/* as they ship. */}
              <Route path="panels/sukuks" element={<SukuksPanelPage />} />
              {/* Track B PR-B2 — Equities panel (sub-class donut + region bar). */}
              <Route path="panels/equities" element={<EquitiesPanelPage />} />
              <Route path="news" element={<NewsPage />} />
              <Route path="holdings-insights" element={<HoldingsInsightsPage />} />
              <Route path="holdings/:assetId" element={<AssetProfilePage />} />
              <Route path="import" element={<ActivityImportPage />} />
              <Route path="accounts/:id" element={<AccountPage />} />
              <Route path="income" element={<IncomePage />} />
              <Route path="performance" element={<PerformancePage />} />
              <Route path="insights" element={<PortfolioInsightsPage />} />
              <Route path="health" element={<HealthPage />} />
              <Route path="assistant" element={<AiAssistantPage />} />
              <Route path="connect" element={<ConnectPage />} />
              <Route path="goals" element={<GoalsDashboardPage />} />
              <Route path="goals/new" element={<GoalNewPage />} />
              <Route path="goals/:goalId/guide" element={<GoalRetirementGuidePage />} />
              <Route path="goals/:goalId" element={<GoalDetailPage />} />
              {/* Dynamic addon routes */}
              {dynamicRoutes.map(({ path, component: Component }) => (
                <Route
                  key={path}
                  path={path}
                  element={
                    <Suspense
                      fallback={
                        <div className="flex h-64 items-center justify-center">Loading...</div>
                      }
                    >
                      <Component />
                    </Suspense>
                  }
                />
              ))}
              <Route path="settings" element={<SettingsLayout />}>
                <Route index element={<GeneralSettingsPage />} />
                <Route path="general" element={<GeneralSettingsPage />} />
                <Route path="accounts" element={<SettingsAccountsPage />} />
                <Route path="appearance" element={<SettingsAppearancePage />} />
                <Route path="about" element={<AboutSettingsPage />} />
                <Route path="exports" element={<ExportSettingsPage />} />
                <Route path="contribution-limits" element={<ContributionLimitPage />} />
                <Route path="market-data" element={<MarketDataSettingsPage />} />
                <Route path="market-data/import" element={<MarketDataImportPage />} />
                <Route path="securities" element={<AssetsPage />} />
                <Route path="taxonomies" element={<TaxonomiesPage />} />
                <Route path="connect" element={<ConnectSettingsPage />} />
                <Route path="ai-providers" element={<AiProvidersPage />} />
                <Route path="addons" element={<AddonSettingsPage />} />
              </Route>
              <Route path="*" element={<NotFoundPage />} />
            </Route>
          </Routes>
        </AddAssetProvider>
      </UpgradeGateProvider>
    </BrowserRouter>
  );
}
