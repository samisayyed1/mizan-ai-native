// Zakat assessment adapter (M3.7).
//
// Thin wrapper around the Tauri `compute_zakat` command. Gold-gated server-
// side (per Feroz 25 May 2026); on Silver the call rejects with a
// `GatedError("zakat_engine")` which the central UpgradeGate handler picks up.

import { invoke } from "./platform";

/** Mirrors `crates/core/src/portfolio/zakat/zakat_model.rs::ZakatReport`. */
export interface ZakatReport {
  /** Liquid cash + precious metals + tradable assets (base currency). */
  totalAssessableAssets: string;
  /** Short-term debts subtracted from the assessable base. */
  deductibleDebts: string;
  /** `totalAssessableAssets - deductibleDebts`. */
  netZakatBase: string;
  /** The user-provided Nisab threshold (same currency). */
  nisabThreshold: string;
  /** True iff `netZakatBase >= nisab` and positive. */
  isAboveNisab: boolean;
  /** 2.5% of `netZakatBase` when above Nisab, else `"0"`. */
  zakatDue: string;
  /** Currency the amounts are denominated in (defaults to user's base). */
  currency: string | null;
  /** UX disclaimers (e.g. "this isn't religious guidance — confirm with your imam"). */
  notes: string[];
}

export interface ComputeZakatInput {
  /** Nisab threshold expressed in the user's base currency, as a decimal string. */
  nisab: string;
  /** Override the user's base currency (rare — used for testing different schools). */
  baseCurrencyOverride?: string;
}

export async function computeZakat(input: ComputeZakatInput): Promise<ZakatReport> {
  return invoke<ZakatReport>("compute_zakat", {
    nisab: input.nisab,
    baseCurrencyOverride: input.baseCurrencyOverride ?? null,
  });
}
