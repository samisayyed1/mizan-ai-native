# Mizan Connect — Backend

Operating manual for Claude Code. Auto-loaded for any session inside
`mizan-ai-native/mizan-connect/`. **Read this fully before making any
change.** The root operating manual (`@../CLAUDE.md`) and the binding
spec (`@../MIZAN_AI_NATIVE_PLAN.md`) load alongside this one.

> **Mizan Connect is part of the product, not infrastructure hidden
> behind it.** It owns identity, billing, entitlements, managed AI,
> Plaid, SnapTrade, sync run ledger, and the production gates.
> See `@../MIZAN_AI_NATIVE_PLAN.md` "Mizan Connect End-to-End Backend
> Contract" — that contract is binding.

## Release sequencing lock

No production credentials until v3 Phase N validation passes. P1–P4
(live Stripe, live Plaid, live SnapTrade, signed distribution) are
locked behind the sandbox MVP gate. The root `.claude/settings.json`
hook blocks `fly deploy`, `stripe live`, `git push`, and `gh pr merge`
unless `MIZAN_ALLOW_PRODUCTION=1` is exported.

## Tier model

Free / Silver / Gold. Capabilities and entitlements in v3 §4 + §Mizan
Connect `/v1/me` contract. The 4-tier model (Basic/Pro/Enterprise) is
retired — migrations must map old → new at the `/v1/me` boundary.

## What this is

Mizan Connect is the **proprietary closed-source backend** for Mizan AI,
the desktop wealth OS. Desktop is AGPL; this backend is NOT open source.
Do not add AGPL/GPL/copyleft dependencies without explicit approval.

Mizan Connect handles: user auth (via Supabase as IdP), subscription billing
(Stripe), brokerage sync (SnapTrade), and E2EE device sync.

## Build status (chunk history — historical)

- Chunk 1: foundation + Supabase JWT auth ✓ shipped
- Chunk 2: `/api/v1/...` aliases + 501 stubs + desktop wiring ✓ shipped
- Chunk 3: SnapTrade integration ✓ shipped
- Chunk 4: Stripe billing ✓ shipped
- Chunk 5: background sync (Redis rate limit + cron) — pending v3 Phase G
- Chunk 6: E2EE device sync — pending v3 Phase G

**v3 ordering supersedes the chunk plan.** The remaining work is sequenced
by `MIZAN_AI_NATIVE_PLAN.md` Phase 0 → N. Chunk numbers stay useful for
module-naming continuity only.

## Module map (post-Chunk 3)

- `src/auth/` — Supabase JWKS + JWT verification, `AuthenticatedUser` extractor.
- `src/users/` — `/v1/me` and `/api/v1/user/me`.
- `src/connect/` — `/api/v1/subscription/plans` 501 stub (Chunk 4 territory).
- `src/snaptrade/` — broker integration. See [`signing.rs`](src/snaptrade/signing.rs)
  doc-comment for the canonical-request format (cited from the official Ruby SDK).
  - `signing.rs`: HMAC-SHA256 signer; frozen-vector unit test pins the canonical form.
  - `encryption.rs`: AES-256-GCM for SnapTrade `userSecret` at rest; startup self-test.
  - `state_token.rs`: HS256 JWT bound to local user (10-min TTL).
  - `client.rs`: typed reqwest wrapper; per-request signed.
  - `rate_limit.rs`: per-user 10/hour bucket on top of `tower_governor`.
  - `repository.rs`: SQLx queries on `broker_connections`.
  - `handlers.rs`: 8 endpoints; `snaptrade_callback` is the only public route in
    the module.

## Required env vars (Chunk 3)

| Var                                  | Required when     | Notes                                      |
| ------------------------------------ | ----------------- | ------------------------------------------ |
| `SNAPTRADE_CLIENT_ID`                | `APP_ENV != test` | from SnapTrade dashboard                   |
| `SNAPTRADE_CONSUMER_KEY`             | `APP_ENV != test` | secret; never logged                       |
| `SNAPTRADE_API_BASE`                 | always            | default `https://api.snaptrade.com/api/v1` |
| `SNAPTRADE_REDIRECT_URI`             | `APP_ENV != test` | must be whitelisted in SnapTrade dashboard |
| `MIZAN_BROKER_SECRET_ENCRYPTION_KEY` | `APP_ENV != test` | base64; decode → exactly 32 bytes          |
| `MIZAN_SNAPTRADE_STATE_SECRET`       | `APP_ENV != test` | base64; decode → ≥ 32 bytes                |

## Things to know operationally

- **Sandbox vs production**: SnapTrade does NOT use a separate sandbox host. Sandbox
  keys hit `api.snaptrade.com` exactly as production keys do; the difference is
  the connection limit (~5) and that some institutions return mock data only.
- **Rate limiter scope**: `/login-portal` is rate-limited 10/hour per local user
  via an in-memory `HashMap<Uuid, Vec<OffsetDateTime>>` in `AppState`. This is
  single-instance only. **Chunk 5 prereq**: move to Redis (or `tokio` channel +
  Postgres) before scaling beyond one Fly machine.
- **Audit events**: `broker.connect.{initiated,completed,failed}`,
  `broker.disconnect`, `broker.refresh`. `event_data` JSONB never includes the
  SnapTrade `userSecret`.
- **Frozen-signature test**: `src/snaptrade/signing.rs::frozen_vector_locks_canonical_form`
  pins one input → expected base64 signature. If it ever fails, the canonical
  request shape (or serde_json's Map order, or the HMAC crate behavior) changed —
  audit the diff before silencing.

## Architecture invariants (NEVER violate these)

1. **Supabase is the IdP.** We never store passwords. JWTs verified server-side via JWKS.
2. **SQLx compile-time checked queries.** After any SQL change run `cargo sqlx prepare` and commit `sqlx-data.json`.
3. **Zero `unwrap()`/`expect()`** in production code paths. Tests can use `expect("clear reason")`.
4. **Zero `println!` / `eprintln!`.** Always `tracing::{info,warn,error,debug}!`.
5. **Datetimes:** `time::OffsetDateTime` only. Never `chrono`, never `std::time::SystemTime` for wall-clock.
6. **Errors:** `thiserror` for domain enums; `anyhow` only allowed in `main.rs` startup. Every error response includes a `request_id`.
7. **Money:** never `f64`. Use `i64` cents or `rust_decimal::Decimal`.
8. **Secrets:** never logged. The tracing layer must redact `Authorization`, `Cookie`, `X-Stripe-Signature` headers.
9. **Public API responses:** explicit DTO types with `#[derive(serde::Serialize)]`. Never serialize domain models directly.
10. **CORS:** allowlist from env. Never `*` in production. Validation fails startup if `*` set with auth enabled.
11. **Async boundaries:** all I/O is async. No `std::fs`, no `reqwest::blocking`. Use `tokio::fs`, async `reqwest`.
12. **Tests are first-class.** Every handler has at least one happy-path and one error-path test using testcontainers.

## Conventions

- **Module layout:** `mod.rs` re-exports public surface; `model.rs` types; `repository.rs` SQL; `handlers.rs` Axum handlers.
- **Naming:** snake*case files/modules, PascalCase types, SCREAMING_SNAKE for env vars (prefix `MIZAN*`for app-specific, plain names for standards like`DATABASE_URL`).
- **API versioning:** all routes under `/v1/`. Health/ready unversioned.
- **Request IDs:** every request gets `X-Request-Id` (UUID v4 if absent). Echo on response. Include in tracing span and error JSON.
- **Pagination:** cursor-based, never offset. `?cursor=...&limit=50`. Default limit 25, max 100.
- **Validation:** every request body wrapped in `Json<T>` where `T: validator::Validate`.

## Common commands

```bash
make dev              # docker compose up + cargo run with watch
make test             # cargo test --workspace
make lint             # cargo fmt --check && cargo clippy -- -D warnings
make migrate          # sqlx migrate run
make sqlx-prepare     # cargo sqlx prepare (run after SQL changes)
make deploy           # fly deploy
```

## When adding code, FIRST consult these skills

Skills live in the monorepo root (`@../.claude/skills/`). Relevant ones:

- New SQLite (desktop) or Postgres (cloud) migration → `mizan-migration-author`
- New tier-gated capability → `mizan-tier-gate`
- Plaid sandbox setup → `plaid-sandbox-test`
- SnapTrade sandbox setup → `snaptrade-sandbox-test`
- Stripe test mode flow → `stripe-test-mode`
- Before commit/push → `mizan-pr-checklist`
- Touching `/v1/me` or any entitlement boundary → `ai-truth-contract`

## Things to ASK the user about (don't guess)

- Adding new dependencies (especially anything copyleft)
- Database schema changes that aren't pure additions
- Anything touching billing, encryption keys, or auth flow
- New external service integrations
- Production deploys

## Off-limits without explicit approval

- Changing the auth model (Supabase IdP)
- Changing the DB driver (SQLx)
- Adding ORMs (Diesel, SeaORM, Prisma — none of them)
- Storing payment card data (PCI scope = no)
- Storing brokerage credentials (SnapTrade does this — never us)
- Logging or returning full JWTs, refresh tokens, Stripe secrets, or SnapTrade `userSecret`
