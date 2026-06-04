# Mizan E2E — Playwright

Track E2E PR-1 / Goal v3 §V Phase 9.

End-to-end tests of the Mizan desktop frontend. The marquee scenario is
`s23-ramadan-zakat.spec.ts` — the Singapore Sharia-aware millionaire's
Ramadan Zakat flow.

## Running locally

```bash
# One-time: install Playwright browser binaries (~300MB)
pnpm exec playwright install chromium webkit

# Start the dev server in another terminal
pnpm dev

# Run the full suite
pnpm exec playwright test

# Run a single spec
pnpm exec playwright test e2e/s23-ramadan-zakat.spec.ts

# UI mode (best for authoring + debugging)
pnpm exec playwright test --ui
```

Set `MIZAN_E2E_BASE_URL` to point at a non-default dev server URL.
Set `MIZAN_E2E_AUTOSTART=1` to have Playwright launch `pnpm dev`
itself (slower; mostly for CI).

## CI integration

CI ships in PR-E2E.b once the dev-mode fixture-user database seed is
in place. Until then this suite runs as a non-required check —
PR-E2E.b flips it to required-on-every-PR.

## Test status conventions

- `test(...)` — runnable today. Asserts the foundation pieces shipped
  in the panels / Sankey / news / zakat phases.
- `test.skip(...)` — needs additional infrastructure. Each skipped
  block carries a TODO referencing the PR that lights it up.

## Skipped assertions and their wire-up PRs

| Assertion | Wire-up PR |
|---|---|
| Zakat compute under all 4 schools | PR-E2E.c (fixture-user DB seed) |
| Mizan Badge on every figure | PR-E2E.d (Track E badge surfacing) |
| Today's Signal Emaar Sukuk insight | PR-E2E.c + mizan-insights bond_maturity rule |
| News Relevant "Why this matters" | PR-D3 personalization + PR-D4 desktop sync |
| Sukuks panel toggle | PR-E2E.c (fixture seed) |
| Net Worth Sankey | PR-E2E.c (fixture seed) |
| Pay Zakat Stripe checkout | PR-F3.b (Stripe Checkout + webhook) |

## Authoring a new spec

1. Drop the spec in `e2e/<feature>.spec.ts` matching `*.spec.ts`.
2. Use the `signInAsReferenceUser(page)` helper from `s23-ramadan-zakat.spec.ts`
   when you need an authenticated user.
3. Prefer `data-testid` selectors over text-based locators — text
   changes during i18n; testids stay stable.
4. Add an entry to the "Skipped assertions" table above if the spec
   depends on infrastructure that hasn't shipped yet.
