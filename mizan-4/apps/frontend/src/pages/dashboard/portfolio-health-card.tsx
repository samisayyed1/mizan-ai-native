import { Card } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Link } from "react-router-dom";

import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";

/**
 * Home portfolio-health card.
 *
 * Renders a compact health summary on the Home dashboard. Pro users see
 * a live placeholder pointing to the full report (the live score is
 * computed via `compute_portfolio_health` when they open the report);
 * Silver users see a Gold monitoring teaser that opens the UpgradeGate.
 *
 * Keeping this card free of an inline `compute_portfolio_health` call
 * for now so it doesn't trigger the Pro gate every Home render — the
 * full breakdown lives on the dedicated report page.
 */
export function PortfolioHealthCard() {
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();
  const locked = !entitlements.advancedReports;

  if (locked) {
    return (
      <Card
        className="border-primary/20 from-primary/5 to-card bg-linear-to-br flex cursor-pointer items-start gap-3 p-5 transition hover:shadow-sm"
        onClick={() => requestUpgrade("advanced_reports")}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            requestUpgrade("advanced_reports");
          }
        }}
      >
        <Icons.PieChart className="text-primary mt-0.5 size-6 shrink-0" />
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold tracking-tight">Portfolio health</h3>
            <span className="bg-primary/10 text-primary rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide">
              Pro
            </span>
          </div>
          <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
            A 0-100 score across concentration, FX, cash drag, and allocation drift — with the
            weakest driver called out. Upgrade to unlock.
          </p>
        </div>
      </Card>
    );
  }

  return (
    <Link
      to="/reports"
      className="focus:ring-primary block rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2"
    >
      <Card className="border-primary/20 flex items-start gap-3 p-5 transition hover:shadow-sm">
        <Icons.PieChart className="text-primary mt-0.5 size-6 shrink-0" />
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold tracking-tight">Portfolio health</h3>
          </div>
          <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
            See your concentration, FX, cash drag, and allocation-drift scores in one place. Open
            the report →
          </p>
        </div>
      </Card>
    </Link>
  );
}
