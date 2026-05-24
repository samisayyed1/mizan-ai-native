// Deterministic report adapters used by Assistant-native summaries.
//
// Thin wrappers over `compute_amortization` and `compute_portfolio_health`
// Tauri commands. Both are Pro-gated server-side; on Free/Basic the call
// rejects with a `GatedError("advanced_reports")` that the central
// UpgradeGate handler intercepts.

import { invoke } from "./platform";

// ---------------------------------------------------------------------------
// Amortization
// ---------------------------------------------------------------------------

/** Mirrors `crates/core/src/portfolio/amortization::AmortizationInputs`. */
export interface AmortizationInputs {
  /** Current outstanding principal balance, decimal string. */
  currentBalance: string;
  /** Annual interest rate as a decimal (e.g. "0.0625" for 6.25%). */
  annualRate: string;
  /**
   * Optional monthly payment. When `null`, the service derives the EMI
   * from balance + rate + remainingMonths.
   */
  monthlyPayment?: string | null;
  /** Optional remaining term in months. Defaults to 360 (50-yr cap). */
  remainingMonths?: number | null;
  /** Schedule start date (typically today), ISO date string `YYYY-MM-DD`. */
  startDate: string;
  /** Currency for display only. */
  currency?: string | null;
}

/** Mirrors `Installment`. */
export interface Installment {
  period: number;
  dueDate: string;
  payment: string;
  interest: string;
  principal: string;
  balanceAfter: string;
}

/** Mirrors `AmortizationReport`. */
export interface AmortizationReport {
  monthlyPayment: string;
  totalPaid: string;
  totalInterest: string;
  totalPrincipal: string;
  payoffDate: string;
  schedule: Installment[];
  currency: string | null;
  notes: string[];
}

export async function computeAmortization(inputs: AmortizationInputs): Promise<AmortizationReport> {
  return invoke<AmortizationReport>("compute_amortization", { inputs });
}

// ---------------------------------------------------------------------------
// Portfolio health
// ---------------------------------------------------------------------------

/** Mirrors `crates/core/src/portfolio/health::HealthPosition`. */
export interface HealthPosition {
  label: string;
  assetClass: string;
  valueBase: string;
  isForeignCurrency: boolean;
  isCash: boolean;
}

/** Mirrors `HealthInputs`. */
export interface HealthInputs {
  positions: HealthPosition[];
  /** Map of asset class → target weight (fraction of 1). */
  targetAllocation?: Record<string, string>;
  baseCurrency?: string | null;
}

/** Mirrors `HealthDriver`. */
export interface HealthDriver {
  id: string;
  label: string;
  /** 0–100, higher = healthier. */
  score: string;
  /** Underlying fraction (e.g. "0.45" for 45%). */
  metric: string;
  note: string;
}

/** Mirrors `HealthReport`. */
export interface HealthReport {
  /** Composite 0–100 score, or null for empty portfolios. */
  score: string | null;
  drivers: HealthDriver[];
  worstDriver: HealthDriver | null;
  baseCurrency: string | null;
  notes: string[];
}

export async function computePortfolioHealth(inputs: HealthInputs): Promise<HealthReport> {
  return invoke<HealthReport>("compute_portfolio_health", { inputs });
}
