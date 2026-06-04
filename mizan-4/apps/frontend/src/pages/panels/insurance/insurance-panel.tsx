/**
 * Insurance panel — Track B PR-B10 / Goal v3 §V Phase 5.
 *
 * Composition:
 *   - Header: total surrender exposure + policy count + stale count
 *   - Donut by category (Investment-Linked / Whole Life / Property /
 *     Term Life / Health / Other) — pure-protection categories
 *     contribute 0 by convention
 *   - Policies list with category + pure-protection chip + stale chip
 *
 * The "stale" chip lights up when `metadata.insurance.lastValuedAt` is
 * absent or older than 7 days. Missing valuation is treated as stale
 * intentionally — we never imply freshness we don't have.
 *
 * Out of scope (track separately as PR-B10.a):
 *   - AI-estimation pipeline for ULIPs lacking declared surrenderValue
 *   - Premium-payment cadence reminder
 *   - Cross-asset linkage from property policy → real-estate holding
 */
import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { Donut } from "@/components/charts";
import { useHoldings } from "@/hooks/use-holdings";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button, Icons } from "@mizan/ui";
import { formatAmount } from "@mizan/ui/lib/utils";

import {
  countStale,
  isInsuranceHolding,
  rollupByCategory,
  rollupByPolicy,
  totalInsuranceExposure,
} from "./rollup";

export default function InsurancePanelPage() {
  const navigate = useNavigate();
  const { holdings: allHoldings, isLoading } = useHoldings(PORTFOLIO_ACCOUNT_ID);
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  // Lock the stale-reference time once per render so the rendered chips
  // and the count agree. Using a fresh Date here is fine — the rollup
  // helpers themselves are deterministic; only the panel reads wall-time.
  const nowMs = useMemo(() => Date.now(), []);

  const insuranceHoldings = useMemo(
    () => (allHoldings ?? []).filter(isInsuranceHolding),
    [allHoldings],
  );
  const totalExposure = useMemo(
    () => totalInsuranceExposure(allHoldings ?? []),
    [allHoldings],
  );
  const categoryRows = useMemo(
    () => rollupByCategory(allHoldings ?? []),
    [allHoldings],
  );
  const policyRows = useMemo(
    () => rollupByPolicy(allHoldings ?? [], nowMs),
    [allHoldings, nowMs],
  );
  const staleCount = useMemo(
    () => countStale(allHoldings ?? [], nowMs),
    [allHoldings, nowMs],
  );

  // Filter zero-value slices from the donut (e.g. all term/health policies)
  const donutSlices = categoryRows.filter((r) => r.totalValueBase > 0);

  return (
    <div className="space-y-6 px-4 py-6 md:px-6 lg:px-10">
      <header className="space-y-2">
        <div className="flex items-center gap-2">
          <Icons.ShieldCheck className="text-muted-foreground h-5 w-5" />
          <h1 className="text-2xl font-semibold tracking-tight">Insurance</h1>
        </div>
        <div className="text-muted-foreground text-sm">
          {insuranceHoldings.length === 0 && !isLoading
            ? "No insurance policies yet."
            : `Surrender ${formatAmount(totalExposure, baseCurrency)} across ${insuranceHoldings.length} ${insuranceHoldings.length === 1 ? "policy" : "policies"}${staleCount > 0 ? ` · ${staleCount} stale` : ""}.`}
        </div>
      </header>

      {insuranceHoldings.length > 0 && (
        <>
          <section
            aria-label="Insurance by category"
            className="bg-card rounded-2xl border p-4"
          >
            <div className="mb-3 text-sm font-medium">By category</div>
            {donutSlices.length === 0 ? (
              <div className="text-muted-foreground py-12 text-center text-sm">
                Pure-protection policies only — no surrender component.
              </div>
            ) : (
              <div className="h-64">
                <Donut
                  data={donutSlices.map((r) => ({
                    label: r.label,
                    value: r.totalValueBase,
                  }))}
                  ariaLabel="Insurance surrender exposure by category"
                  palette="categorical"
                />
              </div>
            )}
          </section>

          <section
            aria-label="Insurance policies"
            className="bg-card rounded-2xl border"
          >
            <header className="border-b px-4 py-3">
              <h2 className="text-sm font-medium">Policies</h2>
            </header>
            <ul role="list" className="divide-y">
              {policyRows.map((p) => (
                <li
                  key={p.holdingId}
                  className="hover:bg-muted/40 flex items-center justify-between px-4 py-3 transition-colors"
                >
                  <button
                    type="button"
                    onClick={() => navigate(`/holdings/${p.holdingId}`)}
                    className="text-foreground flex-1 text-left text-sm font-medium"
                  >
                    <div className="flex items-center gap-2">
                      <span>{p.label}</span>
                      {p.isStale && (
                        <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs">
                          stale
                        </span>
                      )}
                    </div>
                    <div className="text-muted-foreground text-xs">
                      {p.categoryLabel}
                      {p.isPureProtection ? " · Pure protection" : ""}
                      {p.policyNumber ? ` · ${p.policyNumber}` : ""}
                    </div>
                  </button>
                  <div className="text-foreground text-sm font-semibold tabular-nums">
                    {p.isPureProtection && p.valueBase === 0
                      ? "—"
                      : formatAmount(p.valueBase, baseCurrency)}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        </>
      )}

      <div className="pt-2">
        <Button variant="outline" size="sm" onClick={() => navigate("/")}>
          <Icons.ArrowLeft className="mr-2 h-4 w-4" />
          Back to dashboard
        </Button>
      </div>
    </div>
  );
}
