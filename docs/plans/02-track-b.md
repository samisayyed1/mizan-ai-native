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
| B1 | ⏸️ Pending | Equities panel — extends existing; adds sub-class donut + geographic bar |
| B2 | ⏸️ Pending | Brokerage Accounts panel — extends SnapTrade UI |
| B3 | ⏸️ Pending | Bank/Cash: Setu provider in Mizan Connect + UI |
| B4 | ⏸️ Pending | Bank/Cash: SGFinDex (Singpass OAuth flow with required redirect_uri) |
| B5 | ⏸️ Pending | Bank/Cash: Tink |
| B6 | ⏸️ Pending | Bank/Cash: Basiq |
| B7 | ⏸️ Pending | Bank/Cash: Lean |
| B8 | ✅ Done (2026-06-04 as PR-B1 per Goal v3 §V Phase 5) | **Bonds & Sukuks panel — §23 anchor surface.** New `mizan-4/apps/frontend/src/pages/panels/sukuks/` with `rollup.ts` pure-math helpers (`isBondHolding`, `rollupByIssuer`, `rollupByMaturityYear`, `extractMaturityYear`, `totalBondExposure`) and `sukuks-panel.tsx` page composition. Bar chart toggle between "by issuer" (default — Emaar/DAR/Sobha desc-by-exposure) + "by maturity year" (asc-by-year). Holdings list below the chart, tap-row navigates to asset detail. Route `/panels/sukuks` registered; the dashboard's twelve-panel grid (PR-A4 #94) now points `bonds-sukuks` → `/panels/sukuks`. The `extractMaturityYear` helper reads `Asset.bond_spec().maturityDate` JSON via an `unknown` cast (frontend Instrument type doesn't expose `metadata` yet — PR-B1.a will thread it through). **24 unit tests** including the §23 Emaar/DAR/Sobha fixture pinning `[Emaar 300K, DAR 200K, Sobha 150K]` issuer-rollup + the `[2026 188K, 2027 350K]` maturity-rollup. Frontend tests 910/910 (was 886; +24); lint 0 errors / 399 warnings unchanged from main; tsc clean. |
| B9 | ⏸️ Pending | Provident Funds panel (CPF/EPF/401k/NPS/Super) + nested holdings UX |
| B10 | ⏸️ Pending | Insurance panel — investment-linked + pure protection split |
| B11 | ⏸️ Pending | Private Equity panel — vintage bar + J-curve projection |
| B12 | ⏸️ Pending | Real Estate panel + AI estimation pipeline + `'ai-estimated'` badge wiring |
| B13 | ⏸️ Pending | Crypto panel — CCXT + chain reader (read-only scope enforced at validation) |
| B14 | ⏸️ Pending | Commodities panel — donut with MetalpriceAPI feed |
| B15 | ⏸️ Pending | Collectibles panel + AI estimation pipeline |
| B16 | ⏸️ Pending | Forex panel + histogram per-pair |
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
