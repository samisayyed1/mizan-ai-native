import { Page, PageContent, PageHeader, PrivacyAmount } from "@mizan/ui";
import { Button } from "@mizan/ui/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@mizan/ui/components/ui/card";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Input } from "@mizan/ui/components/ui/input";
import { Label } from "@mizan/ui/components/ui/label";
import { Separator } from "@mizan/ui/components/ui/separator";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { computeZakat, type ZakatReport } from "@/adapters";
import { useEntitlements, useUpgradeGate } from "@/features/mizan-connect";
import { useSettingsContext } from "@/lib/settings-provider";

/**
 * Zakat & Purification assessment page (M3.7, Gold feature).
 *
 * Wraps the pure-math service shipped in `crates/core/src/portfolio/zakat/`.
 * The Tauri command applies the Gold gate; on Silver the mutation rejects
 * with a GatedError("advanced_reports"), the central UpgradeGate raises a
 * modal, and this page never has to think about the cap.
 *
 * Disclaimer: the Rust service emits the "not religious guidance, confirm
 * with your imam" note; we render every note verbatim.
 */
export default function ZakatPage() {
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";
  const { entitlements } = useEntitlements();
  const { requestUpgrade } = useUpgradeGate();

  const [nisab, setNisab] = useState("");

  const mutation = useMutation<ZakatReport, Error, string>({
    mutationFn: (n) => computeZakat({ nisab: n }),
  });
  const report = mutation.data;
  const handleCompute = () => {
    if (!entitlements.advancedReports) {
      requestUpgrade("advanced_reports");
      return;
    }
    if (!nisab.trim()) return;
    mutation.mutate(nisab.trim());
  };

  return (
    <Page>
      <PageHeader
        heading="Zakat & Purification"
        text={`Computes 2.5% on your zakatable wealth (in ${baseCurrency}) after subtracting short-term debts. Gold feature.`}
      />
      <PageContent>
        <div className="mx-auto w-full max-w-2xl space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Inputs</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="nisab">Nisab threshold ({baseCurrency})</Label>
                <Input
                  id="nisab"
                  inputMode="decimal"
                  placeholder="e.g. 5000"
                  value={nisab}
                  onChange={(e) => setNisab(e.target.value)}
                  className="mt-1.5"
                />
                <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
                  The Nisab is the minimum threshold below which Zakat is not owed. Common values
                  are the gold-Nisab (~87.48g of gold) or silver-Nisab (~612.36g of silver) at
                  current market price; check your local imam for the standard you follow.
                </p>
              </div>
              <Button
                onClick={handleCompute}
                disabled={mutation.isPending || !nisab.trim()}
                className="w-full sm:w-auto"
              >
                {mutation.isPending ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Computing…
                  </>
                ) : (
                  <>
                    <Icons.Coins className="mr-2 h-4 w-4" />
                    Compute Zakat
                  </>
                )}
              </Button>
              {mutation.isError && (
                <p className="text-destructive text-sm">{mutation.error.message}</p>
              )}
            </CardContent>
          </Card>

          {report && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Result</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <ResultRow
                    label="Total assessable assets"
                    amount={report.totalAssessableAssets}
                    currency={report.currency ?? baseCurrency}
                  />
                  <ResultRow
                    label="Deductible short-term debts"
                    amount={report.deductibleDebts}
                    currency={report.currency ?? baseCurrency}
                  />
                  <ResultRow
                    label="Net Zakatable base"
                    amount={report.netZakatBase}
                    currency={report.currency ?? baseCurrency}
                  />
                  <ResultRow
                    label="Nisab threshold"
                    amount={report.nisabThreshold}
                    currency={report.currency ?? baseCurrency}
                  />
                </div>
                <Separator className="my-5" />
                <div className="text-center">
                  <p className="text-muted-foreground text-xs uppercase tracking-wider">
                    Zakat due
                  </p>
                  <p className="text-primary mt-1 text-4xl font-bold tabular-nums">
                    <PrivacyAmount
                      value={Number(report.zakatDue)}
                      currency={report.currency ?? baseCurrency}
                    />
                  </p>
                  <p className="text-muted-foreground mt-2 text-xs">
                    {report.isAboveNisab
                      ? "Your net Zakatable wealth crosses Nisab — 2.5% is due."
                      : "Your net Zakatable wealth is below Nisab — no Zakat is owed this year."}
                  </p>
                </div>
                {report.notes.length > 0 && (
                  <div className="bg-muted/40 mt-5 space-y-2 rounded-md border p-4">
                    {report.notes.map((note, i) => (
                      <p key={i} className="text-muted-foreground text-xs leading-relaxed">
                        {note}
                      </p>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          )}
        </div>
      </PageContent>
    </Page>
  );
}

function ResultRow({
  label,
  amount,
  currency,
}: {
  label: string;
  amount: string;
  currency: string;
}) {
  return (
    <div>
      <p className="text-muted-foreground text-xs uppercase tracking-wide">{label}</p>
      <p className="text-foreground mt-1 text-base font-medium tabular-nums">
        <PrivacyAmount value={Number(amount)} currency={currency} />
      </p>
    </div>
  );
}
