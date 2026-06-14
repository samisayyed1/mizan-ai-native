# Mizan — Production Readiness Declaration v2

**Date:** 2026-06-14
**Author:** Atomic-grade production sweep (sessions 1–4)
**Supersedes:** Any prior "PR-READY" declaration

This document grounds the readiness claim in **actual code state, not
audit-doc claims**. Several earlier audits over-reported gaps that were
already closed on `main`; this declaration was written by reading the
codebase, not by trusting prior summaries.

---

## Section 1 — Atomic-grade sweep PR ledger

Three days of focused work landed on `main`:

| # | Title | Block |
|---|---|---|
| [#178](https://github.com/samisayyed1/mizan-ai-native/pull/178) | Dashboard IA + world-class UI pass (ADR-0018b) | PR-FIX-3 |
| [#179](https://github.com/samisayyed1/mizan-ai-native/pull/179) | Silent `unwrap_or(Decimal::ONE)` FX fallback killed | PR-FIX-1 |
| [#180](https://github.com/samisayyed1/mizan-ai-native/pull/180) | Release-gate workflow rescued | PR-FIX-5 |
| [#181](https://github.com/samisayyed1/mizan-ai-native/pull/181) | `audit_log` wired into Connect handlers | PR-FIX-2 |
| [#182](https://github.com/samisayyed1/mizan-ai-native/pull/182) | `.env.example` + LOCAL-DEV-SETUP.md | PR-WIRE-1 |
| [#183](https://github.com/samisayyed1/mizan-ai-native/pull/183) | Web adapter graceful degradation + `WebUnavailableError` | PR-WEB-1 |
| [#184](https://github.com/samisayyed1/mizan-ai-native/pull/184) | Coverage-floor CI gate (initial — bug found later) | PR-FIX-6 |
| [#185](https://github.com/samisayyed1/mizan-ai-native/pull/185) | `pre-canary-check.sh` 10-item environment gate | PR-PROD-SCRIPT |
| [#186](https://github.com/samisayyed1/mizan-ai-native/pull/186) | FX rates UI / advisor tile — no leaked provider names or sprint IDs | PR-CLEAN-1 |
| [#187](https://github.com/samisayyed1/mizan-ai-native/pull/187) | `domain-types` 0% → ≥90% + per-crate path filter | PR-COV-4 |
| [#188](https://github.com/samisayyed1/mizan-ai-native/pull/188) | `csv-import` 87.87% → 94.47% + workflow split into reusable | PR-COV-3 |
| [#189](https://github.com/samisayyed1/mizan-ai-native/pull/189) | `financial-truth` 90.39% → 97.42% | PR-COV-2 |
| [#190](https://github.com/samisayyed1/mizan-ai-native/pull/190) | `MIZAN_ADDON_STORE_API_BASE_URL` override + secrets audit | PR-HARDEN-1 |

**Two real CLAUDE.md §0 violations cleared** in this sweep:
1. Silent FX fallback (§0 rule 2) — removed the `get_fx_rate_or_fallback` helper, routed every call site through `try_get_fx_rate`, added a contract-pin test that fails loudly if any future PR reintroduces `unwrap_or(Decimal::ONE)` on an FX lookup
2. Dormant audit log — every state-mutating Connect handler now emits an `audit::record_event` row (billing webhook, checkout, portal, admin overrides, SnapTrade lifecycle, team invites)

---

## Section 2 — Code-quality metrics (post-sweep)

| Metric | Value | Source |
|---|---|---|
| Coverage: `financial-truth` | **97.42%** lines | PR #189, cargo-llvm-cov 0.6.18 |
| Coverage: `zakat` | **97.73%** lines | per-crate measurement (the prior 3.85% was a measurement bug — fixed by the workflow rewrite in #188; the actual crate-local tests were always there) |
| Coverage: `csv-import` | **94.47%** lines | PR #188 |
| Coverage: `domain-types` | **≥90%** lines | PR #187 |
| Coverage floors (CI-enforced) | 95/95/90/90 | `.github/workflows/coverage-floors.yml` + `coverage-floors-job.yml` |
| TypeScript errors | 0 | `pnpm --filter frontend type-check` |
| ESLint warnings | 0 on changed files | `pnpm --filter frontend lint:quiet` |
| Clippy warnings on changed crates | 0 | `cargo clippy -p X --all-targets -- -D warnings` |
| `cargo audit` vulnerabilities | 0 known at last CI run | CI history |
| `println!` / `dbg!` / `eprintln!` in production Rust | 0 | grep audit |
| Stray `console.log` in shipped frontend | 0 | grep audit (the two refs in `stream.ts` are JSDoc @example; `core.ts:344` is the logger adapter's intentional pass-through) |
| Hardcoded API keys / tokens / secrets | 0 | PR-HARDEN-1 audit |
| `.env*` files in `.gitignore` | ✓ (with `!.env.example` exception) | confirmed |

---

## Section 3 — CLAUDE.md §0 rule enforcement

| Rule | Enforcement | Status |
|---|---|---|
| 1. Truth Ledger sanctity — every Zakat number backed by a hash-chained ledger entry | `crates/financial-truth/` — SHA-256 hash chain, `GENESIS_PREV_HASH`, `canonical_payload` BTreeMap-sorted; 97.42% line coverage; 5 new contract-pin tests in PR #189 | ✅ enforced |
| 2. No silent FX fallbacks. Ever. | PR #179 removed the lenient helper; the `test_no_silent_fx_fallback_anywhere` contract-pin test fails CI if any future PR re-introduces `unwrap_or(Decimal::ONE)` on an FX lookup. Plus the `lint-fx-silent-fallback.sh` script in CI hygiene scan | ✅ enforced |
| 3. No plaintext tokens on disk | All third-party tokens (Plaid, SnapTrade) AES-256-GCM encrypted via `SecretCipher` and per-provider keys; PR-HARDEN-1 audit confirmed no token in committed code; `.env.fly` in `.gitignore` | ✅ enforced |
| 4. Forward-only migrations | `mizan-connect/migrations/` is the only path; `lint-migration-cache-manifest.sh` runs in CI hygiene scan | ✅ enforced |
| 5. No f64 in money paths | `lint-no-f64-in-money-paths.sh` runs in CI hygiene scan; financial paths use `rust_decimal::Decimal` throughout | ✅ enforced |
| 6. 95% coverage floors on financial-truth/zakat/ai/dispatcher/auth/billing/webhooks | `.github/workflows/coverage-floors.yml` hard-gates financial-truth (95%), zakat (95%), csv-import (90%), domain-types (90%). Connect-side floors (auth/billing/webhooks) covered by the existing CI test suite; no per-crate quantitative gate yet — those crates live in `mizan-connect/src/` and the workflow only currently covers `mizan-4/crates/` | ⚠️ partial — `mizan-4/crates/` gated; mizan-connect modules tested but not floor-gated |

---

## Section 4 — Architecturally available but not yet wired

These items have schema + types + supporting code on `main`. The
handler-level wiring is the remaining work. Each is a well-scoped,
multi-hour-to-multi-day PR.

| Item | What's done | What's NOT done |
|---|---|---|
| **OAuth handlers (Track J PR-J1.b)** | Migration 0012 (3 tables: oauth_providers, user_oauth_connections, oauth_suggestions); `mizan-connect/src/oauth/{types.rs,catalog.rs}` with 8 providers + scopes | Route handlers + router wiring for `POST /v1/oauth/connect/*`, `GET /v1/oauth/callback/*`, `POST /v1/oauth/disconnect/*`, `GET /v1/oauth/connections` |
| **MCP gateway (Track K)** | Migration 0013; `mizan-connect/src/mcp/{types.rs,sandbox.rs,egress_dlp.rs}` | Route handlers + 30+ adversarial DLP tests + 5-server hand-vetted catalog seed |
| **Advisor drill-down (Track G PR-M5.2b)** | Migration 0014 (advisor_links, advisor_sign_offs); frontend tile renders polished "in active development" line (post PR-CLEAN-1) | Client list page, read-only portfolio view, note-taking, sign-off action |
| **Hanbali debt deduction (Track F PR-F2.c)** | `property_intent.rs` table documents the F2.c behaviour as pending | `assess_portfolio` wiring: long-term mortgage deduction, proportionate-share on locked retirement |
| **Per-user / per-endpoint rate limiting (PR-HARDEN-3 extension)** | `tower_governor` wired with global per-IP `SmartIpKeyExtractor` (correct for Fly's edge proxy); `RATE_LIMIT_PER_MINUTE` env var documented | Per-endpoint limits (auth 10/min, billing 5/min/user, Plaid 20/min/user, MCP 30/min/user, OAuth 10/min/user, other 60/min/user). Needs auth middleware to run before rate limit + keying by user_id. |
| **§23 Playwright E2E (PR-TEST-E2E)** | 1 runnable + 8 skipped at `apps/frontend/e2e/s23-ramadan-zakat.spec.ts` | §23 fixture-seed wiring (the reference user portfolio loaded into SQLite before each test run) |
| **Zakat UI / Advisor / Health detection unit tests** | Health detection has 1 test (issue detail sheet); Advisor + Zakat UI have 0 tests | PR-TEST-UI-1/2/3 |
| **Live-API sandbox verification (Block B)** | Code paths for Plaid Link, SnapTrade portal, Stripe Checkout, MetalpriceAPI, Twelve Data, NewsAPI, Anthropic all exist and are wired | End-to-end live verification with your sandbox keys (not runnable inside this environment) |

---

## Section 5 — Already shipped that prior audits over-reported as missing

These items were claimed "missing" in the continuation-prompt MDs but
are actually on `main`. Verified by reading the code; future audits
should start by checking these paths before recommending fresh work.

- **6 AI write tools** (PR-AI-1): `mizan-4/crates/ai/src/tools/{create_account,update_account,add_alternative_asset,create_liability,update_liability,create_goal}.rs` — 31 integration tests, dispatcher-wired via the `invoke!` macro in `agent_dispatcher.rs`
- **Command palette** (PR-AI-2): `apps/frontend/src/components/app-launcher.tsx`, Cmd/Ctrl+K via the `cmdk` library
- **Sentry initialization**: `mizan-connect/src/telemetry.rs:62-76` with DSN from env, traces sample rate configurable
- **Rate limiting middleware**: `tower_governor` + `SmartIpKeyExtractor` in `server.rs:10-11,28` (per-IP global; per-user/per-endpoint extension noted in §4)
- **CORS**: `CorsLayer` + `build_cors(config)` in `server.rs:206-222`, allowed origins from env, wildcard rejected at startup when auth endpoints are present
- **Maliki property-intent routing (F2.b)**: `crates/zakat/src/property_intent.rs` ships routing table; `service.rs:91-117` wires it into `assess_portfolio`
- **Secrets-inventory + sentry-alerts runbooks**: already at `docs/runbooks/`
- **`audit_log` Plaid handler integration**: 3 sites in `plaid/handlers.rs` (PR #181 added every OTHER handler module)
- **`connect_test.rs`**: 117 LOC of real path-alias coverage tests (NOT the empty stub the audit claimed)

---

## Section 6 — Remaining human actions

These are not things code can resolve. The user has to do them.

### A. Paid production API keys

Each one has a documented sign-up + paste-target in `docs/LOCAL-DEV-SETUP.md`:

- Plaid production (`PLAID_CLIENT_ID` + `PLAID_SECRET`, `PLAID_ENV=production`)
- SnapTrade production (`SNAPTRADE_CLIENT_ID` + `SNAPTRADE_CONSUMER_KEY`, `SNAPTRADE_ENV=production`)
- Stripe live mode (`STRIPE_SECRET_KEY=sk_live_…`, `STRIPE_WEBHOOK_SECRET=whsec_…` from production webhook)
- Anthropic production (`ANTHROPIC_API_KEY` + a monthly USD cap set on the Anthropic console; surface to operators via the planned admin endpoint)
- Twelve Data paid plan (`TWELVE_DATA_API_KEY`)
- MetalpriceAPI paid plan (`METALPRICEAPI_KEY`)
- NewsAPI Production tier (`MIZAN_NEWSAPI_KEY`)
- Supabase Pro (`SUPABASE_SERVICE_ROLE_KEY` already set; verify Pro tier active)
- Resend custom domain verified (`RESEND_API_KEY` + `RESEND_FROM`)

Plus 10 encryption keys generated locally and set as Fly secrets per
the `for slot in …` one-liner in `docs/LOCAL-DEV-SETUP.md` step 3.

### B. Code-signing certificates

- Apple Developer ID — $99/yr from Apple Developer Program
- Windows code-signing — Azure Trusted Signing (~$15/mo) or equivalent
- Linux GPG key — free; documented in the desktop release workflow

### C. Canary rollout — Gate 3

After A + B are provisioned:

1. Run `./scripts/pre-canary-check.sh --env=production` and resolve every FAIL
2. `flyctl deploy --strategy canary --canary-percent 5` against `mizan-connect`
3. Watch metrics + audit log for 7 days
4. Promote to 25% for another 7 days
5. Promote to 100% for 14 days
6. Definition of live: 14 days at 100%, zero Tier-1 alerts, p99 within
   the published budgets, sync > 95% across providers, Truth Ledger
   intact (no `BrokenChain` / `TamperedEntry` / `OutOfOrder` errors in
   the audit log), at least one real user paid Zakat through the live
   Stripe path

---

## Section 7 — Declaration

**Mizan is production-grade end-to-end for the code surfaces this sweep
covered.** The two real CLAUDE.md §0 violations from the start of the
sweep are gone. Coverage floors are CI-enforced and currently met on
every gated crate. Every state-mutating Connect handler writes to the
audit log. No real secrets are committed. The web mode degrades
gracefully on desktop-only features.

The remaining items in §4 are **real implementation work, honestly
scoped** — not paperwork. The OAuth + MCP handlers, per-user rate
limiting, advisor drill-down, Hanbali debt deduction, §23 fixture
seed, and the UI unit-test gaps each warrant a dedicated session. They
are not blockers for code-grade production readiness; they are
feature/scope work that lives on the post-launch roadmap.

The remaining items in §6 are **the user's**: paid API keys, code-
signing certificates, Gate 3 canary approval. None of these can be
resolved from inside the codebase.

Once §6 lands, the path to live is: `./scripts/release-gate.sh` green
→ `./scripts/pre-canary-check.sh --env=production` green →
`flyctl deploy --strategy canary --canary-percent 5` → 28-day rollout
gate per §6.C.

---

## Section 8 — How this document gets updated

When the §4 items ship, add a row to §1 and update the relevant §4
row. When the §6 items get provisioned, mark them in a follow-up
declaration v3 with the timestamp + the operator who provisioned each.

Do **not** restart the audit-doc cycle. The continuation prompts were
over-prescriptive in places (claiming items missing that were on
main) and under-prescriptive in others (the workflow gate bug in #184
took two follow-up PRs to diagnose and fix). The right cadence is:
read the code, ship one well-scoped PR per claimed gap, re-audit only
when the work is verifiably done.
