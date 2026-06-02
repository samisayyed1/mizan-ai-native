# ADR 0021 — Asset Class Expansion Plan

| Status | ✅ Accepted (autonomous-execution authority — Track B foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/02-track-b.md](../plans/02-track-b.md), [ADR 0018 — Dashboard IA](0018-dashboard-information-architecture.md), [ADR 0019 — Charting Vocabulary](0019-charting-vocabulary.md), [ADR 0012 — AAOIFI Screening Criteria](0012-aaoifi-screening-criteria.md) |

## Context

Mizan today supports a core set of asset classes via Wealthfolio inheritance: Equities (US/EU brokerage via SnapTrade) and Bank/Cash (manual entry + Plaid integration scaffold). The Mizan Evolution Spec §5 prescribes expanding to **12 asset class panels** in fixed dashboard order, each with its own data shape, valuation logic, sync provider(s), and Mizan Badge stack.

This ADR locks the per-asset-class shape so Track B's 30+ PRs (per `docs/plans/02-track-b.md`) land against a stable contract — including the provider-integration choices (Setu / SGFinDex / Tink / Basiq / Lean / CCXT / chain readers), the AI-estimation pipelines for real estate + collectibles, and the ETF look-through purification worker for Sharia ETFs.

## Decision

### The 12 panels (fixed dashboard order per ADR 0018 + Spec §3(e))

| # | Panel | Sub-class taxonomy | Sync providers | AI-estimation? | Sharia screening? |
|---|---|---|---|---|---|
| 1 | **Equities** | by GICS sector + geography | SnapTrade (existing) | ❌ — quote-driven | ✅ — AAOIFI per ADR 0012 |
| 2 | **Brokerage Accounts** | per-broker | SnapTrade + Plaid (existing) | ❌ | ✅ — surface holding-level badges |
| 3 | **Bank / Cash** | by region | Plaid (US) + Setu (IN) + SGFinDex (SG) + Tink (EU) + Basiq (AU) + Lean (UAE) | ❌ | ✅ — Sharia-compliant savings = no interest |
| 4 | **Bonds & Sukuks** | by issuer + maturity | Bondevalue (read-only data) + manual | ❌ | ✅ — sukuk vs conventional flagged |
| 5 | **Provident Funds** | CPF / EPF / 401k / NPS / Super | per-jurisdiction read-only providers | ❌ | ✅ — flag fund composition |
| 6 | **Insurance** | investment-linked (ULIP) vs pure protection | manual + Setu IDV | ❌ | 🟡 — surrender-value Zakatable flag only |
| 7 | **Private Equity** | by vintage + GP | manual (quarterly NAV imports) | 🟡 — J-curve projection only | ✅ — fund-level halal verdict |
| 8 | **Real Estate** | primary residence vs rental vs held-for-sale | manual + AI-estimation pipeline | ✅ — PropertyGuru / Magicbricks / Zillow / DLD comparable lookups | n/a |
| 9 | **Crypto** | by chain | CCXT (exchange) + chain readers (Etherscan / BscScan / Solscan / Blockchair) | ❌ — quote-driven | ✅ — toggleable per ADR 0036 (planned with PR-F9) |
| 10 | **Commodities** | gold / silver / palladium / platinum / etc. | MetalpriceAPI feed | ❌ | ✅ — physical vs paper gold flagged |
| 11 | **Collectibles** | by category (watches / sneakers / etc.) | manual + AI-estimation pipeline | ✅ — Chrono24 / WatchCharts / Hagerty / StockX comparables | n/a |
| 12 | **Forex** | per pair | manual; live FX from existing service | ❌ | n/a |

### Per-panel data shape (universal pattern from Spec §6)

Each panel renders:
1. **Header strip** — total value + 24h delta sparkline + badge stack
2. **Chart** — donut (allocation by sub-class) per ADR 0019
3. **Holdings list** — sortable, filterable, with per-row badges
4. **Insights card** — top 1-2 panel-specific insights from `crates/insights`
5. **Actions** — Add holding / Sync / Generate report / Run Zakat
6. **History sub-section** — sparkline + tap-to-expand timeline

### Provider integration sequencing

| PR | Provider | Region | Pre-req | Notes |
|---|---|---|---|---|
| PR-B3 | Setu | IN | ✅ existing scaffold | Account Aggregator framework |
| PR-B4 | SGFinDex | SG | Singpass redirect_uri | OAuth flow specific to SG |
| PR-B5 | Tink | EU | PSD2 license check | OAuth + AISP scope only |
| PR-B6 | Basiq | AU | CDR compliance review | OAuth + read-only |
| PR-B7 | Lean | UAE | ✅ existing partnership | Same OAuth shape as Tink |
| PR-B13 | CCXT | Crypto exchanges | API key / OAuth per exchange | **scope enforcement:** withdraw + trade scopes REJECTED at validation per ADR 0026 (Spec §5.9) |
| PR-B13 | Chain readers | Crypto wallets | Public address only — NO private keys, NO seed phrases (CLAUDE.md §8 bright line) | Etherscan, BscScan, Solscan, Blockchair APIs |

### AI estimation pipelines

PR-B12 (Real Estate) + PR-B15 (Collectibles) ship dedicated AI estimation pipelines:

**Real Estate** (`crates/ai/src/estimation/real_estate.rs`):
- PropertyGuru (SG) / Magicbricks (IN) / Zillow (US) / DLD (UAE)
- Inputs: street address + sqft + bedroom_count + property_type
- Outputs: { estimate, low, high, confidence ∈ [0,1] }
- **Never auto-writes** — always surfaces as `'ai-estimated'` badge with user confirmation per Track E ADR 0023

**Collectibles** (`crates/ai/src/estimation/collectibles.rs`):
- Chrono24 (watches) / WatchCharts (watches) / Hagerty (cars) / StockX (sneakers)
- Inputs: brand + model + condition + provenance hints
- Outputs: same shape as real-estate
- Same `'ai-estimated'` badge surface

### ETF look-through purification worker

PR-B17 ships the ETF look-through worker for Sharia-compliant ETFs:
- For each held Sharia ETF, fetch the latest disclosed purification amount per share (quarterly disclosure)
- Apply to the user's dividend events: a portion of each dividend is non-halal and must be donated to charity (not Zakat)
- Surfaces as a Mizan Badge `'purification-pending'` modifier on the dividend row
- Lives in `mizan-connect/src/sharia/etf_lookthrough.rs` so the data is cloud-cached for all users + the desktop reads via Mizan Connect API

## Rationale

**Why 12 panels exactly (not 8, not 20)?**
The 12-panel set is the spec-locked surface; user research (Spec §3) showed that 12 is at the cognitive-load ceiling for a single dashboard scroll. Adding a 13th class would force the heatmap to drop a tile or the panels to compress further. Tracked as PR-A14.1 if a new asset class emerges.

**Why fix the order?**
Per ADR 0018 §"Why fixed-order asset class panels" — predictability + cache key simplicity. The order matches conventional wealth-management report ordering.

**Why a separate ETF look-through worker (not inline in PR-B17)?**
The purification calculation needs cloud-cached fund data + cross-user amortization (the disclosure fetch happens once for all users, not once per user). Putting it in `mizan-connect/src/sharia/etf_lookthrough.rs` lets the cost be amortized; lets the data refresh on the same schedule the AAOIFI screening worker uses; and lets the desktop read it via the existing Mizan Connect API surface.

**Why never auto-write AI estimates?**
Working-agreement §0 rule 1 + §13 past-bug list: AI-derived numbers must surface as estimates with explicit confirmation before they enter the user's holdings or Zakat base. The `'ai-estimated'` badge is the visual contract; user confirmation is the write trigger.

**Why CCXT scope rejection at the validation layer (not at exchange-API-call layer)?**
Defence-in-depth: rejecting at validation means a misconfigured API key with withdraw/trade scope never reaches the exchange — no risk of the credential being stored in keychain with overly-broad scope. Documented in ADR 0026 (Spec §5.9; lands with PR-B13).

## Consequences

**Positive:**
- Each panel has a clear scope + dedicated provider per the table above. PR-B* lands mechanically against this matrix.
- AI estimation lives outside Truth Ledger paths (never auto-writes) — keeps the ledger's invariants intact.
- Sharia screening is composable: AAOIFI per holding (ADR 0012) + ETF look-through (PR-B17) + per-class halal/non-halal flags surface independently in the badge stack.

**Negative / accepted:**
- 30+ PRs is a long Track B execution arc. Mitigation: per-provider PRs are independently mergeable; the panel skeleton (PR-B0) unblocks the universal pattern even before all providers ship.
- AI estimation accuracy varies by region (PropertyGuru data is denser than Zillow for SG; the reverse for US). Mitigation: per-region confidence scoring + the AI-estimated badge with explicit range surfaces uncertainty to the user.

**Risks:**
- Per-provider OAuth flows mean per-provider security review burden. Each provider PR (B3..B7, B13) ships with the standard CLAUDE.md §6 security checklist + the per-provider ADR (0021-0029).
- Chain-reader integration means trusting the read-only blockchain explorer endpoints. Mitigation: rate-limit + circuit-breaker per provider; bright line that NO private keys / NO seed phrases ever cross the wire (CLAUDE.md §8).

## Alternatives considered

- **Ship fewer panels (e.g. start with 6, expand)** — rejected because the dashboard's heatmap is fixed-order, so a partial set produces a half-baked surface visible to users. Better to ship the skeleton + a placeholder marker for unimplemented panels.
- **One provider per panel (drop the multi-region matrix)** — rejected because users self-identify by region; Singapore users need SGFinDex, India users need Setu, etc. The provider matrix matches the spec's user-region targeting.
- **Roll the ETF look-through into the dispatcher** — rejected; the disclosure data is amortizable across users + needs cloud caching. The worker home is `mizan-connect`, not desktop.

## Implementation map

| PR | What lands |
|---|---|
| **PR-B0 (universal panel skeleton)** | Component + Storybook scaffold + 6 standard slots |
| PR-B1 | Equities panel (sub-class donut + geography bar) |
| PR-B2 | Brokerage Accounts panel (extends SnapTrade UI) |
| PR-B3 | Setu (IN) provider in Mizan Connect + UI |
| PR-B4 | SGFinDex (SG) provider |
| PR-B5 | Tink (EU) provider |
| PR-B6 | Basiq (AU) provider |
| PR-B7 | Lean (UAE) provider |
| PR-B8 | Bonds & Sukuks panel |
| PR-B9 | Provident Funds panel (CPF/EPF/401k/NPS/Super) |
| PR-B10 | Insurance panel (ULIP + pure protection) |
| PR-B11 | Private Equity panel (vintage bar + J-curve) |
| PR-B12 | Real Estate panel + AI estimation pipeline + 'ai-estimated' badge wiring |
| PR-B13 | Crypto panel (CCXT + chain reader read-only) |
| PR-B14 | Commodities panel (MetalpriceAPI feed) |
| PR-B15 | Collectibles panel + AI estimation pipeline |
| PR-B16 | Forex panel + histogram per-pair |
| PR-B17 | ETF look-through purification worker (Mizan Connect) |

Per-provider ADRs (0021-0029) land alongside their respective PRs (B3-B7, B13). Each PR ≤ 500 lines per working-agreement §A21.
