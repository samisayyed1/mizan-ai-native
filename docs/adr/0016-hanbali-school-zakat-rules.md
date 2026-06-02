# ADR 0016 — Hanbali school Zakat rules

| Status | 🟡 **PENDING SCHOLARLY APPROVAL — Uncle Ferox** |
|---|---|
| Date drafted | 2026-06-03 |
| Drafted by | ai (auditor; under autonomous-execution authorization) |
| Reviewers required | Uncle Ferox (fiqh al-muamalat — registered Track F reviewer per [docs/REVIEWERS.md](../REVIEWERS.md)) |
| Final-sign-off requirement | Per [feedback-autonomous-execution-mode](../../../.claude/projects/-Users-samisayyed-mizan-ai-native/memory/) — autonomous-execution authority **does not** apply to Track F fiqh substance. Uncle Ferox's approval is the gate. |
| Related | [0015-maliki-school-zakat-rules](0015-maliki-school-zakat-rules.md) (sibling — same review batch) |

---

## ⚠️ This is a DRAFT for scholarly review

Companion to ADR 0015 — same shape, same `❓ SCHOLAR-VERIFY` convention. Do NOT implement (`crates/zakat/src/schools/hanbali.rs`) until Uncle Ferox signs off.

---

## Context

See [ADR 0015](0015-maliki-school-zakat-rules.md) §"Context" — identical motivation. This ADR is the Hanbali sibling.

## Hanbali-school rules

### 1. Nisab thresholds

Same gold/silver thresholds as the other Sunni schools (85g gold / 595g silver, lower-of-both as the operative threshold per the AAOIFI standard).

**❓ SCHOLAR-VERIFY:** Hanbali contemporary scholarship is generally aligned with the AAOIFI lower-of-both; legacy literature sometimes prefers gold-only. **Uncle Ferox: confirm default.**

### 2. Zakat rate

**2.5% (1/40th)**, identical to other Sunni schools.

### 3. Hawl

Cohort-based, identical pattern to other schools. Hanbali is more lenient than Maliki on continuous-holding interruptions — most Hanbali opinions allow the cohort to continue even if wealth dips below Nisab mid-year, so long as it's above Nisab at the Hawl date.

**❓ SCHOLAR-VERIFY:** Confirm the default — Mizan's current implementation matches the lenient Hanbali position. Uncle Ferox should confirm this is OK to apply to Hanbali users (which would mean Maliki + Hanbali users get different mid-year-dip semantics).

### 4. Asset-class-specific rules

#### 4.1 Cash, bank balances, savings accounts

Zakatable at full value. Same as other schools.

#### 4.2 Equities — long-term investment intent

Hanbali contemporary scholarship (e.g. Sheikh Salih al-Fawzan, the AAOIFI standard which is heavily Hanbali-aligned) endorses the **proportional Zakatable-assets** approach for Sharia-compliant equity holdings — same as Maliki §4.2 default.

#### 4.3 Equities — trading intent

Full market value Zakatable as trading inventory.

#### 4.4 Brokerage / mutual funds / ETFs

Same as Maliki §4.4 — apply equities rule to underlying holdings; honour fund-disclosed purification amounts when available.

#### 4.5 Bonds & Sukuks

- **Conventional bonds:** Hanbali takes the same position as Maliki — principal is Zakatable wealth (held as a debt receivable), the interest is non-permissible and must be donated (not Zakat). The user is advised to divest.
- **Sukuks:** Zakatable on market value + accrued profit, like a permissible bond.

#### 4.6 Real estate — primary residence

Not Zakatable.

#### 4.7 Real estate — held for rental

**Hanbali position:** Same as Maliki — the property is NOT Zakatable; the rental income is Zakatable on receipt.

**❓ SCHOLAR-VERIFY:** This is one of the most agreed-upon four-school positions, but confirm Hanbali doesn't have a contemporary divergence we should encode.

#### 4.8 Real estate — held for sale (trade inventory)

Zakatable on full market value at Hawl. Same as other schools.

#### 4.9 Private equity — long-term

Proportional Zakatable share of the fund's Zakatable assets, same as Maliki §4.9.

#### 4.10 Pension / provident funds — locked

**Hanbali position (proposed):** majority Hanbali contemporary opinion treats the LOCKED portion as not-yet-Zakatable until accessible. Same default as Maliki §4.10.

**❓ SCHOLAR-VERIFY:** Sheikh Ibn Uthaymin's position differs (full balance Zakatable annually); Mizan should default to the more lenient majority position but allow the user-memory override to the stricter view. Confirm the default for Hanbali users.

#### 4.11 Insurance — investment-linked (ULIP)

Zakatable on surrender value, same as Maliki §4.11.

#### 4.12 Insurance — pure protection

Not Zakatable.

#### 4.13 Cryptocurrency

**Toggleable per ADR 0036 (PR-F9).** Hanbali contemporary scholarship: the Saudi Hanbali establishment leaned cautious early but the AAOIFI 2024 standard (heavily Hanbali-aligned) is broadly accepting of cryptocurrency as a Zakatable wealth category for users who treat it as currency.

**Default:** Zakatable as cash equivalent. **❓ SCHOLAR-VERIFY: Uncle Ferox to confirm Hanbali default.**

#### 4.14 Commodities

Same as Maliki §4.14.

#### 4.15 Forex

Same as Maliki §4.15.

#### 4.16 Debts owed TO the user

Same general framework as Maliki §4.16: user picks (a) include annually or (b) defer-to-receipt. Hanbali typically encodes a slightly more lenient default — most Hanbali opinions endorse option (b) for any debt that isn't immediately collectible.

**❓ SCHOLAR-VERIFY:** Mizan defaults to (a) for solvent debtors. Confirm Hanbali users should default to (b) for non-immediately-collectible debts.

#### 4.17 Debts owed BY the user (liabilities)

**Hanbali position on debt deduction:** Hanbali is generally MORE permissive than Maliki on long-term debt deduction. Most contemporary Hanbali scholars allow deducting the FULL outstanding long-term debt (e.g. mortgage principal) from the Zakatable base in the year it would otherwise be Zakatable, subject to:
- The debt must not exceed total Zakatable wealth
- The debt must be a recognised obligation (not a vague future commitment)

This is distinct from Maliki §4.17 (only current-year portion deductible) and is one of the more practically important divergences between the schools.

**❓ SCHOLAR-VERIFY:** This is the highest-impact divergence — users with mortgages get materially different Zakat amounts between Maliki + Hanbali. **Uncle Ferox: confirm the Hanbali full-deduction default is correct, or specify the upper bound (e.g. "no more than 12 months' worth of mortgage payments").**

#### 4.18 Business assets / inventory

Same as Maliki §4.18.

#### 4.19 Personal-use assets (jewellery)

Hanbali sides with the **majority** position — worn jewellery is NOT Zakatable. Same as Maliki §4.19. This differs from the Hanafi position (Hanafi Zakats worn jewellery).

## Consequences

**Positive:**
- Hanbali users get rules matching their school's positions
- The most important divergences (long-term debt deduction §4.17, locked-retirement default §4.10) are documented and encoded correctly
- The user-memory override mechanism handles edge cases

**Negative / accepted:**
- Several `❓ SCHOLAR-VERIFY` markers — Uncle Ferox's review will resolve

**Risks:**
- Long-term debt deduction §4.17 is the highest-stakes rule (materially affects Zakat amount for mortgage-holding users). Golden tests will pin the expected behaviour.

## Alternatives considered

Same alternatives as ADR 0015 — rejected for the same reasons.

## Implementation (locked until Uncle Ferox approves)

- **PR-F5** — `crates/zakat/src/schools/hanbali.rs` + 19 asset-class rules + golden tests
- **PR-F12** — `compute_zakat` extension to honour `user_memory.scholarly_school = 'hanbali'`
- **PR-F13** — Truth Ledger entry per calculation records the school used + the rule applied per holding

Golden tests will cover the same matrix as Maliki (ADR 0015) plus Hanbali-specific divergences (§4.16, §4.17, §4.19).
