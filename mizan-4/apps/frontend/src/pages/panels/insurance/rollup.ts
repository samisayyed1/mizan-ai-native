/**
 * Insurance panel rollup — Track B PR-B10 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Insurance" and per the autonomous directive at
 * `Mizan_Continue_Autonomous.md` line 25-27: donut by category
 * (Investment-Linked / Term Life / Whole Life / Health / Property /
 * Other), ULIP surrender-value computation, stale badges when
 * `lastValuedAt` is more than 7 days old.
 *
 * Holdings declare insurance membership via `instrument.metadata.insurance`
 * JSON:
 *
 *   {
 *     "category": "INVESTMENT_LINKED" | "TERM_LIFE" | "WHOLE_LIFE"
 *               | "HEALTH" | "PROPERTY" | "OTHER",
 *     "surrenderValue": 12345.67,   // base-ccy, optional
 *     "cashValue":      12345.67,   // base-ccy, optional, ULIP fallback
 *     "lastValuedAt":   "2026-05-20T00:00:00Z",  // ISO timestamp
 *     "policyNumber":   "MIZ-..."
 *   }
 *
 * Value-resolution order for donut + list:
 *   1. surrenderValue (the cashable component on demand)
 *   2. cashValue       (ULIP unit account value)
 *   3. holding.marketValue.base (fallback so the row always renders)
 *
 * Pure protection covers (TERM_LIFE, HEALTH) carry surrenderValue=0 by
 * convention — they show up in the donut at value 0 (i.e. don't slice
 * the donut) but still appear in the list with a "Pure protection"
 * sub-label. Investment-linked + whole-life carry positive surrender.
 *
 * Stale: `isStaleAt(h, nowMs)` returns true when `lastValuedAt` is
 * absent OR older than 7 calendar days. Stale rows render a chip in
 * the list and are flagged in `staleCount`.
 *
 * Out of scope (track separately as PR-B10.a):
 *   - AI-estimation pipeline for ULIPs lacking declared surrenderValue
 *   - Premium-payment cadence reminder (extends mizan-insights)
 *   - Cross-asset linkage from a property policy → real-estate holding
 */
import type { Holding } from "@/lib/types";

/** Canonical category set. `OTHER` is the catch-all. */
export type InsuranceCategory =
  | "INVESTMENT_LINKED"
  | "TERM_LIFE"
  | "WHOLE_LIFE"
  | "HEALTH"
  | "PROPERTY"
  | "OTHER";

/** 7 days in milliseconds — stale threshold per the directive. */
export const STALE_THRESHOLD_MS = 7 * 24 * 60 * 60 * 1000;

/**
 * Categories that DO NOT have a surrender component — pure protection
 * cover. They display in the list but don't slice the donut.
 */
export function isPureProtection(category: InsuranceCategory): boolean {
  return category === "TERM_LIFE" || category === "HEALTH";
}

/**
 * Coerce a number or numeric-string to a non-negative finite number.
 * Returns null when the input is missing, non-numeric, NaN, or
 * negative. Avoids `String(value)` on arbitrary unknowns which would
 * stringify objects to `[object Object]`.
 */
function coerceNonNegativeNumber(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? value : null;
  }
  if (typeof value === "string") {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
  }
  return null;
}

function readInsuranceMeta(h: Holding): {
  category?: string;
  surrenderValue?: unknown;
  cashValue?: unknown;
  lastValuedAt?: string;
  policyNumber?: string;
} | null {
  const inst = h.instrument as unknown as
    | { metadata?: { insurance?: Record<string, unknown> } }
    | null
    | undefined;
  const meta = inst?.metadata?.insurance;
  if (!meta || typeof meta !== "object") return null;
  return meta as {
    category?: string;
    surrenderValue?: unknown;
    cashValue?: unknown;
    lastValuedAt?: string;
    policyNumber?: string;
  };
}

/** Predicate: holding has a `metadata.insurance` declaration. */
export function isInsuranceHolding(h: Holding): boolean {
  return readInsuranceMeta(h) !== null;
}

/** Normalise a raw category string to the canonical set. */
export function normaliseCategory(
  raw: string | undefined | null,
): InsuranceCategory {
  if (!raw) return "OTHER";
  const upper = raw.toString().trim().toUpperCase().replace(/[\s-]+/g, "_");
  if (upper === "INVESTMENT_LINKED" || upper === "ULIP" || upper === "ILP")
    return "INVESTMENT_LINKED";
  if (upper === "TERM_LIFE" || upper === "TERM") return "TERM_LIFE";
  if (upper === "WHOLE_LIFE" || upper === "WHOLE") return "WHOLE_LIFE";
  if (upper === "HEALTH" || upper === "MEDICAL") return "HEALTH";
  if (upper === "PROPERTY" || upper === "HOME") return "PROPERTY";
  return "OTHER";
}

/** Display label for a category — what the donut slice reads. */
export function categoryLabel(c: InsuranceCategory): string {
  switch (c) {
    case "INVESTMENT_LINKED":
      return "Investment-Linked";
    case "TERM_LIFE":
      return "Term Life";
    case "WHOLE_LIFE":
      return "Whole Life";
    case "HEALTH":
      return "Health";
    case "PROPERTY":
      return "Property";
    default:
      return "Other";
  }
}

/** Extract category for a holding. */
export function extractCategory(h: Holding): InsuranceCategory {
  const meta = readInsuranceMeta(h);
  return normaliseCategory(meta?.category);
}

/**
 * Extract the "displayed value" for a holding — surrenderValue if
 * declared, else cashValue, else holding.marketValue.base. Pure-
 * protection categories return 0 unless an explicit surrender is
 * declared (cash-out riders).
 *
 * Returns 0 (NOT null) so totals + donut slices have a safe default.
 */
export function extractSurrenderValueBase(h: Holding): number {
  const meta = readInsuranceMeta(h);
  if (!meta) return 0;

  // Try surrenderValue (accept number or numeric string only)
  const surrender = coerceNonNegativeNumber(meta.surrenderValue);
  if (surrender !== null) return surrender;

  // Then cashValue
  const cash = coerceNonNegativeNumber(meta.cashValue);
  if (cash !== null) return cash;

  // Pure protection has no displayed value
  if (isPureProtection(normaliseCategory(meta.category))) return 0;

  // Fall back to market value (clamped to ≥ 0)
  const mv = h.marketValue?.base ?? 0;
  return Number.isFinite(mv) && mv > 0 ? mv : 0;
}

/** Parse an ISO timestamp into ms. Returns null on failure. */
function parseTimestamp(iso: string | undefined): number | null {
  if (!iso) return null;
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : null;
}

/**
 * Is this holding's last valuation stale (older than 7 days) relative
 * to `nowMs`? Missing `lastValuedAt` is treated as stale — we never
 * imply a fresh valuation we don't actually have.
 *
 * `nowMs` is taken as a parameter (not read from `Date.now()`) so
 * tests are deterministic.
 */
export function isStaleAt(h: Holding, nowMs: number): boolean {
  const meta = readInsuranceMeta(h);
  if (!meta) return false; // not insurance — caller filters
  const valuedAt = parseTimestamp(meta.lastValuedAt);
  if (valuedAt === null) return true; // missing → stale
  return nowMs - valuedAt > STALE_THRESHOLD_MS;
}

/** Per-category donut slice (excludes pure-protection zeros). */
export interface CategoryRow {
  category: InsuranceCategory;
  label: string;
  totalValueBase: number;
  policyCount: number;
}

/**
 * Roll insurance holdings into per-category rows desc by value.
 * Zero-value slices ARE included if at least one policy lives there —
 * the donut renderer skips them visually, but the policy count surfaces
 * "you have 2 health policies (pure protection)" in the panel header.
 */
export function rollupByCategory(holdings: readonly Holding[]): CategoryRow[] {
  const map = new Map<InsuranceCategory, { sum: number; count: number }>();
  for (const h of holdings) {
    if (!isInsuranceHolding(h)) continue;
    const category = extractCategory(h);
    const value = extractSurrenderValueBase(h);
    const entry = map.get(category) ?? { sum: 0, count: 0 };
    entry.sum += Number.isFinite(value) && value > 0 ? value : 0;
    entry.count += 1;
    map.set(category, entry);
  }
  const rows: CategoryRow[] = [];
  for (const [category, { sum, count }] of map.entries()) {
    rows.push({
      category,
      label: categoryLabel(category),
      totalValueBase: sum,
      policyCount: count,
    });
  }
  rows.sort((a, b) => b.totalValueBase - a.totalValueBase);
  return rows;
}

/** Per-policy row for the list. */
export interface InsurancePolicyRow {
  holdingId: string;
  label: string;
  category: InsuranceCategory;
  categoryLabel: string;
  valueBase: number;
  isPureProtection: boolean;
  isStale: boolean;
  policyNumber: string | null;
}

/**
 * Roll insurance holdings into per-policy rows desc by surrender value.
 * Pure-protection policies stay in the list (with their pure-protection
 * sub-label) even though their valueBase is 0.
 *
 * `nowMs` is passed so the stale flag is deterministic in tests.
 */
export function rollupByPolicy(
  holdings: readonly Holding[],
  nowMs: number,
): InsurancePolicyRow[] {
  const rows: InsurancePolicyRow[] = [];
  for (const h of holdings) {
    if (!isInsuranceHolding(h)) continue;
    const category = extractCategory(h);
    const meta = readInsuranceMeta(h);
    rows.push({
      holdingId: h.id,
      label: h.instrument?.name ?? h.instrument?.symbol ?? h.id,
      category,
      categoryLabel: categoryLabel(category),
      valueBase: extractSurrenderValueBase(h),
      isPureProtection: isPureProtection(category),
      isStale: isStaleAt(h, nowMs),
      policyNumber: meta?.policyNumber?.toString().trim() || null,
    });
  }
  // Pure-protection sorts after positive-value policies
  rows.sort((a, b) => b.valueBase - a.valueBase);
  return rows;
}

/** Sum of all insurance surrender exposure in base currency. */
export function totalInsuranceExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isInsuranceHolding(h)) continue;
    const v = extractSurrenderValueBase(h);
    if (Number.isFinite(v) && v > 0) sum += v;
  }
  return sum;
}

/** Count of stale (last-valued > 7d old) policies. */
export function countStale(holdings: readonly Holding[], nowMs: number): number {
  let n = 0;
  for (const h of holdings) {
    if (!isInsuranceHolding(h)) continue;
    if (isStaleAt(h, nowMs)) n += 1;
  }
  return n;
}
