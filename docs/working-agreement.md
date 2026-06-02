# CLAUDE.md — Mizan Codebase Working Agreement

*The operating contract for any AI assistant working on Mizan. Read this before touching anything. Every rule here has a scar behind it.*

---

## 0. Read This First

Six rules that override everything else:

1. **The Truth Ledger is sacred.** Every write to balances or holdings emits a `prev_hash || event_payload → blake3 → curr_hash` chain entry. You never bypass this. You never break the chain. Golden tests verify integrity (QA-P2.4). One tampered row → instant detection.
2. **No silent FX fallbacks. Ever.** The QA Pass 8 lesson: a `?? 1.0` on a missing rate corrupted real user net worth figures. Every cross-currency computation reads `fx_rates` explicitly with a timestamp. If a rate is missing, the function errors loudly. There is no "reasonable default."
3. **AI tools obey the Safety Runtime contract.** Per-turn cap, audit log entry per call, numeric bounds, Truth Ledger emission on financial mutations. Tools registered in `crates/ai/dispatcher` without all four properties are rejected at compile time. No exceptions for "just a quick tool."
4. **Provider tokens are never in plaintext on disk.** Plaid, SnapTrade, Setu, SGFinDex, Tink, Basiq, Lean, CCXT — all access tokens are AES-GCM-256 encrypted via `SecretCipher::from_bytes(&{PROVIDER}_TOKEN_ENCRYPTION_KEY)` and live only in Mizan Connect Postgres. The token-plaintext scan from QA-P1.2 stays green. Forever.
5. **Migrations are forward-only.** Crash-safe DDL guarantees the DB is in the pre- or post-migration state, never in-between. There are no down migrations. If you need to revert schema state, write a corrective forward migration.
6. **Production rejects HS256 JWTs even if the test secret leaks.** Auth verification uses RS256 via JWKS in prod; the HS256 fallback is gated to `cfg(test)` / dev with `MIZAN_TEST_JWT_SECRET`. Production builds refuse it regardless of env. Don't try to be clever here.

If you are about to do something that violates any of these, stop and surface the problem in chat. Don't ship it.

---

## 1. The Mission and the Bar

Mizan is an **AI-native command center for personal wealth**, built for sophisticated multi-jurisdictional users — particularly the global Muslim affluent diaspora whose balance sheets include Sukuks, CPF/EPF, ULIPs, GCC bank accounts, and multi-currency holdings that no mainstream tracker handles well. The Zakat engine is the headline differentiator. The Sharia screening layer is a moat that takes years to build.

The product is **local-first** (data lives on the user's device, in SQLite, encrypted at rest), with an optional **cloud layer** (Mizan Connect on Fly.io + Supabase Postgres) handling broker aggregation, AI agent orchestration, billing, and cross-device sync.

The bar is Apple / Netflix grade. Not as a slogan — as an enforced engineering posture. The 1,912 workspace tests, the 18 QA passes, the `cargo clippy --workspace --all-targets` zero-warnings rule, the §A19 performance budgets (cold start < 1.2s, chart first-paint < 200ms) — that's the floor, not the ceiling. New work must clear the same bar.

---

## 2. The Three Surfaces

Anchor where you are before you start changing things:

- **Mizan Desktop** — Tauri 2 (Rust + WebView), single Rust process owns SQLite, IPC, all crypto. React 18 + TS strict + Vite + Tailwind + shadcn/ui in the sandboxed WebView. Recharts for visualization. Code-signed: Apple Developer ID + notarization for macOS, Azure Trusted Signing for Windows. Tauri auto-updater pinned to `mizan.app/updates/latest.json`.
- **Mizan Connect** — Axum + Tokio on Fly.io, Postgres on Supabase Pro, Sqlx with compile-time-verified queries. 23+ migrations (forward-only, numbered). Supabase Auth with RS256 JWT via JWKS in prod. Currently on v38+. AES-GCM-256 envelope encryption on all external provider tokens. Multi-secret Stripe webhook rotation. Idempotent webhook processing.
- **Mizan Badge** — provenance affordance with `origin` enum on every account/holding row. Renders across the entire UI showing where every number came from. Treat this as a product surface, not a styling primitive.

Every code path in Mizan belongs to one of these three. Know which one before you touch it.

---

## 3. Hard Rules — Non-Negotiables Beyond Section 0

### 3.1 Authentication & Authorization

- Every Mizan Connect endpoint is JWT-verified through the existing middleware. New endpoints inherit this — don't write bare handlers.
- Authorization is scoped to `user_id` from the session token, never from the request body. No IDOR. CI has a regression test for this; don't disable it.
- The admin endpoints (`/v1/admin/...`) use constant-time bearer comparison via `subtle::ConstantTimeEq`. Never use `==` on the admin token. Past bug; don't repeat.
- `secrecy::SecretString` wraps every env-loaded secret. `.expose_secret()` is called only at the use site, never logged.
- The Supabase service_role key never reaches the desktop client. Every privileged op terminates in Mizan Connect.

### 3.2 Webhooks

- Every webhook endpoint verifies signature before doing anything else. The five Stripe rotation tests (single-secret, rotation-pair, neither-matches, whitespace tolerance, empty-entry skip, timestamp-outside-tolerance) are the model. New webhook integrations must have parity coverage.
- Idempotency is by provider event ID, lookup-or-insert (the QA-P1.3 pattern). Never re-process a webhook.
- Plaid webhook key cache fetches verification keys from `/webhook_verification_key/get` on first use and caches by key_id. Don't bypass this cache.

### 3.3 Money Math

- All financial computation lives in `crates/financial-truth` (FIFO cost basis, TWR, IRR, realized P&L). Don't reimplement it elsewhere. Golden-tested against known answers.
- Decimal arithmetic on currency: `rust_decimal::Decimal`, never `f64`. The agent's tool dispatcher enforces numeric bounds — `f64` slipped in once and produced a $1.32 rounding drift on a $1.7M portfolio. Caught in QA Pass 4. Never again.
- Currency conversions read `fx_rates` table explicitly with timestamps. If you're touching a function that converts currencies and you don't see the timestamp parameter, you're looking at a bug.

### 3.4 The MCP Sandbox Bright Line

MCP tools (Gold+ tier) may never write to Truth Ledger, holdings, activities, or balances. Enforced at the dispatcher in a read-mostly gate that rejects mutations to financial tables. If a future product decision opens "trusted MCP" tier, that's a separate architectural change requiring a security review — not a relaxation of this rule.

---

## 4. Code Conventions

### 4.1 Rust

- `rustfmt` workspace config, gated at CI. No exceptions.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero warnings. The lint gate has never been relaxed and never will be.
- Errors: `thiserror` for library crates, `anyhow` for binary crates. No `unwrap()` in write paths (the `apple-5` pass eliminated every silent `?? 1` and `unwrap` in mutation paths — don't reintroduce).
- Public functions: doc comments with `# Errors` and `# Panics` sections when applicable.
- Async runtime: `tokio` exclusively. No mixing with `async-std`.
- Lifetimes: minimal explicit annotations; let the compiler infer where possible. When you need them, name them precisely (`'src`, `'cipher` — never `'a` `'b` in production code).
- Module boundaries: each crate owns its types. Cross-crate types go through a shared `mizan-types` crate, never duplicated.

### 4.2 TypeScript

- `strict: true`, `noImplicitAny`, `noUncheckedIndexedAccess`. All enabled. CI gates.
- `prettier` + `eslint` with strict ruleset. No `// eslint-disable` without an issue link.
- Component primitives from `shadcn/ui`. Don't roll your own Button, Card, Dialog, Badge unless you have a documented reason.
- State: `TanStack Query` for server state, `useState` / `useReducer` for local. Don't pull in Redux or Zustand — the current state model is intentional.
- Routing: `TanStack Router` with type-safe routes. Hand-rolled `<a href>` to internal routes is a code smell.
- Numbers in UI: tabular numerals always (`font-feature-settings: 'tnum'`), right-aligned in tables, color-coded by sign with always-included sign/shape cues for accessibility (never color alone).

### 4.3 SQL

- Lowercase keywords (`select`, `where`, `join`, `order by`).
- Snake_case identifiers throughout. Never camelCase a column.
- Indented JOIN clauses. Multi-line queries when there's more than one JOIN.
- Every WHERE clause on a frequently-queried path must be index-backed. CI lint warns on full-table scans against tables > 10k rows.
- Sqlx compile-time-verified queries. If a query can't compile, the build fails. Don't bypass with `query_unchecked!`.

### 4.4 Naming

- Precise, English, no abbreviations except domain-standard: FX, KYC, AML, NAV, YTM, AAOIFI, FRS (CPF Full Retirement Sum), ISIN, CUSIP.
- Variable names never repeat their type. `account_account` is banned. `account_id` is the right form when ID is needed.
- Functions: verbs (`compute_zakat`, `sync_account`, `refresh_token`). Not nouns.
- Booleans: positive framing (`is_active`, not `is_not_inactive`). Never double negatives.

### 4.5 Comments

Comments explain **why**, not what. Code that needs a comment to explain *what* it does gets rewritten. The rare exception: code interfacing with an external system whose behavior is non-obvious (Plaid's `redirect_uri` being optional, SnapTrade's 405 mapping). Those comments cite the external doc.

No `// TODO` without a tracked issue link and an owner. CI lints for this.

---

## 5. When You Touch Specific Subsystems

### `crates/financial-truth`

- Hard floor 95% coverage. Mutation testing runs nightly via `cargo mutants`. Any surviving mutant is an immediate fix.
- All changes require two reviewers.
- Golden tests are the source of truth. If a golden test fails, do not "update" the expected output to match the new value without first proving the new value is correct. The whole point of the golden test is to catch silent drift.
- FIFO cost basis math has been verified against three independent tax-software outputs. Don't refactor "for cleanliness" without re-verification.

### `crates/zakat`

- Same 95% coverage floor as `financial-truth`. Same two-reviewer rule. Same mutation testing.
- Hanafi / Shafi'i / Maliki / Hanbali school differences are encoded as explicit branches, not config flags. Each school's edge cases (locked retirement treatment, debt deduction, business inventory, real estate intent) is documented in `docs/adr/zakat-schools.md`.
- Nisab values fetched from spot gold/silver via MetalpriceAPI at calculation time, never cached past the current calculation.
- Every Zakat run writes a Truth Ledger entry capturing inputs, school, Nisab values, asset cohort states, and final number. Audit trail. Always.

### `crates/ai`

- New tools are registered in `dispatcher::register_tool` with their AI Safety Runtime properties declared in the registration call: per-turn cap weight, audit log scope, numeric bounds (if applicable), Truth Ledger emission flag. A tool without all four is a compile error.
- Tool implementations live in `crates/ai/tools/<tool_name>.rs`. One file per tool.
- The memory writer is the only function allowed to write to `user_memory`. Don't bypass it from tool handlers — implicit memory writes pollute the store.
- Prompt templates live in `crates/ai/prompts/`. Anthropic prompt caching is configured on the system prompt + memory pull blocks. Cache invalidation tied to template version hashes — bumping a template version evicts the cache.

### `crates/insights`

- Rules are deterministic functions: `(InsightsContext) -> Vec<InsightCandidate>`. No I/O inside a rule. Test with table-driven golden tests.
- The rule registry in `lib.rs` is the discovery point. Adding a rule means: write the function, add to registry, add a golden test, add a TypeScript binding for the rendered notification copy. CI rejects partial additions.
- The agent renders structured InsightCandidates into natural language separately. The rule does not produce strings.

### `mizan-connect/src/auth`

- 95% coverage floor. Two reviewers.
- JWT verification path through `verify_token`. Don't write parallel verification logic. If you need to extract a claim, add a method to the existing verifier.
- JWKS cache TTL is 6 hours by default; configurable. Don't cache forever even if you're tempted by latency.
- Test secret rejection in prod is verified by an integration test that runs in CI. Don't disable that test "just to ship."

### `mizan-connect/src/billing`

- 95% coverage floor. Stripe webhook signature verification has the 5 rotation tests; mirror that pattern for any new payment provider.
- The DELETE-then-INSERT pattern in `set_subscription_for_team` exists because the partial unique index `idx_subscriptions_team_active` doesn't cover `incomplete` rows. Don't switch to `ON CONFLICT`; you will reintroduce the bug.
- `ensure_solo_team` is called in `POST /v1/billing/checkout` before any team_id read. Without it the user crashes on first checkout if `team_id` is NULL. This is in the code with a comment — don't remove the comment.

### `mizan-connect/src/sync/*`

- Every new sync provider follows the template: `/v1/sync/{provider}/link-token`, `/exchange`, `/sync`, `/webhook`. Endpoints share middleware via `Router::with_state`.
- Tokens encrypted with provider-specific `SecretCipher` instances. Each provider's encryption key is a separate env secret, rotated quarterly.
- Webhook signature verification is mandatory. Idempotency by provider event ID, lookup-or-insert into a per-provider event table.
- Sync runs write to `sync_run_ledger` with status, duration, error. The desktop UI reads from this ledger to show "last synced" timestamps. Don't skip the write — the UI breaks silently.

### Mizan Badge Components

- The badge primitive accepts `origin` + `modifiers[]`. Order in `modifiers` is severity (most severe first): `'stale'` > `'pending-reconciliation'` > `'ai-estimated'` > compliance badges > `'audit-trail'` > `'agent-modified'` > `'mcp'`.
- Every account / holding render in the UI must include the badge. There is no "compact mode" without provenance. If your design proposes one, change the design.
- Hover popover content comes from a per-modifier renderer. Adding a modifier means adding the renderer in the same PR. CI lints for this.

### Recharts Visualizations

- Donut for categorical breakdowns. Bar for item-by-item comparison. Heatmap for the dashboard and Equities sector exposure. Sparkline in panel cards. Sankey only on the Net Worth page. That's the entire visual vocabulary. Don't introduce new chart types without product approval.
- Use the shared chart theme tokens (`--color-chart-positive`, `--color-chart-negative`, `--color-chart-neutral`). Never hardcode hex.
- Empty states are explicit. Don't render a chart with a fake zero line that looks like data. Render the empty state surface.

---

## 6. Testing Standards

Workspace floor: 80% line, 70% branch. Hard floors at 95%: `crates/financial-truth`, `crates/zakat`, `crates/ai/dispatcher`, `mizan-connect/src/auth`, `mizan-connect/src/billing`, `mizan-connect/src/webhooks`.

Test types:

- **Unit** — per function, table-driven where the input space is enumerable.
- **Integration** — across module boundaries. For the cloud, runs against a real Postgres in CI.
- **Golden** — financial computations (TWR, IRR, FIFO, Zakat) tested against pre-computed expected outputs from independent sources. Updating a golden file requires justification in the PR description.
- **Mutation** — `cargo mutants` nightly on critical crates. Score floor 80% (95% for financial-truth and zakat).
- **End-to-end** — Playwright on the desktop UI. 14 critical-path tests today: onboarding, add holding, run Zakat, generate report, edit via agent, reconcile sync conflict, etc. Don't let this count drop.
- **Performance regression** — fixed-spec CI runner measures cold start, chart paint, query p99. Regression > 5% fails the build.

When a bug is fixed, a test is added that would have caught it. This is how the 18 QA passes accumulated. Each pass left a permanent test behind.

---

## 7. Performance Budgets

The §A19 budgets, enforced at release gate:

- App cold start: < 1.2s on the reference Apple Silicon machine
- Chart first-paint: < 200ms from cached data
- Mizan Connect endpoint p99: < 300ms for read endpoints, < 800ms for sync endpoints
- Agent round-trip (intent classification): < 500ms
- Agent round-trip (tool call): < 2s for read tools, < 5s for write tools
- Notification panel open animation: < 100ms

Regression > 5% on any budget fails the build. If you're adding a feature that fundamentally can't meet a budget, propose a budget revision in an ADR before writing code.

Measurement methodology:

- Cold start measured from process spawn to first interactive paint (read from the Tauri main → WebView ready event)
- Chart paint measured via Performance API marks placed at component mount and first render
- Endpoint latency from request-id middleware timing; p99 over 1h trailing window
- Agent latency from dispatcher entry to dispatcher exit

---

## 8. Security Boundaries

The bright lines:

- **Private keys and seed phrases are never accepted, stored, or logged.** Crypto integrations are read-only public address reads (Etherscan-family) or read-only exchange API keys (CCXT, with `withdraw`/`trade` scopes rejected at validation).
- **CAPTCHAs are never solved or bypassed.** The user solves them.
- **Permissions and access-control settings are never modified on a user's behalf.** Mizan can read; the user grants and revokes.
- **No data exfiltration to MCP servers without explicit per-call user confirmation.** The egress DLP rules in `mcp_egress_filter` reject payloads matching sensitive identifier patterns (SSN, PAN, Aadhaar, full card numbers) before they leave the gateway.
- **Audit logs are append-only and immutable in production.** Cryptographic write-once semantics where the storage backend supports it; otherwise enforced at the application layer with a chained hash.

---

## 9. Cache, Versioning, Self-Updating

Every cache table has a TTL declared in `crates/storage-sqlite/src/cache_policy.rs`. New cache tables without an entry are rejected by CI lint. No cache row lives forever.

On app version mismatch (binary version != `app_version` row in SQLite at startup), the cache eviction worker runs synchronously before the WebView is allowed to render. Major version bumps trigger full eviction; minor/patch trigger selective per the migration manifest.

The Tauri auto-updater:

- Signs manifests; signatures verified against the production public key bundled with the binary
- Takes a pre-update DB snapshot (`mizan.db.pre-{old_version}`, retained 30 days)
- Runs a post-install self-test on first launch (schema match, crypto round-trip, Twelve Data heartbeat, Mizan Connect heartbeat, Truth Ledger chain head verification)
- Offers automatic rollback to the snapshot on self-test failure
- Channels: stable / beta (Gold+ opt-in) / nightly (internal)

Mizan Connect API: production endpoints under `/v1/...`. Breaking changes ship as `/v2/...` with a minimum 6-month deprecation window. Clients send `X-Mizan-Client-Version` so handlers can branch during transitions.

Vite emits content-hashed asset filenames. WebView verifies bundle hash matches binary expectation at load. Mismatch → wipe and reload.

---

## 10. Database Discipline

### SQLite (Desktop)

- Migrations under `mizan-desktop/migrations/`, numbered, forward-only, crash-safe DDL. The in-house migrator runs on app start.
- Encryption at rest via SQLCipher (option) plus OS-level FileVault/BitLocker reliance (default).
- Every migration touching a cache-referenced schema declares which caches to evict (CI rejects otherwise).
- `truth_ledger` table is append-only. Even soft-deletes write a compensating event; never `UPDATE` or `DELETE` from `truth_ledger`.
- `sync_run_ledger` is append-only by same principle.

### Postgres (Mizan Connect, Supabase Pro)

- Migrations under `mizan-connect/migrations/`, numbered, forward-only. Currently 23+.
- Daily automated backups via Supabase Pro. Point-in-time recovery enabled.
- `pg_stat_statements` enabled. Slow-query log monitored; queries > 100ms p99 reviewed weekly during tech debt sweep.
- Row-level security policies on every table holding user data. Reviewed per table; never relaxed without security sign-off.
- Partial unique indexes (like `idx_subscriptions_team_active`) are first-class citizens — when modifying constraints, check what's not covered.
- Quarterly bloat monitoring via `pgstattuple`. `VACUUM FULL` scheduled in low-traffic windows when needed.

---

## 11. AI Agent Development

The agent has a registered toolset. Adding a tool is a four-file change:

1. Tool implementation in `crates/ai/tools/<tool_name>.rs`
2. Registration call in `crates/ai/src/dispatcher.rs` declaring per-turn cap weight, audit scope, numeric bounds, Truth Ledger emission flag
3. Test in `crates/ai/tests/tool_<tool_name>.rs` covering happy path + AI Safety Runtime compliance + Truth Ledger emission (if applicable)
4. TypeScript binding in `web/src/lib/ai/tool-types.ts` for type-safe agent event handling

Skipping any of these four is rejected at review. The dispatcher has a registry-validation routine that panics on startup if a tool was added in registration without being added in tests — fail-fast is the principle.

Memory writer discipline: only `crates/ai/src/memory/writer.rs::write_fact` writes to `user_memory`. Implicit memory writes from tool handlers are forbidden. The writer is invoked by the agent's planner when it has decided a fact is worth persisting, and the user has the right to see, edit, and delete every fact in the store.

Prompt template versioning: every change to a system prompt or memory pull template bumps a template version hash. Anthropic prompt cache invalidates on version bump. Documentation lives in `crates/ai/prompts/CHANGELOG.md`.

The "App without AI ceases to function" contract is architectural. If you're proposing a UI path that bypasses the agent for a mutation that should flow through it (because it has reasoning to share, confirmation to ask, memory to update), reconsider. The agent is the OS, not an optional helper.

---

## 12. Mizan Badge — Provenance Always

Every number on screen carries its source. Implementation rule: in the UI, no holding / account / amount renders without a Mizan Badge attached. There is no "compact density mode" without provenance. The badge primitive is the carrier — origin variant + modifier stack.

When you add a new sync provider, you add a new origin variant. When you add a new state condition (stale data, AI-estimated value, Sharia status), you add a new modifier badge with its renderer.

The audit-trail badge specifically links any displayed financial figure back to its Truth Ledger hash. This is forensic capability — years from now a user must be able to prove what any number was based on. Don't render high-value figures (net worth, Zakat owed, returns) without the audit-trail badge accessible.

---

## 13. Past Bugs — Things We Learned the Hard Way

The 18 QA passes left a permanent trail. Don't undo these:

- **QA Pass 3 — Date parser.** A single date format crashed the taxonomies seed. Fixed by the 5-format fallback parser. Don't simplify "for cleanliness" — the brokers really do return five formats.
- **QA Pass 8 — Silent FX fallbacks.** The `?? 1.0` produced wrong net worth figures. Every FX function now reads `fx_rates` explicitly. The lint rule for this is `clippy::disallowed_methods` blocking specific fallback patterns.
- **Plaid `redirect_uri` Option fix.** Made truly optional so non-OAuth deployments don't crash. Don't make it required again.
- **SnapTrade 405 graceful mapping.** When SnapTrade login isn't enabled, the UI gracefully handles the 405 instead of crashing. Don't "fix" this by removing the handler.
- **`--no-cache` deploy at v37.** Fly deploys cached binaries when source actually changed. The fix is `--no-cache` on deploys that involve binary changes. The runbook is `docs/runbooks/deploy.md` — follow it.
- **Constant-time admin compare.** `==` on bearer tokens leaks via timing. Use `subtle::ConstantTimeEq`. Always.
- **Production HS256 rejection.** Even if the test secret leaks into prod env, prod builds refuse HS256 verification. The check is in `auth::verify_token::check_algorithm_for_environment`. Don't remove it.
- **Plaid `redirect_uri: Option<String>` schema.** It's `Option`, not required. Tests in `mizan-connect/tests/plaid_link_token.rs` cover the None case.
- **Numeric drift via f64.** `f64` slipped into a P&L calculation and drifted $1.32 on a $1.7M portfolio. The fix is `rust_decimal::Decimal` everywhere financial. CI lint rejects `f64` in money paths.
- **Net-worth race at cold start.** Fixed in QA Pass 2 — the headline-figure synthesizer was reading before all repository hydration completed. The fix is an explicit `await_hydration()` in the synthesizer entry. Don't remove.
- **Heatmap recharts prop-shape mismatch.** UX-11 fix — the data shape passed to the heatmap component didn't match Recharts' expected schema, rendering a blank chart that looked like empty data. There's now a runtime schema validation in dev mode. Keep it.

When you write a fix for a new bug, write a test that would have caught it. That's how this list got built.

---

## 14. Workflow — How to Approach a Change

1. **Read before you write.** Skim the file, the related files, the tests. Understand the surface area before touching it.
2. **Minimum viable change.** The smaller the diff, the easier to review, the lower the bug risk. Resist the urge to refactor adjacent code "while you're there." File a separate issue.
3. **Tests first if the change is non-trivial.** TDD isn't religion here, but for financial logic, security boundaries, or AI tool registration, the test is the spec.
4. **Run the existing tests locally before pushing.** CI catches regressions but you waste team time if you push known breakages.
5. **PR description tells a story.** What was wrong / what you changed / why / what could go wrong / how you tested. The PR template enforces this.
6. **Self-review your diff before requesting review.** Read it as if someone else wrote it. Catch the obvious issues yourself.
7. **Address every review comment.** Either fix, or push back with reasoning, or file a tracked issue. No "I'll get to it later" without a deadline.

For larger changes (new asset class, new sync provider, new AI tool category):

1. ADR in `docs/adr/` first. Decision, rationale, alternatives considered. Reviewed before implementation.
2. Implement behind a feature flag where possible. Supabase config can toggle.
3. Canary deploy: internal team → beta opt-in users → 5% → 25% → 100% over 4 hours, with Sentry-monitored auto-rollback.

---

## 15. The Monitoring Dashboard

A private internal surface at `admin.mizan.app`, served by Mizan Connect, accessible only to authenticated team members with `team_members.role = 'admin'`. Built so the team knows the health, growth, and health of the product without anyone having to ask.

### 15.1 Acquisition Metrics

- **Downloads** — per day per platform (macOS aarch64 / macOS x86_64 / Windows x86_64 / Linux deb / Linux AppImage). Source attribution (direct, referral, search, social, paid).
- **Geographic distribution** — country-level via download IP geolocation (privacy-preserving, no per-user tracking).
- **App version distribution** — what % of installed base is on each version.
- **Update adoption curves** — how fast users move to a new version after release. Stable / beta / nightly channel splits.

### 15.2 Engagement Metrics

- DAU / WAU / MAU with 90-day trailing graph
- D1 / D7 / D30 retention cohorts
- Session length distribution (heatmap by hour of day)
- Feature usage heatmap — which dashboard panels get opened, which charts get tapped, which agent commands get used
- AI agent invocations per user, broken down by tool
- MCP tool calls per Gold+ user (when Track K ships)

### 15.3 Tier Distribution

- Free / Silver / Gold / Enterprise / Advisor counts
- Free → Silver conversion rate (trailing 30 / 60 / 90 day cohorts)
- Silver → Gold conversion rate
- Churn rate per tier
- Trial-to-paid conversion (if trials introduced)
- Reactivation rate (churned → returned)

### 15.4 Revenue

- MRR / ARR with month-over-month delta
- Net revenue retention (NRR)
- Average revenue per user (ARPU) per tier
- Stripe webhook latency p99 — leading indicator of billing system stress
- Failed payment rate
- Refund rate (count + amount)
- Churned MRR vs new MRR vs expansion MRR

### 15.5 Reliability

- Mizan Connect uptime per Fly region
- Sentry error rate per release (so a bad deploy is visible within minutes)
- Performance budget compliance — cold start p50/p99, chart paint p50/p99, query p99
- Sync run success rate per provider (Plaid, SnapTrade, Setu, SGFinDex, Tink, Basiq, Lean, CCXT)
- Crash rate per platform per app version
- Auto-rollback events (count, reason, recovery time)

### 15.6 AI Cost and Quality

- AI tokens consumed per tier per day
- Cost per active user per tier (this is the metric that determines tier margins)
- Anthropic prompt cache hit rate (target > 80%)
- Model routing distribution — % of requests served by small / Sonnet / Opus
- Average agent round-trip latency per tool
- Tool call distribution (which tools are hot)
- Hallucination rate (sampled audit — a fraction of agent responses are human-reviewed weekly)
- Memory store growth per user (gives a sense of long-term cost trajectory)

### 15.7 Sharia and Zakat (Gold-Specific)

- Sharia screenings run per day, with cache hit ratio
- Compliance status flips detected (and surfaced to affected users via notification)
- Zakat calculations performed per Ramadan period (the annual peak)
- Zakat donations facilitated through the platform — count, total amount, distribution by charity
- Scholarly school distribution among Gold users (informs roadmap)

### 15.8 Compliance

- GDPR / DPDP / CCPA data subject requests received (export, rectification, deletion)
- SLA compliance per request type (target: export within 30 days, deletion within 30 days)
- Right-to-delete completions
- Security audit findings open / closed
- Failed login attempts (broken down by suspected attacker vs legitimate user friction)
- Anomalous access patterns flagged

### 15.9 Architecture

- Admin endpoint set: `/v1/admin/metrics/...` with constant-time bearer auth (existing pattern). Per-metric endpoints scoped by metric category.
- Materialized views in Postgres for fast reads on aggregates. Refreshed every 5 minutes for hot metrics, hourly for cold metrics.
- Time-series data in TimescaleDB extension (added to Supabase Pro) for DAU / MRR / error rate time series.
- Front-end: separate React app at `admin.mizan.app`, deployed alongside the marketing site. Reuses the design system tokens — same Tailwind, same shadcn primitives, same Recharts vocabulary (donut + bar + sparkline + heatmap). Looks like Mizan, feels like a private NASA control panel.
- Real-time updates via Server-Sent Events for hot metrics (DAU counter, error rate ticker).
- Authorization: only team members in `team_members` with `role = 'admin'`. JWT-verified per the standard middleware.
- Audit log: every admin access logged in `admin_access_log` with user_id, endpoint, timestamp, IP, query parameters. Reviewable in the dashboard itself.

### 15.10 Alerting

The dashboard isn't passive. It alerts:

- Sentry error rate > 2x rolling 24h average → page on-call
- Performance budget breach sustained > 15 minutes → page on-call
- Sync provider success rate < 95% over rolling 1h → notify the team channel
- Churn spike > 2 standard deviations from rolling 30d average → notify the team channel
- Failed payment rate > 2x rolling 7d average → notify the team channel
- AI cost per user > $X budget threshold → notify product

Alerts are calibrated. Noisy alerts get tuned or removed within one cycle. The team's working agreement: every alert is actionable, or it's not an alert.

---

## 16. Documentation Requirements

- Every public crate has a `lib.rs` doc comment with purpose, key types, examples.
- Every Mizan Connect endpoint has an OpenAPI spec entry (auto-generated from handler signatures via `utoipa` or equivalent, manually verified).
- ADRs in `docs/adr/`. Numbered. Format: context / decision / consequences / alternatives. Every significant choice is captured. ADRs are reviewed annually for staleness.
- Runbooks in `docs/runbooks/`. One per operational task. Format: when to run, prerequisites, steps, verification, rollback. Reviewed annually.
- README per crate, focused on what the crate is responsible for and how to test it in isolation.

---

## 17. Anti-Patterns

Things to never do, even if tempted:

- **Don't store passwords, seed phrases, or private keys.** Doesn't matter how convenient. Not happening.
- **Don't log secrets.** `SecretString` exists for a reason. `tracing` macros never receive raw secrets. CI scans for this pattern.
- **Don't return raw provider data to the client.** Always normalize through the `ProviderInterface` shape. The client doesn't need to know Plaid's specific quirks.
- **Don't add a dependency without justification.** The dependency tree is reviewed quarterly. Every new crate needs to earn its place.
- **Don't comment out code.** Delete it. Git remembers.
- **Don't add a TODO without an issue.** It becomes archeology.
- **Don't bypass the AI Safety Runtime "just this once."** The runtime is the guardrail. It exists because hand-written guardrails get forgotten.
- **Don't add a chart type outside the vocabulary** (donut, bar, heatmap, sparkline, Sankey). Product approval required.
- **Don't ship without tests.** Coverage gates aren't suggestions.
- **Don't merge your own PR** (docs-only is the only exception).
- **Don't relax a security rule "temporarily."** Temporary becomes permanent.
- **Don't let `clippy` warnings accumulate.** Zero today, zero tomorrow.

---

## 18. References and Influences

The principles in this document are drawn from sources worth knowing:

- **Andrej Karpathy — "A Recipe for Training Neural Networks"** (karpathy.github.io/2019/04/25/recipe). Not literally about Mizan, but the philosophy is the same: most failures come from skipping the "boring" verification steps. Karpathy's recipe is *be paranoid; check the data; check the data again; visualize everything; start small; build up*. The QA Pass cadence and the golden test discipline are the Mizan equivalent.
- **Donald Knuth — Literate Programming.** The principle that code is read more than written. Comments explain *why*. Variable names describe intent. The reader's experience matters more than the writer's.
- **Stripe API Guidelines** (stripe.com/docs/api). The discipline of versioned APIs (`/v1/...`), idempotency keys on mutations, webhook signature verification with multi-secret rotation, request_id propagation through every log line. Mizan Connect inherits these patterns directly.
- **Linux Kernel Coding Style** (Documentation/process/coding-style.rst). The principle that style is enforced, not negotiated; that long-lived code is more important than clever code; that 8-space indentation forces you to reconsider deeply-nested logic.
- **MIT 6.031 — Software Construction** (web.mit.edu/6.031). The principles of immutability where possible, types as documentation, test-first when the spec is unclear, code review as collective ownership.
- **Daniel Bernstein — Cryptography engineering** (cr.yp.to). The principle that crypto code is the kind of code where "almost correct" means "broken." Constant-time comparison isn't a stylistic preference. Side-channel resistance isn't paranoia.
- **John Carmack — programming notes** (various). The principle that you understand what your code does, fully, or you don't ship it. No "this seems to work."
- **Anthropic — published guidance on agentic systems.** The principle that AI tools need explicit safety contracts (input bounds, output bounds, audit logs, undo-ability) and that the dispatcher is the place to enforce them, not the individual tool. The AI Safety Runtime is the Mizan implementation of this principle.
- **The 18 QA Passes themselves.** The deepest reference. Read `docs/qa-passes/` before changing anything in financial-truth, zakat, or the sync providers. Each pass left a permanent test and a permanent rule. Respect the scars.

---

## 19. The Working Agreement

You are working on a Rust-native, local-first, hash-chained wealth engine that real people will trust with millions of dollars across multiple jurisdictions, multiple currencies, multiple religious obligations, and multiple life stages. The bar is not "this works." The bar is "this works correctly, securely, performantly, and provably — and a year from now another engineer can verify all four."

You are not alone on this codebase. Every rule in this document is the result of someone before you finding a way the system could fail. Treat the rules as gifts.

When you find a rule that no longer makes sense, propose its update in an ADR. Don't silently work around it. When you find a violation in existing code, fix it as you encounter it, even if it isn't yours. The codebase is collectively owned.

When you are stuck, ask. When you are uncertain about whether something is correct, write a test. When the test is unclear, write the spec. When the spec is unclear, write an ADR.

The standard is the most advanced, most user-trustworthy, most internally consistent AI-native fintech application that has ever shipped. Every commit either gets us closer to that standard or it doesn't merge.

Ship it well.

---

*Working agreement v1.0 — April 2026. Update via ADR in `docs/adr/`. This file is the source of truth.*
