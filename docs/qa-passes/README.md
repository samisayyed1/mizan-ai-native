# QA Passes

Numbered, repeatable QA procedures. Per the working agreement past-bug history, each QA pass left a permanent test and a permanent rule. New QA passes follow the same shape.

## Format

Each QA pass document:

1. **Trigger** — what surfaced the bug or motivated the pass
2. **Hypothesis** — the suspected root cause
3. **Procedure** — exact steps to reproduce + verify
4. **Findings** — what was actually wrong
5. **Fix** — what was changed (with file refs)
6. **Permanent test** — what now guards against regression
7. **Permanent rule** — what new constraint was added to the working agreement, if any

## Historical passes (QA-P1 through QA-P18)

These ran before this directory existed. Their permanent rules are encoded in `docs/working-agreement.md` Section 13 (Past Bugs). Examples:

- QA Pass 3 — Date parser (5-format fallback) — encoded in `docs/working-agreement.md` §13
- QA Pass 8 — Silent FX fallbacks — encoded in §13 + `clippy::disallowed_methods` rule
- QA Pass 11 — Cross-consistency: cash totals — encoded in §13
- QA Pass 13 — Frontend vs backend TWR formula mismatch — encoded in §13
- QA Pass 14 — AI valuation tool double-counts TOTAL synthetic — encoded in §13

## Future passes (planned)

- QA-P19 — Truth Ledger emits on AI-initiated writes (Track C foundation)
- QA-P20 — Mizan Badge modifier severity ordering (Track E)
- QA-P21 — Donut + bar chart paint budget (Track A)
- QA-P22 — MCP sandbox gate read-mostly enforcement (Track K)
- QA-P23 — MCP egress DLP filter (Track K)
- QA-P24 — Annual OAuth re-consent worker (Track J)
- QA-P25 — Maliki + Hanbali Zakat school golden tests (Track F)
