# Track B — Asset Class Expansion

**Status:** Pending. Depends on Track A (panel composition) + Track E (badge variants).
**Estimated sprints:** 4.
**Source:** `docs/plans/00-master-plan.md` → "Track B — Asset Class Expansion".

## Scope

**In:** 12 panels per spec §5, each following spec §6 universal pattern. New Mizan Connect providers: Setu (India), SGFinDex (SG), Tink (EU), Basiq (AU), Lean (UAE), CCXT (crypto), chain readers (Etherscan/BscScan/Solscan/Blockchair). AI estimation pipelines for real estate + collectibles.

**Out:** Dashboard composition (Track A); the badges themselves (Track E); AI agent tools (Track C); news (Track D); Zakat behavior of each class (Track F).

## PRs

| # | Status | Title |
|---|---|---|
| B0 | ⏸️ Pending | Universal asset class panel skeleton — spec §6 (header / chart / list / insights / actions / history) |
| B1 | ✅ Done (2026-06-04 as PR-B2 per Goal v3 §V Phase 5 step B2) | Equities panel. New `apps/frontend/src/pages/panels/equities/` with `rollup.ts` pure-math helpers (`isEquityHolding`, `rollupBySubclass`, `rollupByRegion`, `totalEquityExposure`) and `equities-panel.tsx` page composition. Donut by sub-class (Stocks / ETFs / Mutual Funds / Options) + bar by region (top-level geographic exposure, weighted-split per holding with re-normalisation when weights >100). Route `/panels/equities` registered; the dashboard's twelve-panel grid now points `equities` → `/panels/equities`. **23 unit tests** including the AAPL/SPUS/VFINX/AAPL_C fixture for the sub-class rollup, the 50/50 region split test, the weight-re-normalisation test (60+60→100/100), the Unspecified fallback test, and the cross-holding aggregation/sort test. Frontend vitest 933/933 (was 910; +23); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B2 | ⏸️ Pending | Brokerage Accounts panel — extends SnapTrade UI |
| B3 | ✅ Done (2026-06-04 as PR-B3 per Goal v3 §V Phase 5 step B3 — frontend slice; Setu cloud-side integration tracks separately) | Bank & Cash panel. New `apps/frontend/src/pages/panels/bank-cash/` with `rollup.ts` (`isCashHolding`, `rollupByCurrency`, `rollupByCountry`, `totalCashExposure`) + `bank-cash-panel.tsx` page (header + donut with Country/Currency toggle + accounts list). Country rollup is weighted-split with re-normalisation (60+60→100/100). Currency uses `instrument.currency` ↑ uppercase, fallback to `localCurrency`, ultimate fallback to "UNKNOWN". Predicate `holdingType === 'cash'` matches `HoldingType.CASH` constant; case-insensitive defensive match. Route `/panels/bank-cash` + descriptor wired. **14 unit tests** including the §23-flavored DBS-SG / HSBC-UAE / OCBC-SG fixture for country rollup. Frontend vitest 947/947 (was 933; +14); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B4 | ⏸️ Pending | Bank/Cash: SGFinDex (Singpass OAuth flow with required redirect_uri) |
| B5 | ⏸️ Pending | Bank/Cash: Tink |
| B6 | ⏸️ Pending | Bank/Cash: Basiq |
| B7 | ⏸️ Pending | Bank/Cash: Lean |
| B8 | ✅ Done (2026-06-04 as PR-B1 per Goal v3 §V Phase 5) | **Bonds & Sukuks panel — §23 anchor surface.** New `mizan-4/apps/frontend/src/pages/panels/sukuks/` with `rollup.ts` pure-math helpers (`isBondHolding`, `rollupByIssuer`, `rollupByMaturityYear`, `extractMaturityYear`, `totalBondExposure`) and `sukuks-panel.tsx` page composition. Bar chart toggle between "by issuer" (default — Emaar/DAR/Sobha desc-by-exposure) + "by maturity year" (asc-by-year). Holdings list below the chart, tap-row navigates to asset detail. Route `/panels/sukuks` registered; the dashboard's twelve-panel grid (PR-A4 #94) now points `bonds-sukuks` → `/panels/sukuks`. The `extractMaturityYear` helper reads `Asset.bond_spec().maturityDate` JSON via an `unknown` cast (frontend Instrument type doesn't expose `metadata` yet — PR-B1.a will thread it through). **24 unit tests** including the §23 Emaar/DAR/Sobha fixture pinning `[Emaar 300K, DAR 200K, Sobha 150K]` issuer-rollup + the `[2026 188K, 2027 350K]` maturity-rollup. Frontend tests 910/910 (was 886; +24); lint 0 errors / 399 warnings unchanged from main; tsc clean. |
| B9 | ⏸️ Pending | Provident Funds panel (CPF/EPF/401k/NPS/Super) + nested holdings UX |
| B10 | ⏸️ Pending | Insurance panel — investment-linked + pure protection split |
| B11 | ⏸️ Pending | Private Equity panel — vintage bar + J-curve projection |
| B12 | ✅ Done (2026-06-04 as PR-B11 per Goal v3 §V Phase 5 — frontend panel slice; AI estimation pipeline + ai-estimated badge wiring track separately as PR-B11.b) | Real Estate panel. New `apps/frontend/src/pages/panels/real-estate/` with `rollup.ts` pure-math helpers (`isRealEstateHolding` predicate on `assetKind==='PROPERTY'`, `rollupByIntent` mapping `metadata.property.propertyType` to Residence/Rental/Land/Commercial/Other, `rollupByProperty` for per-property bars, `totalRealEstateExposure`) + `real-estate-panel.tsx` page (header + intent donut + per-property bar + properties list, tap-row → holding detail). Route `/panels/real-estate` + descriptor wired. **16 unit tests** including the §23 anchor fixture (Bukit Batok residence + 3 Hyderabad rentals + 1 Hyderabad held-for-sale = $1.65M / 5 properties), case-insensitive propertyType, missing-metadata Other fallback, non-property reject, same-symbol distinct-property pin, holdingId nav preservation. Frontend vitest 963/963 (was 947; +16); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B13 | ✅ Done (2026-06-04 as PR-B9 per Goal v3 §V Phase 5 — frontend panel slice; CCXT + chain-reader cloud integrations track separately as PR-B9.b) | Crypto panel. New `apps/frontend/src/pages/panels/crypto/` with `rollup.ts` (`isCryptoHolding`, `chainForSymbol`, `rollupByChain`, `totalCryptoExposure`) + `crypto-panel.tsx` page. Predicate matches PR-C5.d.3 desktop side (`CRYPTOCURRENCY`/`CRYPTO`). Chain classification covers Bitcoin (BTC/WBTC/TBTC/BTCB), Ethereum (ETH/WETH/stETH/wstETH/rETH/cbETH/wBETH), Solana (SOL/wSOL/JitoSOL/mSOL/bSOL), BNB Chain, Polygon, Stablecoins (USDC/USDT/DAI/PYUSD/TUSD/BUSD/FRAX/USDP/GUSD/USDD — always win over their underlying chain), Other fallback. Case-insensitive + whitespace-trimmed symbol matching. Route `/panels/crypto` + descriptor wired. **37 unit tests** including 20 chain-symbol cases (BTC, ETH, stETH, USDC, DOGE, etc.), aggregation across wrapped variants (BTC+WBTC → Bitcoin, ETH+stETH → Ethereum), stablecoin-wins-over-chain pin, non-crypto reject, zero/neg skip. Frontend vitest 1000/1000 (was 963; +37); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B14 | ✅ Done (2026-06-04 as PR-B10 per Goal v3 §V Phase 5 — frontend panel slice; MetalpriceAPI feed cloud-side tracks separately as PR-B10.b) | Commodities panel. New `apps/frontend/src/pages/panels/commodities/` with `rollup.ts` (`isCommodityHolding`, `metalForSymbol`, `rollupByMetal`, `totalCommoditiesExposure`) + `commodities-panel.tsx` page. Predicate accepts EITHER `assetKind === 'PRECIOUS_METAL'` OR `assetType.key` in {METAL/PRECIOUS_METAL/COMMODITY}. Metal classifier covers Gold (XAU/GOLD/GC/GLD/IAU/PHYS/SGOL), Silver (XAG/SILVER/SI/SLV/SIVR/PSLV), Platinum (XPT/PL/PPLT), Palladium (XPD/PA/PALL), Other fallback. Case-insensitive + whitespace-trimmed. Route `/panels/commodities` + descriptor wired. **36 unit tests** including 17 metal-symbol cases (COMEX + ETF aliases), either-signal sufficiency pin, ETF aggregation (XAU+IAU+GLD → Gold $100K), unknown→Other, zero/neg skip. Frontend vitest 1036/1036 (was 1000; +36); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B15 | ⏸️ Pending | Collectibles panel + AI estimation pipeline |
| B16 | ✅ Done (2026-06-04 as PR-B16 per Goal v3 §V Phase 5 — frontend panel slice; per-pair histogram + price-history overlay tracks separately as PR-B16.a alongside the FX history hydration from PR-C5.d.4 #114) | Forex panel. New `apps/frontend/src/pages/panels/forex/` with `rollup.ts` (`isFxHolding`, `extractBaseCurrency`, `rollupByPair`, `rollupByLongLeg`, `totalFxExposure`) + `forex-panel.tsx` page. Predicate accepts EITHER `assetKind === 'FX'` OR `assetType.key` in {FX, CURRENCY, FOREX}. `extractBaseCurrency` parses USD/INR (slash), USD-INR (dash), USDINR (concatenated), USDINR=X (Yahoo) — all enforce 3-char ISO 4217 strictness so junk like "MYSTERY-FX" falls to Unknown. Per-pair bar (positions stay distinct, NOT aggregated by pair) + long-leg donut (USD/INR + USD/SGD aggregate as USD long). Route `/panels/forex` + descriptor wired (iconKey `RefreshCw`). **31 unit tests** including either-signal pin, 10 base-currency-extraction cases, USD-long-across-pairs aggregation, Yahoo `=X` format support, strict ISO-4217 rejection. Frontend vitest 1067/1067 (was 1036; +31); lint 0 errors / 399 warnings unchanged; tsc clean. |
| B17 | ⏸️ Pending | ETF look-through purification worker (Sharia-compliant ETFs like SPUS) |

## ADRs (planned)

- 0021 — Setu AA integration
- 0022 — SGFinDex Singpass required redirect_uri
- 0023 — Tink PSD2
- 0024 — Basiq CDR
- 0025 — Lean UAE
- 0026 — CCXT crypto exchanges read-only scope enforcement
- 0027 — Chain reader public-address only (no private keys, no seed phrases — working agreement §8 bright line)
- 0028 — AI estimation pipeline (real estate + collectibles)
- 0029 — ETF look-through purification

## Security checklist (per provider)

- [ ] Tokens encrypted at rest with provider-specific encryption key, AES-GCM-256
- [ ] Webhook signature verification mandatory
- [ ] Idempotency by provider event ID, lookup-or-insert
- [ ] `redirect_uri` Option-typed (SGFinDex requires it; others optional)
- [ ] CCXT scope enforcement: `withdraw` / `trade` rejected at validation
- [ ] Chain reader: public address only — never private keys / seed phrases
- [ ] AI estimation never auto-writes a holding's value — surfaces a range with confidence; user confirms

## Definition of Done

- All 12 panels live, each follows the universal pattern
- Per-class donut/bar visualizations meet design bar (animated, hover-to-expand, center label)
- All 6 new sync providers + 4 chain readers live with security checklist green
- Reference user (Singapore Sharia-aware) can connect Setu bank + SnapTrade broker + CCXT exchange + chain reader, and see every holding in the right panel with the right badge
