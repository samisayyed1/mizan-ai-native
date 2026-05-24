# mizan-ai-native

AI-native personal wealth tracking for Muslims. Two apps in one repo:

- **mizan-4** — the desktop app (Tauri + React + Rust). What end-users install.
- **mizan-connect** — the backend (Rust / Axum, deployed to Fly.io). Plaid Gold sync, auth, billing, AI proxy.

The product ships two plans only:

| Plan   | What you get                                                                                          |
| ------ | ----------------------------------------------------------------------------------------------------- |
| Silver | Private, local-first AI wealth tracking. CSV/file ingestion, chat-driven asset creation, alternative assets, encrypted local storage, zakat. |
| Gold   | Everything in Silver, plus Plaid live sync — liabilities, investments, holdings, background monitoring, allocation/cash-drag detection, weekly AI summaries, proactive alerts. |

There is no free tier and no SnapTrade/yfinance product path.

## Repo layout

```
.
├── mizan-4/          desktop app
│   ├── apps/         frontend (React) + tauri host + server (web mode)
│   ├── crates/       core / storage / market-data / connect-client Rust crates
│   └── ...
├── mizan-connect/    backend service
│   ├── src/          Axum router, Plaid, billing, auth, telemetry
│   ├── migrations/   sqlx Postgres migrations
│   ├── fly.toml      Fly.io deploy config
│   └── Dockerfile    multi-stage build, distroless runtime
└── .github/workflows/
    ├── ci.yml                 monorepo CI (frontend + Rust crates + Rust server)
    └── deploy-mizan-connect.yml  pushes mizan-connect to Fly on main
```

## Development

### Desktop (mizan-4)

```sh
cd mizan-4
pnpm install
pnpm --filter frontend type-check
pnpm --filter frontend test -- --run
pnpm tauri dev       # full desktop app against CONNECT_API_URL
```

### Backend (mizan-connect)

```sh
cd mizan-connect
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib

# Local run against a dev Postgres:
DATABASE_URL=postgres://... \
SUPABASE_URL=https://...supabase.co \
MIZAN_PLAID_TOKEN_ENCRYPTION_KEY=$(openssl rand -base64 32) \
cargo run
```

### Deploying mizan-connect

Production target: `mizan-connect.fly.dev` with Postgres app `mizan-connect-db`.

```sh
cd mizan-connect
fly deploy --remote-only
fly logs                 # tail
fly secrets list         # confirm required secrets are set
```

Required Fly secrets:

- `DATABASE_URL`
- `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`
- `PLAID_CLIENT_ID`, `PLAID_SECRET`, `PLAID_ENV`, `PLAID_REDIRECT_URI`
- `MIZAN_PLAID_TOKEN_ENCRYPTION_KEY` (base64 of 32 random bytes)
- `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET` (optional — endpoints 501 without them)
- `OPENAI_API_KEY` (optional — AI proxy disabled without it)
- `SENTRY_DSN` (optional)

## Security notes

- Plaid access tokens are encrypted with AES-256-GCM via `MIZAN_PLAID_TOKEN_ENCRYPTION_KEY`. Lose the key, lose all live sync.
- Plaid webhooks are signature-verified (ES256 JWT) before any body is persisted. See `mizan-connect/src/plaid/webhook_verifier.rs`.
- Manual `/api/v1/sync/plaid/sync` has a 60s per-(user, item) cooldown.
- Desktop encrypts local SQLite when configured; the assistant never sends raw financial blobs to OpenAI.
