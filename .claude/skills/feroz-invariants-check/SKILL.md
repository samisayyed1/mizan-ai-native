---
name: feroz-invariants-check
description: Use after any change to dashboard, portfolio, asset class, holdings, liabilities, net worth, goals, or onboarding code. Audits the diff against the 20 binding invariants from the May 17, 2026 Feroz Siddiqui meeting (v3 §8).
---

# Feroz invariants audit

The 20 binding invariants from the May 17, 2026 meeting are
load-bearing for the entire product. Any drift breaks the model.
This skill is a read-only audit — it doesn't change code, it reports
whether the change holds the invariants.

## The invariants (v3 §8 — binding)

1. **"Accounts" is renamed to Portfolio everywhere.** Any user-visible
   string saying "account" in the new model is wrong (banking
   institution rows are "accounts" inside the Bank Accounts asset
   class; that's the only place the word survives).

2. **Dashboard hierarchy**: `Dashboard → Portfolio → Asset Class → Holdings`.
   The dashboard does NOT show holdings; it shows portfolios + goals
   - net worth + a consolidated history graph.

3. **Portfolios are multi-currency containers.** Each portfolio picks
   a currency at create time. Mixed-currency portfolios are
   first-class — every holding inside carries its own currency.

4. **Bank Accounts is an asset class.** Each bank = a holding inside
   the Bank Accounts class. Multi-currency per bank is fine.

5. **Vehicles are excluded from net worth.** They're depreciating; the
   user can track them outside net worth if they want, but not inside.

6. **Liabilities are first-class.** Each row has: type, current
   balance, balance date, origination date, duration, optional %.
   **EMI is the monthly payment, NOT the liability.** Never store EMI
   in the balance field.

7. **Primary / master dashboard currency** lives in Settings → Currency.
   All cross-portfolio aggregates convert to this currency. FX rates
   carry source + as-of timestamp.

8. **Custom goals exist.** A goal has a name, target amount, target
   currency, optional target date, optional linked portfolios.

9. **Free tier: 1 portfolio + 20 holdings.** Silver+ removes both caps.

10. **AI is the front door.** Onboarding never directs users to the
    Add-Asset wizard as the recommended path.

11. **3 example liabilities seed on first launch** with `metadata.example=true`
    and name prefix `"Example —"`. Edit-first: when the AI proposes
    `create_liability` and an example matches, it must `update_liability`
    instead.

12. **Manual entry is first-class.** Every flow that supports Plaid/
    SnapTrade also supports manual entry with parity affordances
    (Manual pill, Update Balance quick-action).

13. **Plaid never proposed for unsupported countries** (India, UAE,
    etc.). System prompt enforces.

14. **Zakat moves to Gold.** Don't surface it on Free/Silver dashboards
    without a Gold upgrade CTA.

15. **Mizan Connect is part of the product.** Sign-in, upgrade,
    entitlement, provider sync, AI proxy all flow through one
    coherent UX.

16. **No fallback systems, no masking.** Provider failures surface
    with specific reasons. Never "Loading..." that lies.

17. **Every AI write requires user confirmation.** No silent mutation.

18. **DraftActionGraph commits atomically.** Either every node in a
    multi-action plan applies, or none.

19. **Money never `f64`.** `rust_decimal::Decimal` everywhere.

20. **Every number has provenance** (Phase O). When the user asks
    "why is my net worth $X?", the app explains by source + timestamp
    - confidence.

## How to audit

For the supplied diff or working tree:

1. Identify which invariant areas the change touches (dashboard,
   portfolio, asset class, holding, liability, net worth, goal,
   onboarding, AI tool, provider sync, currency).
2. For each touched area, walk the relevant invariants above.
3. Report `Pass / Fail / Indeterminate` per invariant with a one-line
   justification. Indeterminate = "this change is fine on its own but
   relies on a downstream invariant that's not yet implemented" —
   note the dependency.
4. If any Fail, propose the minimal patch.

## Output shape

```
Invariant audit for <change description>:

Touched areas: [Liabilities, Net worth]

Invariant 6 (EMI != balance): Pass.
   Reason: liability balance stored as `current_balance: Decimal`;
   monthly payment in `monthly_emi: Decimal`.
Invariant 19 (Decimal not f64): Pass.
Invariant 11 (Edit-first): Indeterminate.
   Reason: seed_examples.rs not yet implemented (Phase C).

Verdict: 2/3 Pass, 1 Indeterminate (depends on Phase C). No Fail.
```
