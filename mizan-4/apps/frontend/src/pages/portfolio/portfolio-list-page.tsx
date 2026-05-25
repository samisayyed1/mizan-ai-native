import { Page, PageContent, PageHeader } from "@mizan/ui";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { useNavigate } from "react-router-dom";

import { AccountsSummary } from "@/pages/dashboard/accounts-summary";

/**
 * M2 — top-level Portfolio listing page.
 *
 * Renders the same portfolio-summary the dashboard already builds — but as a
 * dedicated page so the new five-tab nav has a real destination for
 * `/portfolio` (the previous nav exposed the dashboard at `/` and the per-
 * portfolio drill-down at `/accounts/:id`, with no single "list of all my
 * portfolios" page in between).
 *
 * Header CTA is "Add portfolio" — because you are looking at the list OF
 * portfolios, the natural verb is "make another one." Previously the CTA
 * said "Add asset" and opened the AddAssetDialog, which was wrong on this
 * surface: the user has to pick a portfolio first before adding an asset,
 * so the asset-add flow belongs inside the per-portfolio drill-down (where
 * the AddAssetDialog already lives). Here we route directly to the
 * AccountEditModal (auto-opened via ?addAccount=1) so adding a new
 * portfolio is one click, not three.
 */
export default function PortfolioListPage() {
  const navigate = useNavigate();

  return (
    <Page>
      <PageHeader
        heading="Portfolios"
        actions={
          <Button size="sm" onClick={() => navigate("/settings/accounts?addAccount=1")}>
            <Icons.Plus className="mr-1.5 h-4 w-4" />
            Add portfolio
          </Button>
        }
      />
      <PageContent>
        <AccountsSummary />
      </PageContent>
    </Page>
  );
}
