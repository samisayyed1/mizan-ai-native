# Demo runbook — Uncle Feroz walkthrough

The 7-minute investor pitch. Sami's nephew demo for Feroz Siddiqui (Hanafi, $1.71M cross-border portfolio).

This runbook is the single source of truth for the pre-demo dry-run. If a step fails, fix it from this runbook before any further work — never improvise mid-demo.

## 0 · Pre-demo checklist (T-15 min)

Run these once before sitting down with Uncle. Each check is a hard gate — do not start the demo until all pass.

| # | Check | Command / location | Pass criteria |
|---|---|---|---|
| 1 | Branch is clean on `main` | `cd ~/Documents/mizan-ai-native && git status` | `nothing to commit, working tree clean` |
| 2 | Anthropic API key in `.env.local` | `grep ANTHROPIC_API_KEY mizan-4/.env.local` | One line, key prefix `sk-ant-api03-` |
| 3 | Key is gitignored | `git check-ignore mizan-4/.env.local && echo OK` | Prints `mizan-4/.env.local` then `OK` |
| 4 | mizan-connect docker is up locally (if testing Stripe) | `docker ps \| grep mizan-connect` | Container running on `:3000` |
| 5 | Stripe CLI ready for `PR-DEMO-PAY` | `stripe listen --forward-to localhost:3000/v1/webhooks/stripe` (in a separate shell) | "Ready! Your webhook signing secret is whsec_..." |
| 6 | Workspace builds | `cd mizan-4 && cargo build` | Finishes without errors |
| 7 | Frontend type-checks | `cd mizan-4 && pnpm --filter frontend type-check` | `Found 0 errors` |

If check 1 fails: stash with `git stash push -u -m "pre-demo-stash-$(date +%s)"` and verify the demo runs first; resume work after.

## 1 · Enable demo mode (T-5 min)

The Mizan Connect entitlements layer gates Zakat engine / advisor / unlimited AI on a paid subscription. The demo install needs Gold without a real Stripe subscription — that's the `MIZAN_DEMO_MODE=1` override.

```sh
cd ~/Documents/mizan-ai-native/mizan-4

# Print the export lines (idempotent — does not mutate any persistent state):
eval "$(cargo run -q -p mizan-connect --bin mizan-demo-mode -- --tier=gold | grep '^export')"

# Verify:
echo "MIZAN_DEMO_MODE=$MIZAN_DEMO_MODE  MIZAN_ALLOW_PRODUCTION=$MIZAN_ALLOW_PRODUCTION"
# Both must read '1'.
```

The override is **double-gated**: both `MIZAN_DEMO_MODE=1` AND `MIZAN_ALLOW_PRODUCTION=1` must be set in the host environment of the launching process. A stray `MIZAN_DEMO_MODE=1` in a customer install will be silently ignored. See `mizan-4/crates/connect/src/entitlements.rs::demo_mode_active`.

To revert after the demo:

```sh
cargo run -q -p mizan-connect --bin mizan-demo-mode -- --tier=off
# Then in the shell:
unset MIZAN_DEMO_MODE MIZAN_ALLOW_PRODUCTION
```

## 2 · Import Uncle's portfolio (T-3 min)

The seed file (`~/Downloads/uncle_portfolio_seed.json`) carries Feroz's actual $1.71M position across SG / IN / KSA / UAE. The seed importer (`mizan-portfolio-seed-import` crate) parses it into 40+ typed `Operation`s that the persistence layer applies in order with Truth Ledger entries.

Verify the parser handles the real fixture (one-time check; the integration test does this every CI build):

```sh
cd ~/Documents/mizan-ai-native/mizan-4
cargo test -p mizan-portfolio-seed-import --test integration_uncle_feroz
# Expect: 7 passed; 0 failed
```

Wire-up status as of this runbook: parser + Operations enum landed; **persistence consumer (Tauri command + frontend dropzone) is the next PR**. Until that lands, do the demo with a pre-populated dev DB:

```sh
# Restore a snapshot DB that has Uncle's portfolio already loaded:
cp ~/Documents/mizan-snapshots/uncle-feroz-2026-04-04.sqlite \
   ~/Library/Application\ Support/mizan/mizan.sqlite
```

If no snapshot exists, fall back to manually adding the 4 sukuks + 2 equity rollups + 4 ETFs via the UI (~10 min — do this at T-30, not T-3).

## 3 · The 8-step smoke (T-0)

Walk these in order. Each step is a hard gate — if any fails, stop and triage from §4.

1. **Open dashboard** — Net Worth tile reads `~$1,713,968`. Sukuks tile is the largest (`$690,947`). No console errors in DevTools. No truncated labels ("Brokerage Accounts" fully visible).
2. **Bonds & Sukuks panel** — Tap the tile. Four rows: Emaar / Dar al Arkan / Sobha / Binghatti with ISIN, custodian, maturity, current value, P&L. Mizan Badge visible on every figure.
3. **Detail page** — Tap Sobha (largest sukuk). Full detail page renders. Issue date 2023-11-27, maturity 2033-07-20, coupon 8.75%.
4. **AI command bar** — `⌘K` opens. Type "What's my Zakat this year?". Response streams in over Tauri channels with a tool-call indicator. Final answer cites the computed figure with origin (`zakat_engine` + Hanafi).
5. **Zakat page** — Navigate to `/zakat`. School defaults to Hanafi. Computation completes <2s. Breakdown table: Sukuks face value ($690,947), Cash above Nisab through Hawl, Equities tradable portion, Bukit Batok excluded (primary residence per Hanafi). Total Zakat figure prominent at the top.
6. **School selector** — Toggle to Shafi'i → distinct number. Maliki → distinct (rental property NOT for-sale excluded). Hanbali → distinct (mortgage debt deducted if any). All four produce DIFFERENT figures.
7. **Pay Zakat → Stripe** — Tap Pay Zakat. Charity catalog renders. Select Islamic Relief. Stripe Checkout opens in test mode. Card `4242 4242 4242 4242`, any future expiry, any CVC. Webhook fires to mizan-connect. Receipt page renders with Hijri + Gregorian dates.
8. **Advisor page** — Navigate to `/advisor`. Polished "Coming Soon" card with Mizan icon — no `M5.2b` sprint ID leak.

If all 8 pass: the founder gives the 7-minute pitch (script in §5).

## 4 · Recovery — if a step fails mid-demo

| Failing step | Most likely cause | Recovery |
|---|---|---|
| 1 (dashboard) | DB not populated | `Cmd+R` reload; if still empty, restore snapshot per §2 |
| 1 (truncated labels) | CSS regression | Switch demo route to `/holdings` and continue narration; file post-demo |
| 4 (AI doesn't stream) | `ANTHROPIC_API_KEY` not loaded by Tauri | Quit app, `source ~/.zshrc`, re-source `mizan-4/.env.local`, relaunch |
| 4 (tool call fails) | Tool registry mismatch | Skip to step 5 (Zakat works without AI); narrate "the assistant calls the same tool the page renders" |
| 5 (>2s compute) | Cold first run, FX cache miss | Re-run once; if still slow, narrate "first run warms the cache; subsequent computes are instant" and continue |
| 6 (schools produce same number) | School routing not wired through `assess_portfolio` | Hard fail — abort demo, fix `crates/zakat/src/service.rs` post-pitch |
| 7 (Stripe Checkout 4xx) | `stripe listen` not running | Open the second terminal, restart `stripe listen --forward-to localhost:3000/v1/webhooks/stripe`, retry |
| 7 (no webhook) | Network or webhook signing secret mismatch | Narrate the receipt would arrive; show the local audit_log entry as proof of the test transaction |

## 5 · The 7-minute pitch (founder script)

| Time | Beat | Talking point |
|---|---|---|
| 0:00 | Open dashboard | "This is your actual portfolio — $1.7M across four countries. Everything you sent me in that Excel file is here, with the right asset classes and the right cross-currency totals." |
| 1:00 | Sukuks panel | "Click into any tile — same data shape, same depth. Emaar, Dar al Arkan, Sobha, Binghatti. ISIN, custodian, maturity, current value, P&L. Every number carries a Mizan Badge — you click it, you see where the number came from." |
| 2:00 | AI bar | "Now watch this. ⌘K, type the question." Type "What's my Zakat?" → answer streams in. "This isn't a chatbot — it called the same Zakat engine the page uses. The number you see is the number it would compute." |
| 3:30 | Zakat page | "Here's the engine. Hanafi by default — your school. Sukuks face value, cash above Nisab through Hawl, your Bukit Batok flat correctly excluded as primary residence." |
| 4:30 | School comparison | "Switch schools — Shafi'i, Maliki, Hanbali. Every school gets a distinct number because every school has different rules. Maliki excludes rental property not held for sale. Hanbali deducts mortgage debt. We don't approximate." |
| 5:30 | Pay Zakat | "Pay your Zakat from here. Pick the charity, Stripe handles the transaction, receipt with Hijri + Gregorian dates lands in your activity feed, Truth Ledger entry is permanent. Auditable. End to end." |
| 6:30 | Close | "This is what your money looks like when the rules of your deen are first-class citizens of the software. Not a setting buried in a menu. Not a calculator someone built in 2018 and forgot. The product." |

## 6 · Post-demo

Within 24 hours of the demo (regardless of outcome):

1. **Rotate the Anthropic API key.** The key in `mizan-4/.env.local` was pasted into a Claude Code session on 2026-06-14 — it's in conversation transcripts and the session JSONL. https://console.anthropic.com/settings/keys → revoke → reissue → update `.env.local`. **The Resend key from earlier is also still un-rotated** per memory.
2. **Disable demo mode.** `cargo run -q -p mizan-connect --bin mizan-demo-mode -- --tier=off` and `unset MIZAN_DEMO_MODE MIZAN_ALLOW_PRODUCTION` in your shell.
3. **Snapshot the demo DB** for next time: `cp ~/Library/Application\ Support/mizan/mizan.sqlite ~/Documents/mizan-snapshots/uncle-feroz-$(date +%F).sqlite`.
4. **Write down what Uncle said.** The honest feedback is worth more than the slick pitch.
