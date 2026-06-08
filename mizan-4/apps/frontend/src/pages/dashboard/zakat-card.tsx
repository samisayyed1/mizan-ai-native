import { Card } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Link } from "react-router-dom";

import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";

/**
 * Home Zakat card (M4.4).
 *
 * Promotes the Zakat assessment as a top-level dashboard tile so Gold users
 * see it without burying it in Settings. Silver users see the same card
 * but tapping it opens the UpgradeGate rather than the page.
 */
export function ZakatCard() {
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();
  const locked = !entitlements.zakatEngine;

  const handleClick = () => {
    if (locked) {
      requestUpgrade("zakat_engine");
    }
  };

  const Body = (
    <div className="flex flex-1 items-start gap-3">
      <Icons.Coins className="text-primary mt-0.5 size-6 shrink-0" />
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-semibold tracking-tight">Zakat assessment</h3>
          {locked && (
            <span className="bg-primary/10 text-primary rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide">
              Gold
            </span>
          )}
        </div>
        <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
          {locked
            ? "Compute this year's Zakat across your liquid cash, precious metals, and tradable assets — minus short-term debts. Upgrade to unlock."
            : "Calculate this year's Zakat across your full portfolio."}
        </p>
      </div>
    </div>
  );

  if (locked) {
    return (
      <Card
        className="bg-card hover:bg-muted/30 border-border/60 flex cursor-pointer items-start gap-3 rounded-xl border p-5 transition-colors"
        onClick={handleClick}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            handleClick();
          }
        }}
      >
        {Body}
      </Card>
    );
  }

  return (
    <Link
      to="/zakat"
      className="focus:ring-primary block rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2"
    >
      <Card className="border-primary/20 flex items-start gap-3 p-5 transition hover:shadow-sm">
        {Body}
      </Card>
    </Link>
  );
}
