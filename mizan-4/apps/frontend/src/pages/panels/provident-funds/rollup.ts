/**
 * Provident Funds panel rollup — Track B PR-B9 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Provident Funds": donut by scheme (CPF / EPF / 401k /
 * NPS / Super) + CPF sub-donut over the four CPF sub-accounts
 * (OA / SA / MA / RA). Holdings declare scheme membership via
 * `instrument.metadata.providentFund` JSON:
 *
 *   {
 *     "scheme": "CPF" | "EPF" | "401K" | "NPS" | "SUPER" | "OTHER",
 *     "subAccount": "OA" | "SA" | "MA" | "RA" | null,
 *     "sourceAccountId": "uuid-of-parent-pf-account" | null
 *   }
 *
 * The `sourceAccountId` enables cross-asset linkage: a SGS bond held
 * inside a CPF Special Account surfaces both on the Sukuks panel
 * (by issuer) AND in the CPF sub-donut here, with the bond's metadata
 * pointing back at the CPF wrapper account. The §23 reference user
 * has CPF OA + SA + MA + RA balances plus the SRS US-equity holdings.
 *
 * Out of scope for this PR (track separately as PR-B9.a):
 *   - CPF interest accrual line (1Y backtest from monthly statements)
 *   - SRS top-up reminder insight (extends `mizan-insights` rules)
 *   - EPF withdrawal eligibility window banner
 */
import type { Holding } from "@/lib/types";

/** Canonical scheme set. `OTHER` is the catch-all for unrecognised codes. */
export type ProvidentFundScheme =
  | "CPF"
  | "EPF"
  | "401K"
  | "NPS"
  | "SUPER"
  | "OTHER";

/** CPF four-account taxonomy. Other schemes don't sub-divide here. */
export type CpfSubAccount = "OA" | "SA" | "MA" | "RA";

/**
 * Extract the raw provident-fund metadata object via `unknown` cast.
 * Returns `null` when the holding has no PF declaration — those
 * holdings drop out of the panel entirely.
 */
function readProvidentFundMeta(h: Holding): {
  scheme?: string;
  subAccount?: string;
  sourceAccountId?: string;
} | null {
  const inst = h.instrument as unknown as
    | {
        metadata?: { providentFund?: Record<string, unknown> };
      }
    | null
    | undefined;
  const pf = inst?.metadata?.providentFund;
  if (!pf || typeof pf !== "object") return null;
  return pf as {
    scheme?: string;
    subAccount?: string;
    sourceAccountId?: string;
  };
}

/** Predicate: holding has a `metadata.providentFund` declaration. */
export function isProvidentFundHolding(h: Holding): boolean {
  return readProvidentFundMeta(h) !== null;
}

/** Normalise a raw scheme string to the canonical set. */
export function normaliseScheme(raw: string | undefined | null): ProvidentFundScheme {
  if (!raw) return "OTHER";
  const upper = raw.toString().trim().toUpperCase().replace(/[\s_-]+/g, "");
  if (upper === "CPF") return "CPF";
  if (upper === "EPF") return "EPF";
  if (upper === "401K" || upper === "K401") return "401K";
  if (upper === "NPS") return "NPS";
  if (upper === "SUPER" || upper === "SUPERANNUATION") return "SUPER";
  return "OTHER";
}

/** Display label for a scheme — what the donut slice reads. */
export function schemeLabel(scheme: ProvidentFundScheme): string {
  switch (scheme) {
    case "CPF":
      return "CPF";
    case "EPF":
      return "EPF";
    case "401K":
      return "401(k)";
    case "NPS":
      return "NPS";
    case "SUPER":
      return "Superannuation";
    default:
      return "Other";
  }
}

/** Extract the scheme classification for a holding. */
export function extractScheme(h: Holding): ProvidentFundScheme {
  const meta = readProvidentFundMeta(h);
  return normaliseScheme(meta?.scheme);
}

/**
 * Extract the CPF sub-account for a holding. Returns `null` for
 * non-CPF holdings and for CPF holdings without a sub-account
 * declared.
 */
export function extractCpfSubAccount(h: Holding): CpfSubAccount | null {
  const meta = readProvidentFundMeta(h);
  if (normaliseScheme(meta?.scheme) !== "CPF") return null;
  const raw = meta?.subAccount?.toString().trim().toUpperCase();
  if (raw === "OA" || raw === "SA" || raw === "MA" || raw === "RA") return raw;
  return null;
}

/** Extract the source-account-id (cross-asset linkage to the PF wrapper). */
export function extractSourceAccountId(h: Holding): string | null {
  const meta = readProvidentFundMeta(h);
  const raw = meta?.sourceAccountId?.toString().trim();
  return raw && raw.length > 0 ? raw : null;
}

/** Per-scheme donut slice. */
export interface SchemeRow {
  scheme: ProvidentFundScheme;
  label: string;
  totalValueBase: number;
  positionCount: number;
}

/**
 * Roll PF holdings into per-scheme rows, descending by value.
 */
export function rollupByScheme(holdings: readonly Holding[]): SchemeRow[] {
  const map = new Map<ProvidentFundScheme, { sum: number; count: number }>();
  for (const h of holdings) {
    if (!isProvidentFundHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const scheme = extractScheme(h);
    const entry = map.get(scheme) ?? { sum: 0, count: 0 };
    entry.sum += value;
    entry.count += 1;
    map.set(scheme, entry);
  }
  const rows: SchemeRow[] = [];
  for (const [scheme, { sum, count }] of map.entries()) {
    rows.push({
      scheme,
      label: schemeLabel(scheme),
      totalValueBase: sum,
      positionCount: count,
    });
  }
  rows.sort((a, b) => b.totalValueBase - a.totalValueBase);
  return rows;
}

/** CPF sub-account donut row. */
export interface CpfSubAccountRow {
  subAccount: CpfSubAccount;
  totalValueBase: number;
  positionCount: number;
}

/**
 * Roll CPF holdings into per-sub-account rows. Returns OA → SA → MA →
 * RA in that canonical order (NOT desc by value — sub-accounts have
 * a meaningful sequence: liquidity-tier descending).
 *
 * Holdings tagged CPF without a sub-account are silently skipped —
 * they roll up into the parent CPF scheme slice but don't show in
 * the sub-donut.
 */
export function rollupByCpfSubAccount(holdings: readonly Holding[]): CpfSubAccountRow[] {
  const map = new Map<CpfSubAccount, { sum: number; count: number }>();
  for (const h of holdings) {
    if (!isProvidentFundHolding(h)) continue;
    const sub = extractCpfSubAccount(h);
    if (!sub) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const entry = map.get(sub) ?? { sum: 0, count: 0 };
    entry.sum += value;
    entry.count += 1;
    map.set(sub, entry);
  }
  const order: CpfSubAccount[] = ["OA", "SA", "MA", "RA"];
  return order
    .filter((sub) => map.has(sub))
    .map((sub) => {
      const { sum, count } = map.get(sub)!;
      return { subAccount: sub, totalValueBase: sum, positionCount: count };
    });
}

/** Per-position row for the holdings list. */
export interface ProvidentFundPositionRow {
  holdingId: string;
  label: string;
  scheme: ProvidentFundScheme;
  schemeLabel: string;
  subAccount: CpfSubAccount | null;
  sourceAccountId: string | null;
  value: number;
}

/** Roll PF holdings into per-position rows, descending by value. */
export function rollupByPosition(
  holdings: readonly Holding[],
): ProvidentFundPositionRow[] {
  const rows: ProvidentFundPositionRow[] = [];
  for (const h of holdings) {
    if (!isProvidentFundHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const scheme = extractScheme(h);
    rows.push({
      holdingId: h.id,
      label: h.instrument?.name ?? h.instrument?.symbol ?? h.id,
      scheme,
      schemeLabel: schemeLabel(scheme),
      subAccount: extractCpfSubAccount(h),
      sourceAccountId: extractSourceAccountId(h),
      value,
    });
  }
  rows.sort((a, b) => b.value - a.value);
  return rows;
}

/** Sum of all PF exposure in base currency. */
export function totalProvidentFundExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isProvidentFundHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
