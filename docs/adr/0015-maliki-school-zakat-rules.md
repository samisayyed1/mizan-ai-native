# ADR 0015 — Maliki school Zakat rules

| Status | 🟡 **PENDING SCHOLARLY APPROVAL — Uncle Ferox** |
|---|---|
| Date drafted | 2026-06-03 |
| Drafted by | ai (auditor; under autonomous-execution authorization) |
| Reviewers required | Uncle Ferox (fiqh al-muamalat — registered Track F reviewer per [docs/REVIEWERS.md](../REVIEWERS.md)) |
| Final-sign-off requirement | Per [feedback-autonomous-execution-mode](../../../.claude/projects/-Users-samisayyed-mizan-ai-native/memory/) — autonomous-execution authority **does not** apply to Track F fiqh substance. Uncle Ferox's approval is the gate. |
| Replaces | n/a |
| Related | [0001-adopt-working-agreement-v1](0001-adopt-working-agreement-v1.md), [0012-aaoifi-screening-criteria](0012-aaoifi-screening-criteria.md), `crates/zakat/src/schools/` (code lands ONLY after this ADR is approved) |

---

## ⚠️ This is a DRAFT for scholarly review

This document captures the AI's best-effort interpretation of Maliki school rules for Zakat across the modern asset classes Mizan supports. Every numeric threshold, every Zakatability classification, every exclusion is **pending Uncle Ferox's review**. Items marked **❓ SCHOLAR-VERIFY** below are the highest-priority verification points where the AI is least confident.

Do NOT implement these rules in code (`crates/zakat/src/schools/maliki.rs`) until this ADR carries Uncle Ferox's sign-off.

---

## Context

Mizan's Zakat engine ships with Hanafi + Shafi'i school rules today (`crates/zakat/src/`). Track F extends coverage to Maliki + Hanbali (the two remaining of the four Sunni schools). Per the autonomous-execution directive of 2026-06-03, Uncle Ferox is the registered scholarly authority for these school rules — code lands only after his approval.

This ADR establishes the Maliki rule set. Hanbali rules are in [ADR 0016](0016-hanbali-school-zakat-rules.md).

## Maliki-school rules

### 1. Nisab thresholds

The Maliki school holds the same gold/silver Nisab thresholds as the other Sunni schools:
- **Gold nisab:** 85 grams of 24-carat gold (20 mithqal)
- **Silver nisab:** 595 grams of silver (200 dirhams)
- Lower of the two market values is the operative Nisab for the Zakatable base.

**❓ SCHOLAR-VERIFY:** Some Maliki sources prefer the GOLD nisab specifically (rather than lower-of-both) when computing a year's wealth, on the basis that gold is the more stable historical reference. The AAOIFI standard uses lower-of-both. Mizan's current Hanafi/Shafi'i implementation uses lower-of-both. **Uncle Ferox: confirm preference for Maliki users.**

### 2. Zakat rate

**2.5% (1/40th)** of the Zakatable base, identical to the other Sunni schools.

### 3. Hawl (lunar year) — fixed per cohort

Maliki treats Hawl per cohort of wealth, identical to Hanafi/Shafi'i:
- A new cohort starts when wealth first crosses Nisab
- Hawl completes 354.36 lunar days later (lunar year)
- Subsequent wealth added inherits the parent cohort's Hawl date IF it's of the same category (e.g. additional cash mid-year stays on the existing Hawl)
- New asset categories form new cohorts

**❓ SCHOLAR-VERIFY:** Maliki is known for taking a somewhat stricter view on continuous-holding: some Maliki opinions disqualify a cohort if wealth drops below Nisab mid-year, requiring a fresh Hawl. Mizan's existing implementation does NOT disqualify in this case. **Uncle Ferox: which Maliki position should we encode by default?**

### 4. Asset-class-specific rules

#### 4.1 Cash, bank balances, savings accounts

Zakatable at full value. Same as other schools.

#### 4.2 Equities — long-term investment intent

Zakatable on the proportional Zakatable assets of the underlying company:
- For a Sharia-compliant company: proportional cash + receivables of the company per share count.
- For a Sharia-non-compliant company: per ADR 0012 (AAOIFI screening), Maliki does not endorse holding non-compliant equity; if the user holds anyway, full market value is Zakatable + the purification ratio applies.

**❓ SCHOLAR-VERIFY:** Some Maliki contemporary sources (e.g. Bin Bayyah) suggest full market value as Zakatable for any equity held with investment intent, simplifying the rule at the cost of higher Zakat. Others require the AAOIFI proportional approach. **Uncle Ferox: which to default to?**

#### 4.3 Equities — trading intent

Full market value Zakatable as trading inventory (`'urud at-tijarah`), identical to other schools.

#### 4.4 Brokerage accounts / mutual funds / ETFs

Treated as portfolios of underlying assets — apply the equities rule to each holding within. Track E's `'halal-screened'` badge informs the screening; for Sharia ETFs with quarterly purification disclosures, the disclosed purification amount supersedes the AAOIFI proportional approximation.

#### 4.5 Bonds & Sukuks

- **Conventional bonds (interest-bearing):** Maliki holds the principal as Zakatable (lent wealth is still wealth); the interest itself is non-permissible income and is excluded from Zakat — it must be donated to charity (not as Zakat). Mizan's existing rules already encode this.
- **Sukuk (Sharia-compliant):** Zakatable on the full market value + accrued profit; same treatment as a permissible bond from a fiqh perspective.

**❓ SCHOLAR-VERIFY:** Conventional-bond principal — some Maliki opinions exclude even the principal as it derives from a non-permissible contract; the user should be advised to divest. **Uncle Ferox: encode "Zakatable principal + advisory note to divest" OR "Excluded entirely with strong advisory"?**

#### 4.6 Real estate — primary residence

**Not Zakatable.** Identical to other schools. The Bukit Batok scenario in the spec §23 worked example excludes the primary residence with reasoning.

#### 4.7 Real estate — held for rental

**Maliki position:** the property itself is NOT Zakatable; the RENTAL INCOME received is Zakatable on receipt (counted at the end of the cohort's Hawl).

This differs from some Shafi'i opinions that Zakat the property's full market value. **❓ SCHOLAR-VERIFY: confirm Maliki position on rental property — Mizan's existing implementation Zakat's rental income only.**

#### 4.8 Real estate — held for sale (trade inventory)

Zakatable on full market value at Hawl. Identical to other schools. The Hyderabad held-for-sale unit in the spec §23 scenario is Zakatable.

#### 4.9 Private equity — long-term holding

Proportional Zakatable share of the PE fund's Zakatable assets (cash, receivables, inventory). Mizan reads the most recent quarterly NAV + asks the user for the % of the NAV that's Zakatable (or imports the GP-provided number).

**❓ SCHOLAR-VERIFY:** Maliki position on PE structures — does the underlying-asset transparency rule apply at full strength, or does Maliki accept a simpler "Zakatable at the proportional NAV" approach? **Uncle Ferox: confirm.**

#### 4.10 Pension / provident funds / 401(k) / EPF / CPF / NPS / Superannuation — locked

**Maliki position (proposed):** the LOCKED portion is NOT Zakatable until accessible (the user has no ownership in the same sense as freely-disposable wealth). The UNLOCKED portion (vested + accessible) is Zakatable annually.

This matches the working-agreement spec §23 "locked retirement two-views" — the user picks the view (or accepts the default) and Mizan applies it consistently every Hawl.

**❓ SCHOLAR-VERIFY:** Maliki contemporary scholarship varies — some opinions treat the full balance as Zakatable each year (Sheikh al-Qaradawi cited); others (more common) treat only the accessible portion. **Uncle Ferox: confirm the default for Maliki users, with the in-memory override letting users opt for the stricter view.**

#### 4.11 Insurance — investment-linked (ULIP, etc.)

Zakatable on **surrender value**, not gross fund value. The user reports the surrender value annually. PR-F8 implements the rule; this ADR establishes the position.

#### 4.12 Insurance — pure protection (term life)

Not Zakatable (no asset value held by the policyholder).

#### 4.13 Cryptocurrency

**Toggleable rule per ADR 0036 (PR-F9).** Maliki contemporary scholarship is divided. Default to: **Zakatable as cash equivalent** at full market value (the prevailing AAOIFI guidance + the position of Sh. Abdul-Bari ath-Thubaity for Maliki users), with a user-toggle to "not Zakatable" if the user follows the alternative view.

**❓ SCHOLAR-VERIFY:** Uncle Ferox to confirm the Maliki default. The user-toggle remains regardless.

#### 4.14 Commodities (gold, silver, precious metals held in physical or paper form)

Zakatable at full market value when above Nisab.

#### 4.15 Forex (currency trading positions)

Zakatable at notional value if held as cash equivalent at Hawl. Trading positions held with intent to sell within the day are treated as trading inventory.

#### 4.16 Debts owed TO the user (loans receivable)

**Maliki position:** the user can choose to:
- (a) Include the debt in the Zakatable base annually (if the debtor is solvent and the user is confident of repayment), OR
- (b) Defer Zakat until repaid, then pay Zakat for all the years that passed in one payment.

Mizan defaults to (a) for solvent debtors; user-overridable.

#### 4.17 Debts owed BY the user (liabilities)

Maliki's position on debt deduction:
- **Immediate debts** (due now): deductible from the Zakatable base, up to the limit that doesn't push the base below zero.
- **Long-term debts** (mortgages, multi-year loans): only the current-year portion is deductible.

This is distinct from the Hanafi position (more permissive on long-term debt deduction). The user's chosen school determines which logic applies. **❓ SCHOLAR-VERIFY: confirm Maliki long-term-debt position.**

#### 4.18 Business assets / inventory

Trading inventory Zakatable at full market value. Fixed assets (machinery, buildings used for production) NOT Zakatable.

#### 4.19 Personal-use assets (car, home furniture, jewellery for personal use)

NOT Zakatable. Jewellery worn for personal use is the source of historic disagreement; Maliki sides with the majority position that worn-jewellery is NOT Zakatable (distinct from Hanafi which Zakat's it).

**❓ SCHOLAR-VERIFY:** The personal-jewellery position is a known Maliki/Hanafi divergence. **Uncle Ferox: confirm the Maliki default (NOT Zakatable for worn jewellery).**

## Consequences

**Positive:**
- Maliki users get rules that match their school's positions
- The 19 asset-class rules above cover the spec §23 reference-user scenario completely
- Where Maliki diverges from other schools (rental property, personal jewellery, long-term debt), the engine routes correctly via the user-selected school

**Negative / accepted:**
- Several `❓ SCHOLAR-VERIFY` markers remain — Uncle Ferox's review will resolve these
- Some rules (#1 Nisab choice, #4.13 crypto default) depend on Uncle Ferox's preferred opinion among multiple acceptable Maliki views

**Risks:**
- A wrong rule = wrong Zakat number for the user. Mitigation: scholarly sign-off precedes code (this ADR), golden-test fixtures encode every rule (PR-F3), and the user has the final override via `user_memory` to set their personally-preferred school + specific overrides.

## Alternatives considered

- **Skip Maliki for v1 and ship only Hanafi/Shafi'i:** rejected — the spec requires four-school coverage.
- **Defer to user-provided rule overrides exclusively:** rejected — most users won't know what to override; the engine needs sensible defaults.
- **Hire a fiqh consultancy for written rule definitions:** parked as a future option if Uncle Ferox is unavailable for the four-school review; not the current path.

## Implementation (locked until Uncle Ferox approves)

- **PR-F3** — `crates/zakat/src/schools/maliki.rs` + 19 asset-class rules + golden tests
- **PR-F12** — `compute_zakat` extension to honour `user_memory.scholarly_school = 'maliki'`
- **PR-F13** — Truth Ledger entry per calculation records the school used + the rule applied per holding

Golden tests will cover at minimum:
- Above-Nisab gold-only / silver-only / mixed scenarios
- Each of the 19 asset-class rules above with at least one positive + one negative case
- The four-school × twelve-asset-class matrix (48 minimum golden cases)
- The spec §23 reference-user scenario produces the expected number when the user's school is Maliki
