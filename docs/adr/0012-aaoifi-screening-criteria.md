# ADR 0012 — AAOIFI Sharia Screening Criteria

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** E (Mizan Badge Expansion) — PR-E4 (AAOIFI screening worker)

## Context

Per spec §8.2 (Mizan Badge modifier `'halal-screened'`) and `docs/plans/05-track-e.md`:

> Sharia compliance screening across all holdings (AAOIFI standards). Applied when `sharia_status = 'compliant'`. Hover surfaces the screening date and the criteria evaluated.

The screening worker (Track E PR-E4) needs explicit criteria thresholds to evaluate. The AAOIFI Sharia Standard No. 21 ("Financial Papers") is the most widely-adopted framework — the **DJIM (Dow Jones Islamic Market)** and **AAOIFI quantitative screens** are the basis for nearly every Islamic ETF (SPUS, ISWD, HLAL, etc.).

This ADR fixes the thresholds the screening worker enforces, the data sources it pulls from, and the verdict surface it produces. Annual review per working agreement §16 (ADRs reviewed annually for staleness).

## Decision

The screening worker applies **two distinct screens** to every equity holding:

### Screen 1 — Business Activity (Qualitative)

A company is **non-compliant** if its primary business is in any of these prohibited categories:

- Alcohol production, distribution, retail
- Conventional banking, insurance, brokerage (interest-bearing financial services)
- Gambling, casinos, lotteries
- Tobacco production, distribution
- Pornography, adult entertainment
- Pork-derived products
- Weapons, defence systems with offensive use
- Cannabis (under most AAOIFI scholar interpretations)
- Music, cinema, hotels with prohibited services (contested — surfaced as `'mixed'`)

**Source:** GICS (Global Industry Classification Standard) sector tagged from Twelve Data, cross-referenced against the prohibited list.

### Screen 2 — Financial Ratios (Quantitative, AAOIFI Standard No. 21)

A company is **non-compliant** if ANY of these ratios exceed their threshold:

| Ratio | Threshold | Numerator | Denominator |
|---|---|---|---|
| **Debt ratio** | **33%** | Interest-bearing debt | 12-month trailing market cap |
| **Receivables ratio** | **33%** | Accounts receivable + cash | 12-month trailing market cap |
| **Non-permissible income ratio** | **5%** | Interest income + other non-permissible revenue | Total revenue |

If the company passes Screen 1 (business activity) AND all three ratios are below thresholds, verdict = **`compliant`**.

If the company passes Screen 1 but has **mixed business lines** (e.g., a tech company with a small banking subsidiary), verdict = **`mixed`** — the agent then computes a **purification ratio** (percentage of non-permissible revenue / total revenue) which the user uses to purify dividends.

If the company fails Screen 1 OR any ratio exceeds threshold, verdict = **`non_compliant`**.

If the data needed to compute the ratios is not available (e.g., illiquid OTC instruments, fund-of-funds), verdict = **`unrated`** — Mizan Badge surfaces this distinctly from `compliant` so the user knows the screen was attempted but inconclusive.

### Data sources

- **Business activity (GICS)**: Twelve Data + Bloomberg-equivalent feed via Mizan Connect proxy
- **Financial ratios**: Pulled from SEC EDGAR (10-K, 10-Q) for US issuers; equivalent regulatory filings for other jurisdictions
- **12-month trailing market cap**: Computed from Twelve Data's daily close × shares-outstanding history
- **Manual override**: User can mark a holding's verdict via `holdings_metadata.sharia_status` write — the manual entry takes precedence and `last_screened_at` reflects user's last update

### Screening frequency

- **At connection**: every newly synced holding screened within 24h
- **Quarterly re-screen**: every active holding re-screened on a rolling 90-day cadence (so the worker's load is steady-state)
- **On dividend posting**: dividend-paying holdings re-screen ahead of dividend ex-date so purification ratio is current
- **On corporate action**: M&A, divestiture, sector change triggers an immediate re-screen

### Output surface

Per spec §8.2, the badge variants are:

- **`compliant`** — green crescent icon. Hover: screening date, ratios, business sector
- **`mixed`** — yellow crescent. Hover: purification ratio percentage (for Gold-tier users to apply to dividend purification calc)
- **`non_compliant`** — no green crescent. Hover: which screen failed (business activity vs which ratio)
- **`unrated`** — gray "screen attempted, no verdict" indicator. Hover: missing data sources

## Rationale

**Why AAOIFI Standard No. 21 vs alternatives (DJIM, MSCI Islamic, FTSE Sharia):**

- AAOIFI is the most widely-recognized in the Gulf + SE Asia (Mizan's primary user base per spec §1)
- The 33% / 33% / 5% thresholds are the closest to scholarly consensus across multiple boards
- DJIM uses 24-month average market cap which is more reactive; AAOIFI's 12-month is the conservative middle ground
- MSCI Islamic uses total assets (vs market cap) for the debt ratio — produces noisier results on growth stocks

**Why `'mixed'` as a distinct verdict (not just `'non_compliant'`):**

Per spec §5.1 (ETF look-through purification): for partially-permissible holdings (a tech ETF with 2% revenue from banking subsidiaries, etc.), the user can still hold the asset but **must purify** the non-permissible proportion of dividends. Treating `mixed` as `non_compliant` would over-flag and under-utilize the purification mechanism. Distinct verdict + purification ratio is the right shape.

**Why `'unrated'` as a distinct verdict (not just NULL `sharia_status`):**

A NULL `sharia_status` means "screen never attempted." An `'unrated'` verdict means "screen attempted, no verdict possible — data unavailable." The distinction matters for support tickets and for the user's trust signal — `'unrated'` tells the user Mizan tried and was honest about not knowing.

**Why annual scholarly review of this ADR:**

AAOIFI standards themselves evolve. The Cabinet of Scholars updates thresholds periodically. Working agreement §16 schedules annual ADR review; this one is on that calendar.

## Consequences

**Positive:**

- Single source of truth for what `'halal-screened'` means in Mizan
- Reproducible verdicts — the same holding screened twice produces the same result given the same input data
- The `unrated` distinction prevents user trust erosion from silent "not screened yet" states
- Purification ratio shape gives Gold-tier Zakat engine the input it needs for §5.1 ETF look-through

**Negative:**

- Data sourcing for non-US issuers is harder than for US (SEC EDGAR is uniquely good)
- The 33%/33%/5% thresholds will produce different verdicts than alternative frameworks (DJIM, MSCI Islamic) — users who hold ETFs screened by one framework may see different Mizan verdicts
- Manual scholarly board review is required when AAOIFI updates thresholds (or when adding a new prohibited business category)

**Follow-ups (tracked):**

- PR-E4: AAOIFI screening worker in `mizan-connect/src/sharia/`
- PR-E5: `find_sharia_status` agent tool consumes this worker's verdicts
- PR-B17: ETF look-through purification computes purification ratio for `'mixed'` holdings (consumes `purification_ratio` field — schema column to be added in `holdings_metadata` if not already; check ADR 0011)
- Annual ADR review on the 2027-Q2 calendar

## Alternatives Considered

**Alternative A: Use DJIM thresholds (24-month rolling avg market cap).** Rejected — DJIM is more reactive but reduces predictability of verdicts mid-quarter. The Gulf user base recognizes AAOIFI more readily.

**Alternative B: Use MSCI Islamic (total assets in debt ratio denominator).** Rejected — produces noisier results on growth-phase companies whose market caps are well above book value.

**Alternative C: Skip quantitative screening; rely only on business activity.** Rejected — most contemporary Sharia scholars require both screens. Permitting a company in a permissible sector but with 80% of revenue from interest income would violate the spirit of the standard.

**Alternative D: Allow user-configurable thresholds per scholarly preference.** Deferred — adds significant complexity for marginal benefit; defer until user research shows demand. The `user_memory` layer can store scholarly preference but the engine sticks to AAOIFI Standard No. 21 thresholds.

## References

- AAOIFI Sharia Standard No. 21 — Financial Papers (Shares and Bonds)
- Dow Jones Islamic Market index methodology
- MSCI Islamic Index methodology
- Spec §5.1 (Equities — Mutual Funds + ETFs), §8 (Mizan Badge), §11 (Zakat Engine)
- `docs/plans/05-track-e.md` PR-E4
- ADR 0011 (Holdings Metadata Design)
- Working agreement §16 (ADR annual review)
