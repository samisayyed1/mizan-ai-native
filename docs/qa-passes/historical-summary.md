# Historical QA Passes — Pre-`docs/qa-passes/` Directory

The Mizan working agreement §13 ("Past Bugs") encodes the lessons from QA Passes 1 through 18 that ran before this directory existed. The full bug reports live in git history + each pass's PRs; this file is the single-page index so a future engineer can find which historical pass owns which encoded rule.

The lessons are binding — re-reading them before changing the relevant subsystem is the explicit working-agreement §13 expectation.

## Index

| QA # | Trigger | Permanent rule encoded | Location of rule |
|---|---|---|---|
| QA-P1 | Multiple inconsistencies surfaced during enterprise-grade hardening sweep — entitlements lock-step, plaintext token scan, Stripe idempotency, AI tool number cross-check, backup/restore round-trip | Entitlements parity test pins matrix; token-plaintext scan in CI; webhook idempotency by event-id; AI valuation numbers cross-checked against synthesis | working-agreement §3.1, §3.2, §3.3 |
| QA-P2 | qty=0 tombstone positions appearing in holdings UI; net-worth race at cold start | Filter qty=0 from holdings; explicit `await_hydration()` in synthesizer entry | working-agreement §13 |
| QA-P3 | SPLIT corporate action not applied even after recompute; broker dates in 5 different formats | 5-format datetime fallback parser in taxonomies | working-agreement §13, `crates/storage-sqlite/src/taxonomies` |
| QA-P4 | $1.32 rounding drift on $1.7M portfolio from f64 in P&L | `rust_decimal::Decimal` mandatory in money paths; `f64` in money paths is a release blocker | working-agreement §0 rule 2, working-agreement §5, `scripts/lint-no-f64-in-money-paths.sh` |
| QA-P5 | Frontend `fxRate ?? 1` silent fallbacks corrupting net worth | No silent FX fallbacks; every cross-currency op reads `fx_rates` explicitly with timestamp | working-agreement §0 rule 2, ADR (planned) for clippy::disallowed_methods |
| QA-P6 | Sweep of CSV import, backup/restore, retirement areas — multiple smaller-scale issues bundled | Per-subsystem golden tests added for each finding | working-agreement §6 |
| QA-P7 | Activity-currency converters returning silent defaults instead of errors | Converters return `Option<Decimal>` or `Result<Decimal, FxError>` — never silent fallback values | working-agreement §0 rule 2 |
| QA-P8 | Silent FX fallbacks in `synthesis.rs` (cash, position value, account→base rate) | Every FX read in synthesis explicit and timestamped; tests pin the no-fallback behavior | working-agreement §0 rule 2, working-agreement §13 |
| QA-P9 | Zakat under-statement — PrivateEquity + Other silently skipped from zakatable inventory | Zakat engine iterates all asset classes explicitly; surfaces "unknown" rather than dropping | working-agreement §11 |
| QA-P10 | Cash holdings silent FX in `calculate_cash_valuation` | Cash valuation reads fx_rates explicitly; rejects missing rate | working-agreement §0 rule 2 |
| QA-P11 | Cross-consistency: `holdings_snapshot.cash_total_base` vs `daily_account_valuation.cash_balance` differ by $926 | Cross-consistency test pins both calculations against same FX snapshot | working-agreement §6 |
| QA-P12 | Holdings page shows $5k cost-basis fallback while dashboard TOTAL silently treats no-quote positions as $0 | No-quote positions handled consistently: either both treat as cost-basis or both as $0 with explicit warning | working-agreement §13 |
| QA-P13 | Frontend vs backend TWR formula mismatch | Both use the same `crates/financial-truth` (planned crate) implementation; frontend reads via IPC, never recomputes | working-agreement §5 — Money Math is canonical in `crates/financial-truth` |
| QA-P14 | AI valuation tool double-counts TOTAL synthetic account | Synthetic TOTAL account filtered out of account-list queries everywhere | working-agreement §13 |
| QA-P15 | Systemic: filter synthetic TOTAL out of account-list queries across all consumer sites | Single helper `accounts_without_synthetic_total()` used at every query site | working-agreement §13 |
| QA-P16 | `accounts-summary.tsx` `fxRateToBase ?? 1` silent fallback (frontend) | Frontend converters propagate `null` for missing rates; UI shows explicit "rate unavailable" | working-agreement §0 rule 2 |
| QA-P17 | Alt-asset valuation uses lenient 1.0 FX fallback for `market_value.base` | Alt-asset valuation rejects missing rate; surfaces error to user | working-agreement §0 rule 2 |
| QA-P18 | Activity date stored as midnight UTC drifts by 1 day for Western timezones | Activity dates stored as `NaiveDate` (no time) in DB; displayed in user TZ at render | working-agreement §13 |

## Distilled invariants the 18 passes left behind

These are the lessons that became permanent rules — they're now structural to the codebase:

1. **Money is Decimal, never f64.** QA-P4 + scripts/lint-no-f64-in-money-paths.sh.
2. **FX rates are read explicitly with timestamps, never silently defaulted.** QA-P5, P7, P8, P10, P16, P17.
3. **Cross-consistency is tested.** Same number computed two ways must match — QA-P11, P13.
4. **Silent fallbacks corrupt invisibly.** Use `Result` / `Option` to surface, never `unwrap_or(default)` on money paths.
5. **Synthesizers wait for hydration.** No race on cold start — QA-P2.
6. **Synthetic accounts are filtered at query sites, not at render.** QA-P14, P15.
7. **Dates store and render in user-explicit semantics.** No silent TZ conversion — QA-P18.
8. **Brokers return data in many formats.** Fallback parsers tested across all known shapes — QA-P3.
9. **Asset classes are iterated explicitly in Zakat.** No silent drops — QA-P9.
10. **Tests are added with every bug fix.** Working-agreement §6 + §13.

## Going forward

New QA passes (QA-P19 onward) get their own file in this directory using the [template](QA-P19-template.md). The historical summary above is locked — new pass detail goes in its own file, not retro-edited into here.
