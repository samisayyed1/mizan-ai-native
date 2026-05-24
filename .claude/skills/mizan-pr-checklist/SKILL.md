---
name: mizan-pr-checklist
description: Use before committing or pushing any change. Runs the Phase N verification block (cargo check, clippy, cargo test, pnpm type-check, lint, vitest, build) in both mizan-4 and mizan-connect. Halts on first red.
---

# Pre-commit / pre-push checklist

Run this before every commit. Halts on first red. No "I'll fix it
later" exceptions — broken main means broken builds means silent
failures means the user trusts nothing.

## Desktop (`mizan-4/`)

In order, halting on the first failure:

```bash
cd ~/Documents/mizan-ai-native/mizan-4

cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib

pnpm --filter frontend type-check
pnpm --filter frontend lint:quiet
pnpm --filter frontend test -- --run
pnpm --filter frontend build
```

If `cargo clippy` reports a warning, fix it — never silence with
`#[allow(...)]` unless the warning is genuinely spurious and you can
explain why in a comment.

## Backend (`mizan-connect/`)

```bash
cd ~/Documents/mizan-ai-native/mizan-connect

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo sqlx prepare --check          # if any SQL changed
cargo test --workspace
```

If `sqlx prepare --check` fails, run `cargo sqlx prepare` and commit
`sqlx-data.json` alongside the SQL diff.

## Repo-wide

```bash
cd ~/Documents/mizan-ai-native
pnpm prettier --check .             # or whatever the configured glob is
```

## What this does NOT cover

- The DraftActionGraph runtime tests (use `mizan-action-graph-validator`).
- The Feroz invariants audit (use `feroz-invariants-check` for any
  dashboard/portfolio/holding/liability/onboarding change).
- The AI safety contract (use `ai-truth-contract` for any change to
  AI tools / system prompt).
- End-to-end provider validation (use `plaid-sandbox-test`,
  `snaptrade-sandbox-test`, `stripe-test-mode`).
- The "No Silent Failure" certification (v3.1 §25) — only run before
  release, not every commit.

## When done

Commit with a one-line subject + a body that explains _why_ (not what).
Push only after the `MIZAN_ALLOW_PRODUCTION=1` consideration — for any
push, confirm we are pushing to `samisayyed1/mizan-ai-native`, not the
deprecated `samisayyed1/mizan-4`.
