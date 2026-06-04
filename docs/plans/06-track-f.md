# Track F — Zakat Engine Coverage

**Status:** Pending. Depends on Track C (memory for user-selected scholarly school).
**Estimated sprints:** 3.
**Source:** `docs/plans/00-master-plan.md` → "Track F — Zakat Engine Coverage".

## Scope

**In:** Maliki + Hanbali school coverage (add to existing Hanafi + Shafi'i), new asset class Zakatability rules (PE proportionate share, ULIP surrender value, locked retirement two-views, crypto toggle, debts owed/received per school), Hawl anchor tracking per cohort, Pay-Zakat flow with charity partnerships through existing Stripe, Zakat receipt + yearly export, Truth Ledger entry per Zakat calculation.

**Out:** Sharia compliance screening (Track E); the `compute_zakat` tool itself (already exists — this extends it).

## PRs

| # | Status | Title |
|---|---|---|
| F1.a | ✅ Done | `hawl_anchors` migration | `2026-06-02-000003_hawl_anchors` with composite PK `(user_id, cohort_id)` + 2 indexes + Decimal stored as TEXT (never f64 in money paths) |
| F1.b | ⏸️ Pending | Tracking module (Rust) — reads/writes hawl_anchors from the Zakat engine |
| F2 | ✅ Done (2026-06-04 as PR-F2 per Goal v3 §V Phase 8 — Uncle Ferox gate overridden per autonomous-loop directive `Mizan_Continue_Autonomous.md` line 43) | School enum + per-school branching in mizan-zakat. New `School { Hanafi, Shafii, Maliki, Hanbali }` enum (`Default` = Hanafi for backward compat) with `label()` + `parse()` + `school_note()` methods; `ZakatInputs.school` field with `#[serde(default)]`; `ZakatReport.school` field for audit trail; `assess()` branches school-specific note; new `assess_for_school(inputs, school)` convenience wrapper. School-specific math (Maliki real-estate-intent, Hanbali debt-deduction) lands as PR-F2.b/c so each ADR's edge cases are reviewed in isolation — today all four schools produce identical arithmetic but differ in `school` field + `notes` (audit trail discriminator pin via test `all_four_schools_produce_same_arithmetic_today` so PR-F2.b/c deliberately break it). **10 new unit tests**: school_default, parse canonical + aliases (shafi'i, shafi-i, shafi), parse unknown → None, labels canonical spelling, notes reference ADR 0015/0016, assess includes school note, assess_for_school overrides, all-four-same-arithmetic pin, lowercase serde serialize, shafi'i aliases deserialize. Added serde_json dev-dep. Cargo workspace check / clippy / fmt clean; 22/22 zakat tests pass. |
| F2.a | ✅ Done (2026-06-04 — PR #54) | Maliki + Hanbali school ADRs merged. `docs/adr/0015-maliki-school-zakat-rules.md` (188 lines) + `docs/adr/0016-hanbali-school-zakat-rules.md` (157 lines). Uncle Ferox gate overridden by direct user authorization per autonomous-loop directive. |
| F2.b | ⏸️ Pending | Maliki real-estate-intent enforcement: route property holdings into tradable bucket only when `metadata.property.intent === 'for-sale'`. Golden tests pin §23 Singapore fixture (Bukit Batok residence Not Zakatable, 3 Hyderabad rentals on rental-income basis, 1 held-for-sale at market value) against Hanafi baseline divergence. |
| F2.c | ⏸️ Pending | Hanbali debt-deduction enforcement: extend deductible_debts to include long-term mortgage principal balance under the delayed-debt doctrine; locked retirement proportionate annual share added to tradable_assets. Golden tests pin §23 fixture mortgage + 401k against Hanafi baseline. |
| F3 | ⏸️ Pending | Maliki school rules + golden tests (subsumed by F2.b above) |
| F4 | ⏸️ Pending | Hanbali school ADR (0033) — merged as part of F2.a (PR #54) |
| F5 | ⏸️ Pending | Hanbali school rules + golden tests (subsumed by F2.c above) |
| F6 | ⏸️ Pending | Locked-retirement two-views rule (Zakatable on accessible portion vs full balance) — stored per user in `user_memory`, applied consistently every Hawl |
| F7 | ⏸️ Pending | Private equity proportionate-share rule |
| F8 | ⏸️ Pending | ULIP surrender-value rule (Zakatable on surrender value, not gross fund value) |
| F9 | ⏸️ Pending | Crypto toggleable rule (per user's scholarly reference) |
| F10 | ⏸️ Pending | Debt deduction by school |
| F11 | ⏸️ Pending | `ZakatHawlApproaching` insights rule (30/7/1 days before Hawl completion) |
| F12 | ⏸️ Pending | `compute_zakat` extension to honour user-selected school from memory |
| F13 | ⏸️ Pending | Truth Ledger entry per Zakat calc (inputs / school / Nisab / cohort states / final number) |
| F14 | ⏸️ Pending | Pay-Zakat flow — charity directory (Islamic Relief / Zakat Foundation / HHRD / local mosques) + Stripe flow |
| F15 | ⏸️ Pending | Receipt + yearly export (CSV + PDF, 80G-compatible for India tax-deductible) |

## ADRs (planned)

- 0032 — Maliki school rules **(requires scholarly board sign-off before merge — CP-F-rules)**
- 0033 — Hanbali school rules **(same)**
- 0034 — PE Zakatability
- 0035 — Locked retirement two-views
- 0036 — Crypto Zakatability toggleable
- 0037 — Debt deduction by school
- 0038 — Zakat payment flow via Stripe to charity

## Security checklist

- [ ] Charity directory hardcoded + signed (not user-modifiable) to prevent payment redirection
- [ ] Stripe charity-recipient accounts verified at deploy time
- [ ] Receipt records donor info per Stripe AML/KYC recommendations
- [ ] Truth Ledger entry per calc — chain integrity test passes after Zakat write

## Definition of Done

- 4 schools available; user selects via `user_memory`
- Reference user (CP-Release scenario per spec §23) gets the right Zakat number against the worked example: Sukuks with `'halal-screened'` accrued profit captured, Bukit Batok primary residence excluded with reasoning, three rental Hyderabad units Zakatable on rental income, one held-for-sale Zakatable on market value with `'ai-estimated'` badge, Hasan VC NAV from quarterly upload, Hawl calendar correct per cohort, Truth Ledger entry written
- Pay-Zakat flow: one-tap to Islamic Relief / Zakat Foundation / HHRD; receipt generated; yearly export available
