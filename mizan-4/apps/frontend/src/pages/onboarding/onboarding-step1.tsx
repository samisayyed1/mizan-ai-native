import { Icons } from "@mizan/ui/components/ui/icons";
import React from "react";

/**
 * M2 onboarding step 1 — friendly welcome screen.
 *
 * Previous content asked first-time users to pick between "Holdings tracking"
 * and "Transactions tracking" up front, surfacing jargon they hadn't asked
 * for and bouncing non-power users. We push that decision down into the
 * per-portfolio setup (each portfolio chooses its own mode there), and keep
 * step 1 to a single sentence: what Mizan is, in one line.
 */
export const OnboardingStep1: React.FC = () => {
  return (
    <div className="w-full max-w-2xl space-y-8 text-center md:space-y-10">
      <div className="space-y-3">
        <h1 className="text-3xl font-bold tracking-tight md:text-4xl">Welcome to Mizan</h1>
        <p className="text-muted-foreground mx-auto max-w-md text-base leading-relaxed md:text-lg">
          Your private wealth, finally clear. Track stocks, sukuk, property, bank accounts, gold,
          and liabilities — all in one place, on your own device.
        </p>
      </div>

      <div className="bg-card text-card-foreground mx-auto max-w-md rounded-2xl border p-6 text-left shadow-sm">
        <div className="space-y-4">
          <div className="flex items-start gap-3">
            <Icons.ShieldCheck className="text-primary mt-0.5 h-5 w-5 shrink-0" />
            <div>
              <p className="text-sm font-semibold">Yours, on your device</p>
              <p className="text-muted-foreground text-xs leading-relaxed">
                Everything is stored locally first. Cloud sync is optional and end-to-end encrypted.
              </p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <Icons.Sparkles className="text-primary mt-0.5 h-5 w-5 shrink-0" />
            <div>
              <p className="text-sm font-semibold">Two minutes to set up</p>
              <p className="text-muted-foreground text-xs leading-relaxed">
                Pick a currency, name your first portfolio, and add one asset. That's it.
              </p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <Icons.Shield className="text-primary mt-0.5 h-5 w-5 shrink-0" />
            <div>
              <p className="text-sm font-semibold">No accounts required</p>
              <p className="text-muted-foreground text-xs leading-relaxed">
                You can use Mizan offline forever. Sign in to Mizan Connect later if you want
                cross-device sync or managed AI.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
