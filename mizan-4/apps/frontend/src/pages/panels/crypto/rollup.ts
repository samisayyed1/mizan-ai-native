/**
 * Crypto panel rollup — Track B PR-B9 / Goal v3 §V Phase 5.
 *
 * Per ADR 0021 §"Crypto":
 *   - Donut by chain (Bitcoin, Ethereum, Solana, Stablecoins, Other)
 *   - Sharia-toggleable per ADR 0036 (planned with PR-F9)
 *
 * Predicate matches PR-C5.d.3 desktop side: `instrument.classifications
 * .assetType.key` in {CRYPTOCURRENCY, CRYPTO}. Predicate must agree
 * across surfaces.
 *
 * Chain classification uses a coin-symbol → chain map. Stablecoins
 * (USDC / USDT / DAI / etc.) get their own slice so the user can see
 * the "dry powder" share separate from volatile chain exposure.
 */
import type { Holding } from "@/lib/types";

/** Predicate matching crypto-classified holdings. */
export function isCryptoHolding(h: Holding): boolean {
  const key = h.instrument?.classifications?.assetType?.key?.toUpperCase() ?? "";
  return key === "CRYPTOCURRENCY" || key === "CRYPTO";
}

/** Chain bucket the donut groups holdings into. */
export type CryptoChain =
  | "Bitcoin"
  | "Ethereum"
  | "Solana"
  | "BNB Chain"
  | "Polygon"
  | "Stablecoins"
  | "Other";

/**
 * Map an instrument symbol to its primary chain.
 *
 * This is a deliberately small static map covering the §23-relevant
 * majors + the stablecoins users commonly hold. Anything else falls
 * to `'Other'` — the user can see the gap, and we don't pretend to
 * classify obscure tokens we can't verify on-chain.
 *
 * Symbol matching is case-insensitive + handles common wrapped /
 * staked / liquid-staking variants (WBTC → Bitcoin, stETH → Ethereum,
 * etc.). Stablecoins always win over their underlying chain so a
 * USDC-on-Solana holding renders as Stablecoins, not Solana.
 */
export function chainForSymbol(symbol: string): CryptoChain {
  const s = symbol.trim().toUpperCase();
  // Stablecoins first — they win over chain placement.
  if (
    s === "USDC" ||
    s === "USDT" ||
    s === "DAI" ||
    s === "PYUSD" ||
    s === "TUSD" ||
    s === "BUSD" ||
    s === "FRAX" ||
    s === "USDP" ||
    s === "GUSD" ||
    s === "USDD"
  ) {
    return "Stablecoins";
  }
  // Bitcoin family
  if (s === "BTC" || s === "WBTC" || s === "TBTC" || s === "BTCB") {
    return "Bitcoin";
  }
  // Ethereum family + LSTs
  if (
    s === "ETH" ||
    s === "WETH" ||
    s === "STETH" ||
    s === "WSTETH" ||
    s === "RETH" ||
    s === "CBETH" ||
    s === "WBETH"
  ) {
    return "Ethereum";
  }
  // Solana
  if (s === "SOL" || s === "WSOL" || s === "JITOSOL" || s === "MSOL" || s === "BSOL") {
    return "Solana";
  }
  // BNB Chain
  if (s === "BNB" || s === "WBNB") {
    return "BNB Chain";
  }
  // Polygon
  if (s === "MATIC" || s === "WMATIC" || s === "POL") {
    return "Polygon";
  }
  return "Other";
}

/** Per-chain donut row. */
export interface ChainRow {
  chain: CryptoChain;
  exposureBase: number;
}

/**
 * Roll crypto holdings into per-chain exposure rows, descending by
 * exposure. Stablecoins always emit as their own slice even when a
 * holding's underlying chain would otherwise classify it differently.
 */
export function rollupByChain(holdings: readonly Holding[]): ChainRow[] {
  const map = new Map<CryptoChain, number>();
  for (const h of holdings) {
    if (!isCryptoHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (!Number.isFinite(value) || value <= 0) continue;
    const symbol = h.instrument?.symbol ?? "";
    const chain = chainForSymbol(symbol);
    map.set(chain, (map.get(chain) ?? 0) + value);
  }
  const rows = Array.from(map.entries()).map(([chain, exposureBase]) => ({
    chain,
    exposureBase,
  }));
  rows.sort((a, b) => b.exposureBase - a.exposureBase);
  return rows;
}

/** Sum of all crypto exposure in base currency. */
export function totalCryptoExposure(holdings: readonly Holding[]): number {
  let sum = 0;
  for (const h of holdings) {
    if (!isCryptoHolding(h)) continue;
    const value = h.marketValue?.base ?? 0;
    if (Number.isFinite(value) && value > 0) sum += value;
  }
  return sum;
}
