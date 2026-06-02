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
| F2 | ⏸️ Pending | Maliki school ADR (0032) + scholarly board sign-off **(CP-F-rules gate)** |
| F3 | ⏸️ Pending | Maliki school rules + golden tests |
| F4 | ⏸️ Pending | Hanbali school ADR (0033) + scholarly board sign-off |
| F5 | ⏸️ Pending | Hanbali school rules + golden tests |
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
