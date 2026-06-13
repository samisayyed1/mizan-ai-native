/**
 * Per-panel "Add" handler.
 *
 * Each asset-class panel surfaces a "+ Add" button in its hero
 * (see `PanelHero` → `onAdd`). The button opens the inline
 * AddAssetDialog with a panel-specific prompt so the assistant lands
 * pointing at the right capability instead of the generic "what would
 * you like to track" intro.
 *
 * Centralising the prompts here keeps them consistent across panels
 * and makes it cheap to tweak copy or wire a different action later
 * (e.g. open a non-AI fast-add form for a specific class).
 */
import { useCallback } from "react";

import { useAddAsset } from "@/features/add-asset";

export type PanelKind =
  | "equities"
  | "sukuks"
  | "bank-cash"
  | "crypto"
  | "real-estate"
  | "forex"
  | "collectibles"
  | "commodities"
  | "private-equity"
  | "provident-funds"
  | "insurance"
  | "brokerage";

const PROMPTS: Record<PanelKind, string> = {
  equities:
    "Tell me about the equity position you want to track — symbol or company name, quantity, and which broker holds it.",
  sukuks:
    "Tell me about the bond or sukuk you want to track — issuer, face value, coupon rate, and maturity date.",
  "bank-cash":
    "Tell me about the cash account — bank name, currency, account type (checking / savings / fixed deposit), and current balance.",
  crypto:
    "Tell me about the crypto holding — symbol (e.g. BTC, ETH), chain or wallet, and quantity.",
  "real-estate":
    "Tell me about the property — its purpose (primary residence, rental, land held for trade, or commercial), location, and current market value.",
  forex:
    "Tell me about the FX position — currency pair (e.g. USDSGD), direction, and notional amount.",
  collectibles:
    "Tell me about the collectible you want to track — for example: a watch, art piece, wine collection, coin, rare stamp, or any other appreciating asset. Tell me its category and current value.",
  commodities:
    "Tell me about the commodity holding — metal (gold, silver, platinum, palladium, copper), quantity in troy ounces / grams, and storage location.",
  "private-equity":
    "Tell me about the private-equity position — fund name, commitment, current NAV, and vintage year.",
  "provident-funds":
    "Tell me about the provident fund — scheme (CPF, EPF, 401(k), Roth IRA, etc.), sub-account if applicable, and current balance.",
  insurance:
    "Tell me about the insurance policy — type (whole life, endowment, term, ILP), sum assured, current surrender value, and premium.",
  brokerage:
    "Tell me about the brokerage account you want to add — broker name, account type (individual, joint, IRA, etc.), and base currency.",
};

/**
 * Returns an `add` callback wired to the panel's prompt. Designed to be
 * passed straight to `<PanelHero onAdd={add} />`.
 */
export function usePanelAdd(kind: PanelKind): () => void {
  const addAsset = useAddAsset();
  return useCallback(() => {
    addAsset.open({
      source: "portfolio",
      prompt: PROMPTS[kind],
    });
  }, [addAsset, kind]);
}
