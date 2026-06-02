# Mizan Evolution — Master Plan (All 11 Tracks)

## Context

Mizan is a Rust-native, local-first, hash-chained personal wealth engine across three surfaces (Desktop / Connect / Badge). The user has handed two source-of-truth documents:

1. **`/Users/samisayyed/Downloads/CLAUDE.md`** — v1.0 working agreement (April 2026). 19 sections covering: 6 absolute rules, the three surfaces, hard rules, code conventions, subsystem-specific rules, testing standards, perf budgets, security boundaries, cache/versioning, DB discipline, AI agent dev rules, Mizan Badge rules, past bugs, workflow, the admin monitoring dashboard, docs requirements, anti-patterns, references, and the working agreement itself.
2. **`/Users/samisayyed/Downloads/Mizan_Evolution_Spec.md`** — the build spec (1,242 lines, 23 sections). Defines: dashboard rewrite, asset class expansion, AI-native depth, news module, badge expansion, Zakat coverage, net worth page, tiering, data model evolution, integration layer, security posture preservation, perf/hygiene/scalability, audit pass, cache/versioning hardening, OAuth framework, MCP capability, and the 11-track evolution order.

The goal: deliver the spec to the bar specified in CLAUDE.md, in 11 named tracks, with Track H (Code Hygiene & Audit Pass) acting as a blocking gate for the public availability of all other tracks.

This plan is the single source of truth for the planning phase. After approval, Phase 3 (execution) will materialise the per-track plans as `docs/plans/00-master-plan.md` ... `docs/plans/11-track-k.md` inside the repo (deferred to execution per plan-mode constraints — only this file may be written in plan mode).

---

## Critical Reality Checks Before Execution

These discrepancies between the spec's assumptions and the actual repo MUST be resolved or accepted as the plan opens:

1. **Repo root** is `/Users/samisayyed/Documents/mizan-ai-native/`. The current shell cwd `/Users/samisayyed/mizan-ai-native/` is a near-empty stub. Every command in execution targets the Documents path.
2. **Desktop crate path** is `mizan-4/crates/...`, not `crates/...` at repo root. Spec/CLAUDE.md text saying "crates/financial-truth" maps to `mizan-4/crates/financial-truth` once that crate exists.
3. **Missing crates referenced by the spec & CLAUDE.md** that do not exist as separate crates today:
   - `financial-truth` — likely lives inside `mizan-4/crates/core` or similar today
   - `zakat` — needs verification; likely in `core` or `ai`
   - `insights` — needs verification
   - `synthesis` — needs verification
   - `csv-import` — needs verification
   
   **Track H must include a sub-task: "Crate extraction audit"** — identify whether to extract these into their own crates (preferred per CLAUDE.md §5 hard floors of 95% coverage on `crates/financial-truth` and `crates/zakat`) or update CLAUDE.md/spec to reference their actual locations. **Recommend: extract them, because the 95%-coverage and mutation-testing rules require crate-level isolation to be enforceable in CI.**
4. **Missing directories**: `docs/adr/`, `docs/runbooks/`, `docs/plans/`, `docs/qa-passes/` do not exist. Track H creates them.
5. **Two CLAUDE.md files**: the repo's existing 6,255-byte one at `mizan-4/CLAUDE.md` (or root) is older than the v1.0 in Downloads. **Recommendation: adopt the Downloads CLAUDE.md as the new working agreement, archiving the prior one to `docs/adr/0001-adopt-claude-md-v1.md` with a diff and rationale.** Open question for user.
6. **Frontend path**: spec references `web/`; actual frontend is `mizan-4/apps/frontend/`.

These are surfaced as PR-1 of Track H ("Repo / Spec Reconciliation ADR") before any other execution.

---

## Track Ordering Rationale

**Hard dependency: Track H gates public release of every other track.** Per CLAUDE.md §0 and Spec §22, no track ships externally until the audit baseline is signed.

**Soft dependency map (what produces inputs another track consumes):**

```
Track E (Badge expansion)  ────┐
                                ├──→  Track B (Asset classes) — new badges render on new panels
Track I (Cache + versioning) ──┤
                                ├──→  Track A (Dashboard rewrite) — version-aware caches + new layouts
Track A (Dashboard) ───────────┤
                                ├──→  Track D (News strip placeholder filled by D)
Track C (AI-native depth) ─────┤
                                ├──→  Track F (Zakat coverage needs new agent tools + memory)
Track J (OAuth framework) ─────┤
                                └──→  Track K (MCP) — both extend Mizan Connect integration shape
Track G (Tiers) ───────────────────→  Track K (Gold gate on MCP, Enterprise on multi-seat)
```

**Recommended execution order (parallel where capacity allows):**

| Phase | Tracks running in parallel | Gating |
|---|---|---|
| Phase 1 | **H** (audit) + **I** (cache/versioning) + **E** (badge variants) | H, I, E unblock everything else |
| Phase 2 | **A** (dashboard) + **C** (AI depth foundation: tool registry, memory) | Need E for badge variants |
| Phase 3 | **B** (asset classes) + **D** (news) + **F** (Zakat) | Need A's panels and C's tools |
| Phase 4 | **G** (Enterprise/Advisor tiers) + **J** (OAuth framework) | Need C's memory & C's tool registry |
| Phase 5 | **K** (MCP) | Last — composes on top of every other surface |

Track H runs **continuously in audit mode** across all later phases — it gates the public-release valve, not the parallel execution.

---

## Estimated Total Scope

| Track | Sprints (per spec) | Refined estimate | Rationale for delta |
|---|---|---|---|
| A — Dashboard Rewrite | 1–2 | **2** | Recharts vocabulary purge + tracking-tight migration is wider than spec implies |
| B — Asset Class Expansion | 3–4 | **4** | 12 panels × universal pattern; brokerage UI exists, others need full build |
| C — AI-Native Depth | 4–6 | **6** | Tool registry expansion (15+ new tools) + memory + multi-modal + predictive |
| D — News Module | 2–3 | **2** | Mostly Mizan Connect work; UI is two-tab + reader |
| E — Mizan Badge Expansion | 1–2 | **1.5** | Tight scope; modifier system is additive |
| F — Zakat Engine Coverage | 2–3 | **3** | Maliki + Hanbali + 6 new asset class rules + Hawl tracking + Pay-Zakat flow |
| G — Enterprise + Advisor | 2–3 | **3** | SSO is the long pole; Advisor multi-client is also substantial |
| H — Code Hygiene & Audit | 1 (blocking) | **2** | Crate extraction + 11 audit checklists + signed report; understated in spec |
| I — Cache & Versioning | 1–2 | **2** | cache_policy registry + updater snapshot + self-test + rollback drill runbook |
| J — OAuth Framework | 2–3 | **2.5** | 8 initial providers, with Google Drive / Notion / Slack as priority three |
| K — MCP Capability | 3–4 | **4** | Gateway + sandbox gate + egress DLP + public catalog review process |

**Total: ~32 sprints** (~64 weeks at 2-week sprints, ~15 months) at single-stream pace. **With 2-stream parallelism: ~9 months. With 3-stream parallelism: ~7 months.** Track H consumes 2 sprints up front; the rest pipelines.

---

## Cross-Track Dependency Map

**Hard dependencies (cannot start later track without earlier):**
- E → B (new badge variants must exist before B's panels render them)
- C-foundation (tool registry + memory) → F (Zakat needs `find_sharia_status`, `compute_zakat` extensions, memory for school preference)
- C-foundation → J (OAuth) and K (MCP) — both extend the agent's tool surface
- A → B (panels live on the rewritten dashboard)
- A → D (news strip placeholder is in A; real news fills it in D)
- I → A (version-aware cache invalidation must be live before dashboard cache schema changes)
- G → K (Gold entitlement gate must exist before MCP gates on it)

**Soft dependencies (better if earlier, but not blocking):**
- H findings inform every later track's clean-up scope
- E modifier ordering should stabilize before D adds "Why this matters to you" badge stacks
- C predictive layer produces inputs that B's asset class panels display

---

## Risk Register — Top 10

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **Schema migration coordination across desktop SQLite + cloud Postgres** — new tables in both, drift could lock users on stale binaries | Critical | Every migration carries a manifest of cache evictions (Track I §19.2); `X-Mizan-Client-Version` header handlers in Connect (Track I); migrations are forward-only with crash-safe DDL |
| 2 | **AI cost runaway under new tool registry** — 15+ new tools, predictive Monte Carlo background jobs, MCP fan-out | Critical | Per-action credit metering in agent input bar; Sentry+Datadog cost-per-user alerts in monitoring dashboard (CLAUDE.md §15); model routing (small fast for intent, Sonnet for std, Opus for Gold complex); Anthropic prompt caching with >80% hit-rate target |
| 3 | **Zakat school edge cases produce incorrect numbers** — fiqh differences in PE proportionate share, ULIP surrender, locked retirement, debt deduction are subtle | Critical | 95% coverage floor + mutation testing (CLAUDE.md §5); two-reviewer rule on `crates/zakat`; golden tests per school × per asset-class combo; scholarly board sign-off on the rules ADR before code lands |
| 4 | **Sharia compliance accuracy** — wrong "halal-screened" badge could cause user to invest in non-compliant equity | Critical | `find_sharia_status` returns rated/unrated; `'mixed-compliance'` badge for ambiguous; AAOIFI screening criteria documented in ADR; user can override per `user_memory`; never auto-act on screening |
| 5 | **Stripe webhook outages corrupt subscription state** — past bug already cost a paid user their plan | High | Multi-secret rotation tests in place; DELETE-then-INSERT pattern (CLAUDE.md §5); admin force-grant endpoint exists; need: Sentry alert on webhook latency p99 spikes (in §15 dashboard already) |
| 6 | **Plaid / SnapTrade API changes break sync** | High | sync_run_ledger captures errors; provider-specific contract tests run nightly; webhook signature verification mandatory; per-provider quarterly key rotation runbook |
| 7 | **MCP user-provided servers exfiltrate sensitive data or manipulate agent reasoning** | High | Sandboxing rules absolute (Track K §21.3): MCP cannot write to financial state; egress DLP rejects payloads with SSN/PAN/Aadhaar/card patterns; per-call user confirmation for outbound; gateway logs every call; public catalog review process |
| 8 | **Crate extraction (financial-truth, zakat, etc.) during Track H breaks downstream consumers** | High | Extract behind feature flag where possible; semver-versioned cross-crate types via existing `mizan-types`-equivalent; per-extraction PR includes regression test pass on all dependent crates |
| 9 | **OAuth scope creep / annual re-consent worker fails silently** — user loses connections without notice | Medium | Background refresh worker has Sentry alerts on failure; re-consent triggers in-app notification 14 days before expiry; failed re-consent fires a `ReconnectionRequired` notification (extends §9.3 list) |
| 10 | **Performance budget regression from Recharts donut+bar reorg + new panels** — cold start, chart paint | Medium | Existing §A19 budgets gated at release; new panels measured against budget in CI on fixed-spec runner; chart paint specifically benchmarked for the 12 new panels; if any fails, the offending panel ships in a feature-flagged second PR |

---

## Approval Checkpoints

These are explicit hand-back-to-user moments during execution. No track proceeds past its checkpoint without sign-off.

| Checkpoint | When | What the user reviews |
|---|---|---|
| CP-0 | Before any code change | This plan, the open questions resolved, the Phase-0 baseline output |
| CP-H1 | After Track H crate-extraction PR | The new crate boundaries — does `financial-truth` / `zakat` etc. as separate crates feel right? |
| CP-H2 | After Track H audit report signed | The 11-section signed audit report; any blocker findings resolved |
| CP-E | After Track E badge variants live | Visual review on every asset class screen — every figure has provenance |
| CP-A | After Track A dashboard rewrite | UX walkthrough — net-worth strip, heatmap, asset class panels in fixed order |
| CP-C-foundation | After tool registry + memory ships | List of new tools + memory editor UI; permission to expand into predictive layer |
| CP-F-rules | After fiqh ADR signed | Maliki + Hanbali school rules ADR — scholarly sign-off before code lands |
| CP-K-sandbox | After MCP sandbox gate ships | Penetration testing of the read-mostly gate; user-provided malicious MCP server attempts |
| CP-Release | Before Track H "production-grade across the board" sign-off | Final audit re-run; all 10 risks have mitigations live |

---

## Open Questions Requiring User Input Before Execution

These should be resolved before Phase 3 starts (or are surfaced as Track H PR-1 ADRs):

1. **Which CLAUDE.md is authoritative?** — Adopt the Downloads v1.0 (April 2026) as the project working agreement, replacing the existing repo CLAUDE.md? **Recommended: yes**, with the prior one archived to `docs/adr/0001-adopt-claude-md-v1.md`.
2. **Crate extraction scope** — Extract `financial-truth`, `zakat`, `insights`, `synthesis`, `csv-import` into their own crates so the 95% coverage floor + mutation testing + two-reviewer rules are enforceable in CI? **Recommended: yes**, as Track H PR-2.
3. **Scholarly board engagement** — Do we have access to scholars who can sign off on the Maliki + Hanbali ADRs before code lands? If not, Track F slips until we do. **User decision needed.**
4. **MCP `trust_level` enum** — Schema-prepare for a future "trusted MCP" tier (i.e. one that could mutate non-financial state) now, or wait until product decides? **Recommended: prepare the schema column now (`trust_level enum default 'untrusted'`), but never wire any code path that respects "trusted" until a security review explicitly authorises**. This avoids a future migration and keeps the gate absolute.
5. **Track J initial three providers** — Recommend **Google Drive** (statement upload, high user value), **Notion** (goal tracking, low complexity, high reuse), **Slack** (alternative notification channel, Gold-tier polish). User can re-order.
6. **Repo path discrepancy** — Standardize on `mizan-4/` as the desktop crate root in all spec/CLAUDE.md text, or rename to `mizan-desktop/`? **Recommended: rename to `mizan-desktop/`** as a Track H housekeeping PR, since the spec's prose assumes that name and renames are cheap before the user count scales.
7. **News provider budget** — Refinitiv is institutional-grade-but-paid. Approve the line item now, or defer to Track D PR-late as a "future spend" toggle? **User decision.**
8. **Quarterly tech-debt sweep calendar** — Pick the four weeks (e.g. last week of Mar/Jun/Sep/Dec) so it's on the calendar from day 1. **User decision.**
9. **Phase 0 baseline runs** — Plan mode forbids me running `cargo test` / `cargo clippy` etc. now. After approval, the very first action in Phase 3 is the user-specified Phase 0 baseline confirmation. **Acknowledge.**

---

# PER-TRACK PLANS

Each plan below contains: scope (in/out), files to create, files to modify, migrations, tests, ADRs, perf-budget impact, security checklist, rollout strategy, PR sequence, definition of done, open questions.

---

## Track H — Code Hygiene & Audit Pass (Blocking Gate)

**Scope IN:** crate extraction (5 candidate crates), the 11-section audit per CLAUDE.md §18.12 / Spec §18.12, `docs/adr/` + `docs/runbooks/` + `docs/plans/` + `docs/qa-passes/` directory bootstrap, repo-path rename (`mizan-4/` → `mizan-desktop/` if approved), CLAUDE.md adoption ADR.
**Scope OUT:** new features (those are Tracks A–G, I–K).

**Files to create:**
- `docs/adr/0001-adopt-claude-md-v1.md` — adoption rationale + diff against prior
- `docs/adr/0002-extract-financial-truth-crate.md`
- `docs/adr/0003-extract-zakat-crate.md`
- `docs/adr/0004-extract-insights-crate.md`
- `docs/adr/0005-extract-synthesis-crate.md`
- `docs/adr/0006-extract-csv-import-crate.md`
- `docs/adr/0007-repo-rename-mizan-desktop.md` (conditional on CP-0)
- `docs/runbooks/deploy.md` — codifies the `--no-cache` lesson from v37
- `docs/runbooks/updater-key-rotation.md`
- `docs/runbooks/incident-response.md`
- `docs/runbooks/gdpr-export.md`
- `docs/runbooks/key-rotation-quarterly.md`
- `docs/qa-passes/QA-P19-truth-ledger-ai-writes.md` (template for the next QA passes Track C will add)
- `docs/audit/2026-Q3-baseline-audit-report.md` — the signed audit output

**Files to modify:**
- Top-level `CLAUDE.md` — replace with v1.0 from Downloads
- `mizan-4/Cargo.toml` — workspace member additions for new crates
- `.github/workflows/*` — add `cargo udeps`, `cargo machete`, `gitleaks`, `trufflehog`, `knip`, `ts-prune` jobs (CLAUDE.md §18.1)
- `mizan-4/scripts/release-gate.sh` (or equivalent) — add the audit-baseline checks
- `mizan-4/apps/frontend/package.json` — add knip + ts-prune dev deps

**Migrations required:** none (this track is hygiene-only; no schema changes).

**Tests to add:**
- Per extracted crate: round-trip integration test proving the new crate boundary preserves existing behavior (run the same fixture inputs through old-location and new-crate, assert identical outputs)
- CI lint rule: `cargo udeps` returns zero; `cargo machete` returns zero; `knip` returns zero; `ts-prune` returns zero
- Secret scan job: gitleaks + trufflehog over full history, gate at zero findings
- Audit baseline checklist as 11 individual CI jobs so a regression of any one section fails the build independently

**ADRs:** 7 listed above. Each follows the standard format (context / decision / consequences / alternatives).

**Performance budget impact:** No new feature work. The crate extraction MAY shift compile-time perf (workspace builds get marginally slower with more crates); measured, accepted if < 15% wall-time hit on `cargo check --workspace`.

**Security review checklist:**
- [ ] Secret scan green over full history
- [ ] No `unwrap()` / `expect()` in write paths
- [ ] No silent FX fallbacks (`?? 1.0`, `unwrap_or(1.0)`, etc.) — `clippy::disallowed_methods` rule added
- [ ] No `f64` in money paths — CI lint
- [ ] All admin bearer compares use `subtle::ConstantTimeEq`
- [ ] HS256 production rejection test still gated in CI
- [ ] Truth Ledger chain integrity test still gated in CI
- [ ] Webhook signature verification mandatory at every webhook endpoint
- [ ] Token-plaintext scan green (QA-P1.2 stays green)

**Rollout strategy:** Track H is internal. No user-visible changes. Each PR merges to main once review passes. The signed audit report is the gate for all subsequent **public-release** PRs across Tracks A–G, I–K.

**PR sequence (each < 500 lines):**
1. PR-H1: ADR 0001 — adopt CLAUDE.md v1.0. Updates `CLAUDE.md` at repo root + writes ADR.
2. PR-H2: Repo rename ADR 0007 + the rename itself if approved.
3. PR-H3: Crate extraction ADRs 0002–0006 + per-crate scaffolding PRs (one PR per crate extraction — so this is really 5 sub-PRs).
4. PR-H4: `docs/runbooks/` bootstrap (deploy, updater-key-rotation, incident-response, gdpr-export, key-rotation-quarterly).
5. PR-H5: `docs/qa-passes/` bootstrap with QA-P19 template + index.
6. PR-H6: CI: add `cargo udeps`, `cargo machete`, `gitleaks`, `trufflehog`, `knip`, `ts-prune` jobs.
7. PR-H7: CI: add `cargo mutants` nightly job pointed at `financial-truth`, `zakat`, `ai/dispatcher`, `insights`, `synthesis` (after extraction).
8. PR-H8: CI: add `clippy::disallowed_methods` for FX silent-fallback lint + `f64`-in-money-paths lint.
9. PR-H9: Run the 11-section audit (dep tree / secret scan / dead code / dead file / schema / query plan / index coverage / cache table / API surface / Tauri command / AI tool). Write the signed audit report. Findings classified blocker / major / minor / informational.
10. PR-H10..N: One PR per blocker finding to resolution.
11. PR-Hfinal: Signed audit report merged. Track H closed. Public-release valve opens.

**Definition of done:**
- 7 ADRs merged
- 5 crates extracted with their consumers migrated
- 5 runbooks live
- 6 CI gates green
- Audit report signed (zero blockers, zero majors, minors filed as tracked issues with owners)
- Coverage thresholds met or exceeded on extracted crates (95% on financial-truth, zakat, ai/dispatcher, billing, auth, webhooks)

**Open questions:**
- Do we rename `mizan-4/` → `mizan-desktop/` (CP-0 Q6)?
- Where do the audit findings live during the resolution window — single mega-issue or per-finding issue? Recommend per-finding tracked issues with `audit-finding` label.

---

## Track I — Cache Invalidation & Versioning Hardening

**Scope IN:** `cache_policy.rs` registry, app-version-mismatch eviction worker, Tauri updater pre-update snapshot + post-install self-test + auto-rollback, `X-Mizan-Client-Version` negotiation, IPC schema versioning crate, Vite hash verification, quarterly rollback drill runbook, Supabase Postgres lifecycle hygiene checklist.
**Scope OUT:** new features in any other track; cache policy decisions for tables that don't exist yet (those land with their owning track).

**Files to create:**
- `mizan-desktop/crates/storage-sqlite/src/cache_policy.rs` — single source of truth for cache TTLs + eviction policies
- `mizan-desktop/crates/storage-sqlite/src/cache_eviction.rs` — synchronous eviction worker run on version-mismatch boot
- `mizan-desktop/crates/ipc-schema/` — new shared crate for versioned Tauri command request/response types (Rust + TS bindings)
- `mizan-desktop/apps/tauri/src/updater_snapshot.rs` — pre-update DB snapshot + post-install self-test
- `docs/runbooks/rollback-drill.md` — quarterly drill procedure
- `docs/runbooks/supabase-lifecycle.md` — slow-query review / index audit / bloat monitoring checklist
- `docs/api-versioning.md` — deprecation calendar
- `mizan-connect/src/middleware/client_version.rs` — `X-Mizan-Client-Version` header handler

**Files to modify:**
- `mizan-desktop/apps/tauri/src/main.rs` — wire cache eviction worker into startup before WebView paint
- `mizan-desktop/apps/tauri/src/updater.rs` — add pre-update snapshot call + post-install self-test invocation; keep existing `cfg(debug_assertions)` short-circuit
- `mizan-connect/src/server.rs` — register `client_version` middleware
- `mizan-desktop/apps/frontend/vite.config.ts` — confirm content-hashed asset filenames (already in place; add CI verification)

**Migrations required:**
- `mizan-desktop/migrations/NNNN_app_version_row.sql` — add `app_version` row to a settings table if not already present
- All future migrations gain a `-- caches-evicted: [list]` manifest comment that CI lints

**Tests to add:**
- Unit: `cache_policy` registry has an entry for every cache table (CI lint walks the schema + the registry)
- Integration: cache eviction worker correctness — fixture DB at vN, binary at vN+1, worker evicts correctly
- Integration: pre-update snapshot exists at `mizan.db.pre-{old_version}` after a simulated update
- Integration: post-install self-test runs (schema match + crypto round-trip + Twelve Data heartbeat + Connect heartbeat + Truth Ledger chain head)
- Integration: auto-rollback on self-test failure restores the pre-update DB
- Integration: `X-Mizan-Client-Version` header branches a handler appropriately
- Mutation: cache_policy parsing logic + eviction worker

**ADRs:**
- `docs/adr/0008-cache-policy-single-source-of-truth.md`
- `docs/adr/0009-updater-snapshot-and-rollback-design.md`
- `docs/adr/0010-ipc-schema-versioning.md`

**Performance budget impact:**
- Eviction worker adds < 50ms to cold start on the reference machine (budget: cold start < 1.2s — eviction is 50/1200 = ~4% of budget). Measured in CI.
- Self-test adds time only on first-launch after update; not in the steady-state cold-start budget.

**Security review checklist:**
- [ ] Pre-update snapshot retention (30 days) doesn't leak into a directory accessible to unprivileged processes
- [ ] Self-test failures fail closed (rollback initiated) not fail open
- [ ] `X-Mizan-Client-Version` header isn't trusted for auth (still verified-from-JWT) — just informational
- [ ] Updater signature still verified against bundled production public key

**Rollout strategy:**
- Track I has no user-visible feature changes, only safety mechanisms. Each PR ships to internal first, then beta channel (Gold+ opt-in), then stable.
- The first rollback drill runs in staging within 2 weeks of Track I closure.

**PR sequence:**
1. PR-I1: `cache_policy.rs` registry + CI lint requiring entry per cache table
2. PR-I2: `cache_eviction.rs` worker + wire into Tauri startup
3. PR-I3: `ipc-schema` shared crate skeleton + first 2 command migrations as proof
4. PR-I4: Updater pre-update snapshot + retention
5. PR-I5: Updater post-install self-test
6. PR-I6: Updater auto-rollback on self-test failure
7. PR-I7: `X-Mizan-Client-Version` middleware on Mizan Connect
8. PR-I8: Vite content-hash bundle verification on WebView load (mismatch → wipe and reload)
9. PR-I9: `docs/runbooks/rollback-drill.md` + first drill scheduled
10. PR-I10: `docs/runbooks/supabase-lifecycle.md` + first slow-query review scheduled

**Definition of done:**
- Every cache table has a `cache_policy.rs` entry
- CI lint rejects new cache tables without policy registration
- Updater pre-update snapshot ships and is verified to restore on failed self-test
- One full rollback drill completed end-to-end in staging
- One full Supabase lifecycle review completed (slow-query log review, index audit, bloat monitor run)

**Open questions:**
- Does the Tauri updater's signed manifest infrastructure already exist? Confirm or build in PR-I4.
- Sentry error-rate threshold for canary auto-rollback (Spec §19.8) — what's the exact threshold? Recommend 2× rolling-24h average sustained for > 15 min.

---

## Track E — Mizan Badge Expansion

**Scope IN:** 10 new origin variants + 8 new modifier badges (Spec §8), badge ordering rules, hover popover content, AAOIFI screening worker on Mizan Connect, `holdings.sharia_status` column + `last_screened_at`.
**Scope OUT:** the new sync providers themselves (those land in Tracks B & J & K); the audit-trail "click to verify Truth Ledger hash" UI (this part requires the Truth Ledger explorer, which is its own sub-feature in Track A's Net Worth page).

**Files to create:**
- `mizan-desktop/apps/frontend/src/components/badge/origin.tsx` — origin variant rendering
- `mizan-desktop/apps/frontend/src/components/badge/modifier.tsx` — modifier variant rendering
- `mizan-desktop/apps/frontend/src/components/badge/popover.tsx` — hover popover renderer (per-modifier content delegated)
- `mizan-desktop/apps/frontend/src/components/badge/popover-renderers/*.tsx` — one file per modifier (8 files)
- `mizan-connect/src/sharia/mod.rs` — AAOIFI screening worker
- `mizan-connect/src/sharia/aaoifi_rules.rs` — debt ratio / business activity / interest income screen
- `mizan-connect/src/sharia/handlers.rs` — `GET /v1/sharia/status/:symbol` endpoint

**Files to modify:**
- `mizan-desktop/packages/ui/src/components/ui/badge.tsx` — extend with `modifiers[]` prop
- `mizan-desktop/crates/core/src/holdings/model.rs` (or wherever holdings model lives) — add `sharia_status`, `last_screened_at`, `ai_estimated`, `ai_confidence`, `ai_value_range_low`, `ai_value_range_high`, `tags` columns to model + serde
- Every existing component that renders an account/holding (search for `<Badge` usages and the holdings list components) — pass new modifiers

**Migrations required:**
- Desktop: `mizan-desktop/migrations/NNNN_holdings_sharia_status.sql` — add columns above
- Desktop: `mizan-desktop/migrations/NNNN_extend_origin_enum.sql` — extend `sync_provider` enum to include `'setu'`, `'sgfindex'`, `'tink'`, `'basiq'`, `'lean'`, `'ccxt'`, `'chain_reader'`, `'twelve_data'`, `'metalprice_api'`, `'bondevalue'`
- Cloud: parallel migrations under `mizan-connect/migrations/`
- Each migration carries the cache-eviction manifest comment

**Tests to add:**
- Unit: badge primitive renders each origin variant
- Unit: badge primitive renders modifier stack in correct severity order: `'stale'` > `'pending-reconciliation'` > `'ai-estimated'` > compliance badges > `'audit-trail'` > `'agent-modified'`
- Integration: AAOIFI screening endpoint returns expected verdicts for fixture symbols (AAPL → unrated, SPUS → compliant, traditional banking stocks → non_compliant)
- Visual regression: Playwright captures the badge variants on a fixture page
- E2E: tap on `'ai-estimated'` modifier shows confidence interval popover

**ADRs:**
- `docs/adr/0011-badge-modifier-severity-ordering.md`
- `docs/adr/0012-aaoifi-screening-criteria.md` — explicit debt ratio threshold (33%), business activity blacklist, interest income threshold (5%); reviewed annually for AAOIFI standard updates

**Performance budget impact:**
- Badge popover hover delay budget: < 50ms (Spec §17.1). Measured per modifier renderer.
- AAOIFI screening worker runs on Mizan Connect, async; no desktop perf impact.

**Security review checklist:**
- [ ] `sharia_status` write path is single-source (the screening worker on cloud) — never client-writable
- [ ] AAOIFI screening result writes go through Truth Ledger if they change a holding's compliance state
- [ ] Hover popover content sanitised (no XSS from issuer names etc.)

**Rollout strategy:**
- New origin variants land first (low risk — enum extension)
- Modifier badges land per-modifier, gated by individual feature flags so QA can verify per-modifier rendering
- AAOIFI screening rolls out to internal first, then 5% of Gold users, then 100%

**PR sequence:**
1. PR-E1: Migration for new origin enum variants + sharia_status column
2. PR-E2: Badge primitive `modifiers[]` prop extension + severity ordering
3. PR-E3: Per-modifier popover renderers (8 PRs sub-batched if too large; otherwise one combined PR around ~400 lines)
4. PR-E4: AAOIFI screening worker + endpoint
5. PR-E5: `find_sharia_status` agent tool (uses Track E's endpoint; this is a Track C dependency but lives here for cohesion)
6. PR-E6: Wire `'halal-screened'` / `'mixed-compliance'` badges into existing holdings list views
7. PR-E7: Wire `'stale'` / `'pending-reconciliation'` / `'audit-trail'` / `'agent-modified'` / `'advisor-reviewed'` (placeholder until G) badges
8. PR-E8: Wire `'ai-estimated'` badge (placeholder until B ships the AI estimation pipelines for real estate / collectibles)

**Definition of done:**
- All 10 origin + 8 modifier variants render correctly with semantic-token colors for dark/light parity
- Severity ordering is enforced
- Hover popover content per modifier is implemented
- AAOIFI screening endpoint live with golden-test fixtures
- Visual regression suite green
- Documentation in `docs/components/mizan-badge.md`

**Open questions:**
- Should `'mcp'` modifier badge land here or in Track K? Recommend Track K — keeps Track E focused.

---

## Track A — Dashboard Rewrite

**Scope IN:** Remove separate Portfolio surface, restructure dashboard per Spec §3 (AI command bar pinned, net worth strip, heatmap, news strip placeholder, 12 asset class panels in fixed order, Today's Signal, quick action pull-up sheet), implement donut + bar charting vocabulary (Spec §4) across all visualizations, rename "break down" → "Net Worth" everywhere, polish notification panel per Spec §9.
**Scope OUT:** the asset class panel contents themselves (those land in Track B); real news (Track D); new badge variants (Track E — must land first); Sankey on Net Worth page (a Net Worth detail sub-task — can ship in Track A end or slip).

**Files to create:**
- `mizan-desktop/apps/frontend/src/pages/dashboard/index.tsx` — new dashboard composition (or modify existing)
- `mizan-desktop/apps/frontend/src/components/dashboard/ai-command-bar.tsx`
- `mizan-desktop/apps/frontend/src/components/dashboard/net-worth-strip.tsx`
- `mizan-desktop/apps/frontend/src/components/dashboard/news-strip-placeholder.tsx`
- `mizan-desktop/apps/frontend/src/components/dashboard/today-signal-card.tsx`
- `mizan-desktop/apps/frontend/src/components/dashboard/quick-action-sheet.tsx`
- `mizan-desktop/apps/frontend/src/components/dashboard/asset-class-panel.tsx` — universal panel skeleton (panel contents populated in Track B)
- `mizan-desktop/apps/frontend/src/pages/net-worth/index.tsx` — new Net Worth page (renamed from Break Down)
- `mizan-desktop/apps/frontend/src/components/charts/donut.tsx` — shared world-class donut primitive
- `mizan-desktop/apps/frontend/src/components/charts/bar.tsx` — shared bar primitive
- `mizan-desktop/apps/frontend/src/components/charts/heatmap.tsx` — extend existing
- `mizan-desktop/apps/frontend/src/components/notifications/panel.tsx` — extended notification panel per §9.1

**Files to modify:**
- Anything referencing "Portfolio" surface — delete the route, update sidebar
- Every reference to "break down" / "Break Down" / "breakdown" across the codebase (`grep -ri 'break[ -]?down'`)
- Existing Recharts usages: replace pie chart usage with donut; remove radar/polar/3D if any exist
- `mizan-desktop/apps/frontend/src/components/notifications/*` — apply alignment, scrolling, day-bucket grouping

**Migrations required:** none (this is UI restructuring on existing data). Possibly add an `asset_class_panel_order` setting if user-reordering is in scope later — defer.

**Tests to add:**
- Playwright: dashboard top-to-bottom — AI command bar present + pinned, net-worth strip with toggleable deltas, heatmap tiles tap-navigates, news strip placeholder, 12 panels in fixed order, Today's Signal card, quick action sheet opens
- Visual regression: every chart type rendered against fixture data
- Unit: each new component
- Performance: cold-start budget maintained, chart paint < 200ms for donut+bar
- Migration: `break-down` text scan returns zero on every locale file

**ADRs:**
- `docs/adr/0013-dashboard-information-architecture.md`
- `docs/adr/0014-charting-vocabulary-donut-bar-heatmap-sparkline-sankey.md`

**Performance budget impact:**
- Cold start: < 1.2s preserved (the new dashboard composition is simpler than the prior; measured)
- Chart first-paint: < 200ms (donut animation completes < 300ms cached, < 1s fetched)
- Notification panel open: < 100ms

**Security review checklist:**
- [ ] AI command bar input sanitised before sending to agent
- [ ] No new IPC surfaces introduced without versioned schema (Track I dependency)

**Rollout strategy:**
- Behind a feature flag `dashboard_v2`
- Internal team first (1 week), then beta opt-in (1 week), then 25%/50%/100%
- Auto-rollback to v1 dashboard on Sentry error spike (Spec §19.8 canary policy)

**PR sequence:**
1. PR-A1: Rename "break down" → "Net Worth" everywhere (mechanical rename PR; small but touches many files)
2. PR-A2: Net Worth page skeleton (replaces Break Down route)
3. PR-A3: Shared chart primitives — donut, bar, heatmap extension, sparkline (foundation for all later tracks)
4. PR-A4: Remove pie/radar/polar/3D chart usages, replace with donut/bar
5. PR-A5: AI command bar component + pinned top
6. PR-A6: Net worth strip with toggleable deltas + sparkline
7. PR-A7: Heatmap tile-tap → asset detail
8. PR-A8: News strip placeholder (real wiring in Track D)
9. PR-A9: Asset class panel skeleton + 12 panels in fixed order (each panel reads from existing repository; contents per Track B)
10. PR-A10: Today's Signal card (reads from existing insights engine; deduped against last 7d)
11. PR-A11: Quick action pull-up sheet
12. PR-A12: Notification panel polish — alignment + scroll + day buckets + filter chips + sticky header + swipe actions
13. PR-A13: Remove separate Portfolio surface; sidebar update; route cleanup
14. PR-A14: Feature flag rollout sequence

**Definition of done:**
- Reference user (Singapore Sharia-aware) opens the app: sees the new dashboard. Says "what's my net worth this week" — gets answered. Taps each panel — opens to its asset class screen. Taps notification bell — sees the polished panel. Word "break down" appears nowhere in the UI or codebase.
- All performance budgets met or improved
- Visual regression suite captures every chart type
- Sentry post-rollout shows error rate ≤ pre-rollout

**Open questions:**
- Is there a route currently named "Portfolio" that needs deprecation messaging? If users have bookmarks, redirect with a brief "Portfolio is now the dashboard" notice.

---

## Track C — AI-Native Depth

**Scope IN (foundation, ships first):** tool registry expansion (Spec §7.1, 15+ new tools), `user_memory` table + vector store + memory writer subroutine, conversational mutation depth, the "App without AI" contract banner.
**Scope IN (later, after foundation):** multi-modal input (voice / image / document / screenshot), predictive layer (Monte Carlo, cash flow forecast, retirement projection, goal tracking), offline robustness with embedded local model, cost discipline instrumentation.
**Scope OUT:** Sharia screening (lives in Track E); Zakat extensions (Track F); news synthesis (Track D); MCP (Track K).

**Files to create:**
- `mizan-desktop/crates/ai/src/tools/create_holding.rs` (and update / delete companions)
- `mizan-desktop/crates/ai/src/tools/add_activity.rs` / `list_activities.rs`
- `mizan-desktop/crates/ai/src/tools/compute_net_worth.rs`
- `mizan-desktop/crates/ai/src/tools/run_scenario.rs` / `compare_scenarios.rs`
- `mizan-desktop/crates/ai/src/tools/get_market_data.rs`
- `mizan-desktop/crates/ai/src/tools/get_fx_rate.rs` — refuses to invent rates
- `mizan-desktop/crates/ai/src/tools/sync_account.rs`
- `mizan-desktop/crates/ai/src/tools/generate_report.rs`
- `mizan-desktop/crates/ai/src/tools/set_reminder.rs` / `set_alert.rs`
- `mizan-desktop/crates/ai/src/tools/get_news.rs` (reads from Track D's table)
- `mizan-desktop/crates/ai/src/tools/summarize_document.rs`
- `mizan-desktop/crates/ai/src/tools/get_user_memory.rs` / `update_user_memory.rs`
- `mizan-desktop/crates/ai/src/tools/get_holding_history.rs`
- `mizan-desktop/crates/ai/src/tools/bond_analytics.rs`
- `mizan-desktop/crates/ai/src/tools/estimate_price.rs`
- `mizan-desktop/crates/ai/src/memory/writer.rs` — the only path writing to `user_memory`
- `mizan-desktop/crates/ai/src/memory/reader.rs`
- `mizan-desktop/crates/ai/src/memory/embeddings.rs` — sqlite-vec on desktop, pgvector mirror on cloud
- `mizan-desktop/crates/ai/src/predictive/monte_carlo.rs`
- `mizan-desktop/crates/ai/src/predictive/cash_flow.rs`
- `mizan-desktop/crates/ai/src/predictive/retirement.rs`
- `mizan-desktop/crates/ai/src/multimodal/voice.rs`
- `mizan-desktop/crates/ai/src/multimodal/image.rs`
- `mizan-desktop/crates/ai/src/multimodal/document.rs`
- `mizan-desktop/apps/frontend/src/components/agent/memory-editor.tsx` — user-visible memory CRUD
- `mizan-desktop/apps/frontend/src/components/agent/offline-banner.tsx`
- `mizan-desktop/apps/frontend/src/components/agent/credit-meter.tsx`

**Files to modify:**
- `mizan-desktop/crates/ai/src/dispatcher.rs` — register every new tool with its AI Safety Runtime properties (per-turn cap weight, audit scope, numeric bounds, Truth Ledger flag); compile-time check rejects tools missing any property
- `mizan-desktop/crates/ai/src/prompts/system.md` — updated prompt template; bump template version hash
- `mizan-desktop/crates/ai/src/prompts/CHANGELOG.md` — append each prompt bump
- `mizan-desktop/apps/frontend/src/lib/ai/tool-types.ts` — TS bindings per new tool

**Migrations required:**
- Desktop: `mizan-desktop/migrations/NNNN_user_memory.sql` — `id`, `user_id`, `fact_text`, `embedding` (sqlite-vec), `category`, `confidence`, `source`, `created_at`, `last_used_at`, `expires_at`
- Desktop: `mizan-desktop/migrations/NNNN_projection_snapshots.sql`
- Desktop: `mizan-desktop/migrations/NNNN_reconciliation_queue.sql`
- Desktop: `mizan-desktop/migrations/NNNN_agent_audit_log.sql`
- Cloud (for Gold+ cross-device): mirror `user_memory` with pgvector
- Cache eviction manifests on each

**Tests to add:**
- Per new tool: happy-path unit test + AI Safety Runtime compliance test (per-turn cap respected, audit log written, numeric bounds enforced, Truth Ledger entry emitted where applicable)
- Memory writer: idempotency, no implicit writes from tool handlers
- Embeddings: round-trip on sqlite-vec; pgvector parity on cloud
- Predictive: golden-test Monte Carlo with deterministic seed
- Multi-modal: each modality has at least one integration test
- E2E: conversational mutation of every example in Spec §7.4 (rename account, sell holding, recurring transfer, update value, surrender analysis, scenario run)
- E2E: offline banner appears when agent service unreachable

**ADRs:**
- `docs/adr/0015-ai-tool-registry-expansion.md` — the 15+ new tools, their safety properties
- `docs/adr/0016-user-memory-layer.md` — memory writer discipline, GDPR rectification surface
- `docs/adr/0017-predictive-layer-monte-carlo.md`
- `docs/adr/0018-multi-modal-input.md`
- `docs/adr/0019-offline-robustness-embedded-model.md` — which embedded model (Llama variant?), size, capabilities
- `docs/adr/0020-ai-cost-discipline.md` — model routing rules, credit metering

**Performance budget impact:**
- Agent round-trip: < 500ms intent classification, < 2s read tools, < 5s write tools (CLAUDE.md §7)
- Memory store growth tracked per user; alerted when > $X cost trajectory
- Anthropic prompt cache hit rate ≥ 80% (CLAUDE.md §15.6)

**Security review checklist:**
- [ ] Every new tool has all four AI Safety Runtime properties at registration (compile-time fail otherwise)
- [ ] `user_memory` writes are GDPR-rectifiable via the editor UI
- [ ] Cloud-mirrored memory encrypted at rest (Mizan Connect AES-GCM-256 envelope)
- [ ] Truth Ledger entry emitted by every financial-mutating tool
- [ ] Numeric bounds prevent rounding drift (Decimal everywhere, never f64)
- [ ] Adversarial-prompt tests for the "no financial advice" guardrail (CLAUDE.md §16.3)
- [ ] Multi-modal inputs sanitised; image OCR results validated before ingestion
- [ ] Voice transcription doesn't leak audio to disk on desktop (existing `tracing` redaction)
- [ ] Embedded local model verified offline-only (no telemetry calls)

**Rollout strategy:**
- Foundation PRs (tool registry + memory) ship to all users behind `ai_v2` flag; internal first, then 25%/100%
- Predictive layer: Gold-tier only initially; expand if cost permits
- Multi-modal: Silver gets voice + image; Gold gets document + screenshot
- Offline embedded model: opt-in setting; default off until stability is proven

**PR sequence (foundation):**
1. PR-C1: `user_memory` migration + crate scaffolding (memory writer + reader + embeddings on sqlite-vec)
2. PR-C2: Memory editor UI (settings panel) — user can view, edit, delete every fact
3. PR-C3: Cloud mirror of memory for Gold+ — Postgres pgvector schema + sync worker
4. PR-C4: Tool registry: `create_holding`, `update_holding`, `delete_holding`
5. PR-C5: Tool registry: `add_activity`, `list_activities`
6. PR-C6: Tool registry: `compute_net_worth`, `get_holding_history`
7. PR-C7: Tool registry: `get_market_data`, `get_fx_rate` (refuses to invent rates)
8. PR-C8: Tool registry: `sync_account`, `generate_report`
9. PR-C9: Tool registry: `set_reminder`, `set_alert`
10. PR-C10: Tool registry: `get_news` (stub — Track D fills the table)
11. PR-C11: Tool registry: `summarize_document` (PDF + CSV layout-aware parsing)
12. PR-C12: Tool registry: `bond_analytics`, `estimate_price`
13. PR-C13: Tool registry: `run_scenario`, `compare_scenarios`
14. PR-C14: System prompt update + Anthropic prompt cache invalidation hook

**PR sequence (later):**
15. PR-C15: Predictive layer — Monte Carlo net worth trajectory + `projection_snapshots` table
16. PR-C16: Predictive layer — cash flow forecast
17. PR-C17: Predictive layer — retirement projection + goal tracking dashboard chips
18. PR-C18: Multi-modal — voice input (Whisper local + cloud STT for Gold+)
19. PR-C19: Multi-modal — image OCR (bank statement photo)
20. PR-C20: Multi-modal — document upload (PDF / XLSX) routed via `summarize_document`
21. PR-C21: Multi-modal — screenshot paste
22. PR-C22: Offline-robustness — embedded local model integration in `crates/ai`
23. PR-C23: Cost discipline — per-action credit metering UI + cost-per-user dashboard hooks

**Definition of done:**
- All 15+ new tools registered, tested, dispatched, ledgered
- `user_memory` editor surface live, every example fact from Spec §7.3 saveable
- Predictive layer runs background jobs in Tauri sidecar; outputs cached in `projection_snapshots`
- 4 modalities (voice / image / document / screenshot) usable end-to-end
- Offline banner appears correctly on agent-service unreachable
- Credit meter visible in agent input bar
- Anthropic prompt cache hit rate measured ≥ 80%

**Open questions:**
- Which embedded model fits the offline-robustness ADR? Recommend a quantized Llama 3.1 8B or Qwen2.5 7B variant — measured cold-start impact and disk footprint before committing.
- Memory expiry policy: default to never-expire, or 12-month TTL on `'agent-inferred'` source? Recommend 12-month TTL on `'agent-inferred'`, never-expire on `'user-stated'`.

---

## Track B — Asset Class Expansion

**Scope IN:** 12 panels per Spec §5 (Equities, Brokerage Accounts, Bank/Cash, Bonds & Sukuks, Provident Funds, Insurance, Private Equity, Real Estate, Crypto, Commodities, Collectibles, Forex), each following Spec §6 universal pattern. New providers added in Mizan Connect: Setu (India), SGFinDex (SG), Tink (EU), Basiq (AU), Lean (UAE), CCXT (crypto), chain readers (Etherscan/BscScan/Solscan/Blockchair). AI estimation pipelines for real estate + collectibles.
**Scope OUT:** dashboard composition (Track A); the badges themselves (Track E); the AI agent tools (Track C — `bond_analytics`, `estimate_price`); news (Track D); Zakat behavior of each class (Track F).

**Files to create:**
- `mizan-desktop/apps/frontend/src/pages/asset-classes/equities.tsx` (and 11 more — one per class)
- `mizan-desktop/apps/frontend/src/components/asset-classes/*` — per-class row schemas
- `mizan-connect/src/sync/setu/{mod,client,handlers,webhook}.rs`
- `mizan-connect/src/sync/sgfindex/...`
- `mizan-connect/src/sync/tink/...`
- `mizan-connect/src/sync/basiq/...`
- `mizan-connect/src/sync/lean/...`
- `mizan-connect/src/sync/crypto/{mod,ccxt_client,handlers,webhook}.rs`
- `mizan-connect/src/sync/chain_reader/{mod,etherscan,bscscan,solscan,blockchair}.rs`
- `mizan-desktop/crates/ai/src/estimation/real_estate.rs` — PropertyGuru / Magicbricks / Zillow / DLD comparable lookups
- `mizan-desktop/crates/ai/src/estimation/collectibles.rs` — Chrono24 / WatchCharts / Hagerty / StockX comparables
- ETF look-through worker for Sharia ETF purification amount on dividends (lives in `crates/ai/src/sharia/`)

**Files to modify:**
- `mizan-desktop/crates/storage-sqlite/src/holdings/...` — extend row schemas per class
- `mizan-desktop/crates/storage-sqlite/src/sync_provider.rs` — enum extension

**Migrations required:**
- Per new provider: encrypted token tables in cloud
- Provider event tables for idempotent webhook handling
- Each migration carries cache-eviction manifest

**Tests to add:**
- Per provider: webhook signature verification (positive + negative — the 5-case Stripe rotation pattern)
- Per provider: idempotency by event ID — lookup-or-insert proven
- Per provider: token encryption round-trip
- Per provider: 401 path smoke test
- Per AI-estimation pipeline: golden tests with fixture comparables → expected confidence interval
- E2E per panel: connect provider → see holdings → tap row → see detail → edit value → see Truth Ledger entry

**ADRs:**
- `docs/adr/0021-setu-aa-integration.md`
- `docs/adr/0022-sgfindex-singpass-redirect-uri-required.md`
- `docs/adr/0023-tink-pst2.md`
- `docs/adr/0024-basiq-cdr.md`
- `docs/adr/0025-lean-uae.md`
- `docs/adr/0026-ccxt-crypto-exchanges-read-only-scope-enforcement.md`
- `docs/adr/0027-chain-reader-public-address-only.md`
- `docs/adr/0028-ai-estimation-pipeline-real-estate-and-collectibles.md`
- `docs/adr/0029-etf-look-through-purification.md`

**Performance budget impact:**
- Each panel must paint < 200ms cached
- Donut/bar animations < 300ms cached
- Sync runs measured against budget; long-running syncs run in Tauri sidecar (no main process block)

**Security review checklist (per provider):**
- [ ] Tokens encrypted at rest with provider-specific encryption key, AES-GCM-256
- [ ] Webhook signature verification mandatory
- [ ] Idempotency by provider event ID, lookup-or-insert
- [ ] `redirect_uri` Option-typed (SGFinDex requires it; others optional)
- [ ] CCXT scope enforcement: `withdraw` / `trade` scopes rejected at validation
- [ ] Chain reader: public address only — no private keys, no seed phrases (CLAUDE.md §8 bright line)
- [ ] AI estimation never auto-writes a holding's value — always surfaces a range with confidence, user confirms

**Rollout strategy:**
- Per-provider feature flags in Mizan Connect
- Per-panel feature flags in desktop
- Provider rollout: internal team's accounts → 5 beta users with that provider → 5% → 25% → 100%

**PR sequence:** (selected highlights — full sequence is 30+ PRs)
1. PR-B0: Universal asset class panel skeleton (Spec §6 — header / chart / list / insights / actions / history)
2. PR-B1: Equities panel (extends existing — adds sub-class donut + geographic bar)
3. PR-B2: Brokerage Accounts panel (extends SnapTrade UI)
4. PR-B3: Bank/Cash — Setu provider in Mizan Connect + UI
5. PR-B4: Bank/Cash — SGFinDex (Singpass OAuth flow, required redirect_uri)
6. PR-B5: Bank/Cash — Tink
7. PR-B6: Bank/Cash — Basiq
8. PR-B7: Bank/Cash — Lean
9. PR-B8: Bonds & Sukuks panel (issuer/maturity toggle bar)
10. PR-B9: Provident Funds panel (CPF/EPF/401k/NPS/Super) + nested holdings UX
11. PR-B10: Insurance panel — investment-linked + pure protection split
12. PR-B11: Private Equity panel — vintage bar + J-curve projection
13. PR-B12: Real Estate panel + AI estimation pipeline + `'ai-estimated'` badge wiring
14. PR-B13: Crypto panel — CCXT + chain reader (read-only scope enforced)
15. PR-B14: Commodities panel — donut with MetalpriceAPI feed
16. PR-B15: Collectibles panel + AI estimation pipeline
17. PR-B16: Forex panel + histogram per-pair
18. PR-B17: ETF look-through purification worker

**Definition of done:**
- All 12 panels live, each follows the universal pattern
- Per-class donut/bar visualizations meet design bar (animated, hover-to-expand, center label)
- All 6 new sync providers + 4 chain readers live with security checklist green
- Reference user can connect a Setu bank, a SnapTrade broker, a CCXT exchange, a chain reader, and see every holding in the right panel with the right badge

---

## Track D — News Module

**Scope IN:** Mizan Connect provider integrations (NewsAPI / NewsCatcher / Benzinga / Polygon / Refinitiv / Bondevalue / regional feeds), personalization worker (relevance scoring against `user_memory` + holdings via vector similarity), `news_items` table on desktop, two-tab UI (Relevant / Global), reading state, saved articles, share.
**Scope OUT:** the news strip placeholder on the dashboard (Track A already covers); the `get_news` agent tool (Track C — but the table here populates it).

**Files to create:**
- `mizan-connect/src/news/{mod,providers,personalization,handlers}.rs`
- `mizan-connect/src/news/providers/{newsapi,newscatcher,benzinga,polygon,refinitiv,bondevalue,regional}.rs`
- `mizan-desktop/apps/frontend/src/pages/news/index.tsx`
- `mizan-desktop/apps/frontend/src/components/news/{card,reader,saved-list}.tsx`

**Files to modify:**
- Dashboard news strip placeholder (from Track A) — wire to real data
- `crates/ai/src/tools/get_news.rs` (stub from Track C) — implement read from `news_items` table

**Migrations required:**
- Desktop: `mizan-desktop/migrations/NNNN_news_items.sql`
- Cloud: `mizan-connect/migrations/NNNN_news_items_per_user.sql` (materialized per-user feed)

**Tests to add:**
- Per provider: contract test against fixture API response
- Personalization: relevance scoring golden test (fixture user portfolio + news set → expected ranking)
- Sync from cloud → desktop: round-trip on `news_items`
- Read state + saved list persistence
- "Why this matters to you" rendering uses the agent's holdings-context

**ADRs:**
- `docs/adr/0030-news-providers-and-personalization-model.md`
- `docs/adr/0031-personal-materiality-scoring.md`

**Performance budget impact:**
- News module first paint: < 200ms cached, < 800ms cold from Mizan Connect (Spec §17.1)

**Security review checklist:**
- [ ] Per-user news cache scoped to user_id from JWT — no cross-user leak
- [ ] News provider API keys encrypted at rest in Mizan Connect
- [ ] No outbound user data to news providers (we pull, we don't push)

**Rollout strategy:**
- Silver gets Relevant tab basic personalization
- Gold gets full materiality scoring
- Internal first; per-region feeds enabled gradually as users join from those regions

**PR sequence:**
1. PR-D1: `news_items` migrations (desktop + cloud)
2. PR-D2: First provider integration (NewsAPI) end-to-end as a template
3. PR-D3: Personalization worker on Mizan Connect (vector similarity against user_memory + holdings)
4. PR-D4: News feed endpoint `GET /v1/news/feed?tab=relevant|global&cursor=...`
5. PR-D5: Desktop sync from cloud on app open + periodic
6. PR-D6: News page — Relevant tab UI
7. PR-D7: News page — Global tab UI
8. PR-D8: News card with "Why this matters to you" reasoning
9. PR-D9: In-app reader + related holdings side panel + "Discuss with Mizan"
10. PR-D10: Read state, saved articles, share
11. PR-D11+: per additional provider (Benzinga / Polygon / Refinitiv / Bondevalue / CNA / Mint / Khaleej Times / IFN / Salaam Gateway)

**Definition of done:**
- Two-tab News page live
- 5+ providers integrated
- Personalization runs cloud-side, ships to desktop
- Reference user sees Sukuk issuer headlines ranked above generic Fed news

---

## Track F — Zakat Engine Coverage

**Scope IN:** Maliki + Hanbali school coverage (add to existing Hanafi + Shafi'i), new asset class Zakatability rules (PE proportionate share, ULIP surrender value, locked retirement two-views, crypto toggle, debts owed/received per school), Hawl anchor tracking per cohort, Pay-Zakat flow with charity partnerships through existing Stripe, Zakat receipt + yearly export, Truth Ledger entry per Zakat calculation.
**Scope OUT:** Sharia compliance screening (Track E); the `compute_zakat` tool itself (already exists per CLAUDE.md §5 — this extends it).

**Files to create:**
- `mizan-desktop/crates/zakat/src/schools/maliki.rs`
- `mizan-desktop/crates/zakat/src/schools/hanbali.rs`
- `mizan-desktop/crates/zakat/src/rules/private_equity.rs`
- `mizan-desktop/crates/zakat/src/rules/ulip_surrender.rs`
- `mizan-desktop/crates/zakat/src/rules/locked_retirement.rs`
- `mizan-desktop/crates/zakat/src/rules/crypto.rs`
- `mizan-desktop/crates/zakat/src/rules/debt.rs`
- `mizan-desktop/crates/zakat/src/hawl_tracker.rs`
- `mizan-desktop/crates/zakat/src/pay/charity_directory.rs` — Islamic Relief, Zakat Foundation, HHRD, local mosques
- `mizan-desktop/crates/zakat/src/pay/receipt.rs`

**Files to modify:**
- `mizan-desktop/crates/zakat/src/lib.rs` — register new schools + rules
- `mizan-desktop/crates/ai/src/tools/compute_zakat.rs` — extend to honor user-selected school from `user_memory`

**Migrations required:**
- Desktop: `mizan-desktop/migrations/NNNN_hawl_anchors.sql` (cohort_id, anchor_date, current_qualifying_amount, last_evaluated)
- Desktop: `mizan-desktop/migrations/NNNN_zakat_payments.sql` (record of payments via Stripe to charities)

**Tests to add:**
- Golden tests per school × per asset class combo (4 schools × 12 asset classes = 48 minimum golden cases)
- Hawl anchor: lunar-year arithmetic golden test
- Pay-Zakat flow: end-to-end through Stripe (test mode) → receipt → ledger entry
- Truth Ledger: Zakat calculation writes an immutable entry capturing inputs / school / Nisab values / cohort states / final number

**ADRs:**
- `docs/adr/0032-maliki-school-rules.md` — must have scholarly board sign-off before merge (CP-F-rules)
- `docs/adr/0033-hanbali-school-rules.md` — same
- `docs/adr/0034-private-equity-zakatability.md`
- `docs/adr/0035-locked-retirement-two-views.md`
- `docs/adr/0036-crypto-zakatability-toggleable.md`
- `docs/adr/0037-debt-deduction-by-school.md`
- `docs/adr/0038-zakat-payment-flow-via-stripe-to-charity.md`

**Performance budget impact:**
- Zakat calculation: < 2s for the full reference-user portfolio (Spec §23)

**Security review checklist:**
- [ ] Charity directory hardcoded + signed (not user-modifiable) to prevent payment redirection attacks
- [ ] Stripe charity-recipient accounts verified at deploy time
- [ ] Zakat receipt records donor info per Stripe recommendations (CLAUDE.md §16.2 AML/KYC)
- [ ] Truth Ledger entry per calculation — chain integrity test passes after Zakat write

**Rollout strategy:**
- Gold-tier only (existing entitlement)
- Maliki + Hanbali ship behind feature flags until ADRs are scholar-signed
- Pay-Zakat flow ships to internal first; one charity at a time to start

**PR sequence:**
1. PR-F1: `hawl_anchors` migration + tracking module
2. PR-F2: Maliki school ADR + scholarly review (CP-F-rules gate)
3. PR-F3: Maliki school rules + golden tests
4. PR-F4: Hanbali school ADR + scholarly review
5. PR-F5: Hanbali school rules + golden tests
6. PR-F6: Locked retirement two-views rule
7. PR-F7: PE proportionate share rule
8. PR-F8: ULIP surrender value rule
9. PR-F9: Crypto toggleable rule
10. PR-F10: Debt deduction by school
11. PR-F11: ZakatHawlApproaching insights rule (extends Track C insights)
12. PR-F12: `compute_zakat` extension to honor user-selected school from memory
13. PR-F13: Truth Ledger entry on every Zakat calc
14. PR-F14: Pay-Zakat flow — charity directory + Stripe flow
15. PR-F15: Receipt + yearly export

**Definition of done:**
- 4 schools available; user selects one in `user_memory`
- Reference user (CP-Release scenario) gets the right Zakat number against the spec's worked example: Sukuks with `'halal-screened'` accrued profit captured, Bukit Batok primary residence excluded with reasoning, three rental Hyderabad units Zakatable-on-rental-income, one held-for-sale Zakatable-on-market-value with `'ai-estimated'` badge, Hasan VC NAV from quarterly upload, Hawl calendar correct per cohort, Truth Ledger entry written
- Pay-Zakat flow: one-tap to Islamic Relief / Zakat Foundation / HHRD; receipt generated; yearly export available

---

## Track G — Enterprise + Advisor Tiers

**Scope IN:** SSO / SAML / OIDC for Enterprise auth, multi-seat team membership extension, Advisor → Client linking model, `'advisor-reviewed'` badge surface (badge from Track E + write path here), per-client report generation, note-taking surface, separate billing model.
**Scope OUT:** the entitlement gating logic at the use sites (those threaded throughout other tracks); the existing solo team / member infrastructure (already in place per CLAUDE.md).

**Files to create:**
- `mizan-connect/src/auth/sso/{saml,oidc,mod}.rs`
- `mizan-connect/src/teams/multi_seat.rs`
- `mizan-connect/src/advisor/{mod,links,scopes,handlers}.rs`
- `mizan-desktop/apps/frontend/src/pages/advisor/clients-list.tsx`
- `mizan-desktop/apps/frontend/src/pages/advisor/client-detail.tsx`
- `mizan-desktop/apps/frontend/src/components/advisor/notes-panel.tsx`
- `mizan-desktop/apps/frontend/src/components/advisor/sign-off-button.tsx`

**Files to modify:**
- `mizan-connect/src/auth/supabase_jwt.rs` — extend to support SAML/OIDC tokens with org-group claims
- `mizan-connect/src/billing/handlers.rs` — Enterprise pricing model, Advisor per-seat or per-client
- `mizan-connect/src/teams/...` — extend team_memberships with SSO group-to-role mapping

**Migrations required:**
- Cloud: `mizan-connect/migrations/NNNN_advisor_links.sql` (advisor_user_id, client_user_id, scope enum, time_limited_token, granted_at, revoked_at)
- Cloud: `mizan-connect/migrations/NNNN_team_memberships_extended.sql` (SSO group-to-role mapping)
- Cloud: `mizan-connect/migrations/NNNN_holding_signoff.sql` (advisor sign-off records per holding)

**Tests to add:**
- SAML/OIDC: round-trip with fixture IdP (e.g. Keycloak in CI)
- Multi-seat: invite flow, role enforcement at handler level
- Advisor → Client: scope enforcement (read-only vs read-write), token expiry
- `'advisor-reviewed'` badge: only Advisor-scoped users can write the sign-off; visible to client + advisor
- Per-client report: contains advisor name on every output

**ADRs:**
- `docs/adr/0039-sso-saml-oidc-enterprise.md`
- `docs/adr/0040-advisor-client-linking-model.md`
- `docs/adr/0041-enterprise-multi-seat-billing.md`

**Performance budget impact:**
- SSO token verification adds < 50ms on first login per session (cached afterwards)

**Security review checklist:**
- [ ] SSO tokens verified against IdP signing keys
- [ ] Advisor scope enforced at every endpoint (not just UI)
- [ ] Time-limited tokens for client access; revocation works immediately
- [ ] Audit log entry per advisor view / write of a client's data
- [ ] Client must explicitly grant + revoke; never granted by advisor unilaterally

**Rollout strategy:**
- Enterprise tier: design-partner customer first (family office); then expand
- Advisor tier: small set of beta advisors; per-advisor billing iteration

**PR sequence:**
1. PR-G1: Multi-seat team extension + Enterprise billing entitlement
2. PR-G2: SAML auth path
3. PR-G3: OIDC auth path
4. PR-G4: SSO group-to-role mapping
5. PR-G5: Advisor-Client link model + scopes + time-limited tokens
6. PR-G6: Advisor clients list UI
7. PR-G7: Advisor client detail view
8. PR-G8: `'advisor-reviewed'` badge write path
9. PR-G9: Notes panel attached to clients + individual holdings
10. PR-G10: Per-client report generation (existing report generator + advisor branding)
11. PR-G11: Audit log entries for advisor accesses

**Definition of done:**
- Enterprise tier: design-partner family office uses SSO, multiple seats, custom rate limits, audit log export
- Advisor tier: an advisor sees N clients who granted access, signs off on individual holdings, exports per-client reports with their name

---

## Track J — OAuth Connector Framework

**Scope IN:** `oauth_providers` registry + endpoint set, initial 3 providers (Google Drive, Notion, Slack — recommended), background refresh worker, user-suggested service queue, annual re-consent worker, Silver+ entitlement gating.
**Scope OUT:** sync providers (Plaid / SnapTrade / Setu etc. — Track B); MCP (Track K).

**Files to create:**
- `mizan-connect/src/oauth/{mod,registry,refresh_worker,reconsent_worker}.rs`
- `mizan-connect/src/oauth/providers/{google_drive,notion,slack}.rs`
- `mizan-connect/src/oauth/handlers.rs` — `POST /v1/oauth/connect/{provider}`, `GET /v1/oauth/callback/{provider}`, `POST /v1/oauth/disconnect/{provider}`, `GET /v1/oauth/connections`
- `mizan-desktop/apps/frontend/src/pages/settings/connections.tsx`
- `mizan-desktop/apps/frontend/src/pages/settings/suggest-service.tsx`

**Files to modify:**
- `mizan-connect/src/server.rs` — register OAuth routes
- Entitlement matrix — add `oauthConnectors` capability gated to Silver+

**Migrations required:**
- Cloud: `mizan-connect/migrations/NNNN_oauth_providers.sql` (name, endpoints, scopes, handler_ref, compliance_status)
- Cloud: `mizan-connect/migrations/NNNN_user_oauth_connections.sql` (user_id, provider, encrypted_token, scopes_granted, granted_at, last_reconsented_at, expires_at)
- Cloud: `mizan-connect/migrations/NNNN_oauth_suggestions.sql` (user_id, suggested_service, status, reviewed_at)

**Tests to add:**
- Per provider: OAuth flow round-trip with fixture provider
- Refresh worker: token approaching expiry → refresh → success / failure path → reconnect notification on failure
- Re-consent worker: 14 days before annual expiry → in-app notification → user re-consents → expires_at extends
- Scope discipline: write-scope requests blocked without per-action confirmation

**ADRs:**
- `docs/adr/0042-oauth-connector-framework.md`
- `docs/adr/0043-initial-provider-selection-google-drive-notion-slack.md`

**Performance budget impact:** background workers; no direct UX path budget impact.

**Security review checklist:**
- [ ] Tokens encrypted at rest with provider-specific encryption key (AES-GCM-256)
- [ ] Disconnect calls provider revocation endpoint server-side first
- [ ] Read-only scopes by default; write requires explicit re-consent
- [ ] Annual re-consent enforced; expired = auto-disconnect
- [ ] Privacy notice surfaces at connect with scope list
- [ ] User-suggested services queued for compliance review before activation

**Rollout strategy:**
- Silver+ entitlement
- Internal first with the three providers
- User-suggested queue: review weekly; approved entries shipped behind feature flag per provider

**PR sequence:**
1. PR-J1: `oauth_providers` registry + connections table + suggestions table
2. PR-J2: Generic OAuth endpoints (`/connect/:provider` etc.)
3. PR-J3: Google Drive provider — read-only Drive scope + Mizan-watched folder for statement ingestion
4. PR-J4: Notion provider — designated database for goals/notes
5. PR-J5: Slack provider — Today's Signal delivery channel option
6. PR-J6: Background refresh worker
7. PR-J7: Annual re-consent worker + 14-day pre-expiry notification (extends Track C insights)
8. PR-J8: Settings UI — Connections list + scope display + revoke buttons
9. PR-J9: Suggest-a-service form + admin review surface
10. PR-J10+: Additional providers (Apple Health, GitHub, Spotify, Calendar, Dropbox/OneDrive/iCloud, Zapier)

**Definition of done:**
- User in Settings → Connections sees three connected services; can revoke any; can suggest new ones
- Google Drive statement-ingestion path proven end-to-end (statement drops in folder → agent ingests → activities written)
- Annual re-consent fires + user re-grants successfully

---

## Track K — MCP Capability

**Scope IN:** per-user MCP gateway in Mizan Connect, `mcp_servers` registry + `mcp_call_log` audit table, dispatcher integration with read-mostly gate, `scratchpad` namespace + UI surface, public catalog with security review process, egress DLP rules for sensitive identifier patterns, Gold+ entitlement gating, the `trust_level` enum prep.
**Scope OUT:** building any actual MCP server (this is consumption-only); the Sharia screen on MCP servers themselves (not in scope).

**Files to create:**
- `mizan-connect/src/mcp/{mod,gateway,registry,catalog,egress_filter,call_log}.rs`
- `mizan-connect/src/mcp/handlers.rs` — `POST /v1/mcp/server`, `GET /v1/mcp/servers`, `DELETE /v1/mcp/server/:id`
- `mizan-desktop/crates/ai/src/dispatcher/mcp_gate.rs` — the absolute read-mostly gate
- `mizan-desktop/crates/storage-sqlite/src/scratchpad.rs` — sandboxed per-user K/V store
- `mizan-desktop/apps/frontend/src/pages/settings/ai/connected-tools.tsx`
- `mizan-desktop/apps/frontend/src/components/badge/popover-renderers/mcp.tsx`

**Files to modify:**
- `mizan-desktop/crates/ai/src/dispatcher.rs` — route MCP-namespaced tools through `mcp_gate`
- Entitlement matrix — add `mcpCapability` capability gated to Gold+

**Migrations required:**
- Cloud: `mizan-connect/migrations/NNNN_mcp_servers.sql` (user_id, server_url, auth_method, name, trust_level enum default 'untrusted', last_reviewed_at)
- Cloud: `mizan-connect/migrations/NNNN_mcp_call_log.sql` (server_id, tool, params_digest, response_digest, duration_ms, timestamp)
- Cloud: `mizan-connect/migrations/NNNN_mcp_catalog.sql` (curated public catalog with security review metadata)
- Desktop: `mizan-desktop/migrations/NNNN_mcp_scratchpad.sql`

**Tests to add:**
- Gate: MCP tool attempting to write to `truth_ledger` / `holdings` / `activities` / `balances` is rejected at dispatcher
- Gate: MCP tool can write only to `scratchpad`
- Egress DLP: payload containing SSN / PAN / Aadhaar / card patterns rejected pre-send
- Rate limit: 60 MCP calls per minute per user enforced
- Catalog: malicious self-registered server attempt → blocked at gateway with credible report → catalog delistable within 24h
- Logs: every MCP call records timestamp/server/tool/params digest/response digest/duration in `mcp_call_log`
- Adversarial: prompt-injection attempt via MCP response → agent doesn't change financial state

**ADRs:**
- `docs/adr/0044-mcp-capability-architecture.md`
- `docs/adr/0045-mcp-sandbox-gate-absolute.md` — the read-mostly rule is non-negotiable
- `docs/adr/0046-mcp-egress-dlp-rules.md`
- `docs/adr/0047-mcp-public-catalog-review-process.md`
- `docs/adr/0048-mcp-trust-level-schema-prep.md` — schema column ready, but never honoured

**Performance budget impact:**
- MCP tool call timeout: 10s default (configurable up to 30s)
- Gateway adds < 100ms overhead per call (measured)

**Security review checklist (the most stringent in any track):**
- [ ] `mcp_gate` rejects any mutation to financial tables — penetration tested
- [ ] Egress DLP rejects sensitive identifier patterns — fixture-tested
- [ ] Outbound MCP uses dedicated egress proxy with separate rate limits
- [ ] Every call logged with digest (no raw payload retention)
- [ ] User per-call confirmation for outbound financial data
- [ ] Public catalog entries reviewed at registration + annually
- [ ] Self-registered servers carry warning badge + require user-acknowledged confirmation
- [ ] Suspected misbehaving servers can be delisted within 24h
- [ ] ToS Gold+ clause states user responsibility for self-registered MCPs

**Rollout strategy:**
- Gold+ entitlement only
- Initial: Mizan-curated public catalog (1-3 servers) — Notion, GitHub, Linear chosen for low-risk read-only-by-design
- Self-registered MCP: opt-in beta with explicit acknowledgment
- Penetration testing required pre-public-launch (CP-K-sandbox)

**PR sequence:**
1. PR-K1: Schema (mcp_servers / mcp_call_log / mcp_catalog / scratchpad)
2. PR-K2: Gateway skeleton + endpoint set
3. PR-K3: Dispatcher integration — MCP-namespaced tools routed through `mcp_gate`
4. PR-K4: The absolute read-mostly gate + penetration tests
5. PR-K5: Scratchpad namespace + UI surface ("Notes from connected tools")
6. PR-K6: Egress DLP filter + fixture patterns
7. PR-K7: Rate limiting + per-server timeouts
8. PR-K8: `mcp_call_log` audit table + digest computation
9. PR-K9: Public catalog UI in Settings → AI → Connected Tools
10. PR-K10: Catalog review process + delisting workflow
11. PR-K11: Self-registration flow + warning badge + acknowledgment
12. PR-K12: Per-call user confirmation UI for outbound financial data
13. PR-K13: ToS Gold+ clause + privacy policy update
14. PR-K14: CP-K-sandbox penetration testing — adversarial servers, prompt-injection attempts

**Definition of done:**
- Gold user registers a Notion MCP server, agent calls Notion read tools, results land in scratchpad with `'mcp'` badge
- A test server attempting `update_holding` is rejected at the gate (test passes)
- Payload containing `xxx-xx-xxxx` SSN pattern is rejected by egress DLP
- Catalog has 3+ vetted entries
- ToS + privacy policy updated and live
- Penetration test report signed off — zero blocker findings

---

# VERIFICATION SECTION — How To Test This End-to-End

After Track-by-Track completion, the **CP-Release** acceptance walk:

**1. Reference-user scenario (Spec §23):**
   - Open Mizan as the reference user (Singapore Sharia-aware millionaire) in Ramadan
   - Say *"what's my Zakat this year?"* — agent answers in < 2 seconds
   - Verify: number broken down by asset class, Hawl calendar per cohort, scholarly school used, every input traceable to Mizan Badge `'audit-trail'` hash mapping to Truth Ledger
   - Verify: Sukuks show `'halal-screened'`, accrued profit current to Bondevalue end-of-day, Bukit Batok primary explicitly Not Zakatable, four Hyderabad units split by intent, Hasan VC NAV from last quarterly with reminder for next due
   - Scroll: Today's Signal shows Emaar Sukuk maturity 47 days, three replacement Sukuks shortlisted; News Relevant tab top item is Dar al Arkan rating upgrade
   - Tap Sukuks panel: bar by issuer, tap Emaar, *"start preparing reinvestment shortlist with $200K minimum and 5-7 year duration"* — agent registers task, sets 30-day reminder
   - Tap Net Worth: Sankey shows quarterly cash flow, agent annotates anomaly (one-time property tax India) confirmed by user
   - Tap dashboard heatmap: tiles render with provenance, every number has a Mizan Badge

**2. Tier matrix walk:**
   - Free user: manual entry only, view-only badges, 50-msg AI daily, rule-based weekly insights
   - Silver user: bank sync (Plaid/Setu/etc), broker sync, real-time data, multi-currency, Today's Signal, 5 AI estimates/mo
   - Gold user: Zakat engine, Sharia screening, unlimited AI estimates, Personal CFO mode, advanced reports, Opus model, MCP capability
   - Enterprise user: SSO login, multi-seat, custom rate limits, audit log export
   - Advisor user: client list view, sign-off badge writes, per-client reports

**3. Security re-runs:**
   - `cargo audit` zero
   - `cargo deny check` zero
   - Token-plaintext scan zero (QA-P1.2 still green)
   - Truth Ledger chain integrity test passes
   - HS256 rejection in production test passes
   - Webhook signature verification mandatory for every webhook endpoint
   - MCP sandbox gate penetration test zero breaches
   - Egress DLP DLP-pattern rejection test passes

**4. Performance re-runs:**
   - Cold start < 1.2s on reference machine
   - Chart first-paint < 200ms cached
   - Endpoint p99 < 300ms read / < 800ms sync
   - Agent round-trip < 500ms intent / < 2s read tools / < 5s write tools
   - Notification panel < 100ms
   - News module < 200ms cached / < 800ms cold

**5. Operational re-runs:**
   - One full rollback drill end-to-end in staging
   - One full Supabase lifecycle review (slow-query log + index audit + bloat monitor)
   - Quarterly tech-debt sweep run once
   - Quarterly key rotation drill run once
   - Audit baseline re-run — no drift from signed baseline

**6. Code hygiene re-runs:**
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings` zero
   - `cargo udeps` zero
   - `cargo machete` zero
   - `cargo mutants` nightly score above floor (80% / 95% on financial crates)
   - `knip` + `ts-prune` zero unused
   - `gitleaks` + `trufflehog` zero findings over full git history
   - Coverage: 80% line / 70% branch workspace; 95% on financial-truth, zakat, ai/dispatcher, billing, auth, webhooks

Any failure here is a release blocker. The audit re-runs quarterly per CLAUDE.md §18.12.

---

# PHASE 3 EXECUTION ENTRY POINT

After approval of this plan via ExitPlanMode, the immediate next steps are:

1. **Phase 0 baseline confirmation (deferred from plan mode)** — run `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo audit`, `cargo deny check`, `cargo fmt --all -- --check`, `pnpm test`, `pnpm lint` against the actual repo at `/Users/samisayyed/Documents/mizan-ai-native/`. Surface any red.
2. **PR-H1: ADR 0001** — adopt the Downloads CLAUDE.md as the project working agreement, replace the repo's existing CLAUDE.md, archive the old to the ADR.
3. **PR-H2: ADR 0007** — repo rename `mizan-4/` → `mizan-desktop/` if approved at CP-0 Q6.
4. **PR-H3 sequence: crate extractions** — financial-truth, zakat, insights, synthesis, csv-import. One PR per crate, behind clean cross-crate type bridges.
5. Continue Track H through PR-Hfinal.
6. Begin Tracks I + E in parallel.
7. Once Track I + E close, begin A + C-foundation.
8. Once A + C-foundation close, begin B + D + F.
9. Once B closes, begin G + J.
10. K runs last.
11. Final CP-Release acceptance walk against the reference-user scenario.
