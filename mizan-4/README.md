<div align="center">
  <img src="apps/frontend/public/logo.svg" alt="Mizan" width="80" height="80">

  <h3 align="center">Mizan</h3>

  <p align="center">
    A beautiful, private desktop investment tracker. Local-first. No cloud.
    <br />
    <br />
    <a href="https://mizan-landing-rho.vercel.app">Website</a>
    ·
    <a href="https://github.com/samisayyed1/mizan-4/releases/latest">Download</a>
    ·
    <a href="https://github.com/samisayyed1">Author</a>
  </p>
</div>

## About

**Mizan** is a private, local-first desktop investment tracker — no
subscriptions, no cloud, no data leaves your machine. Built and maintained by
**[Sami Sayyed](https://github.com/samisayyed1)** as an opinionated take on what
a portfolio app should be: fast, honest with numbers, and yours alone.

### What's inside

- **Decimal-precision pipeline end-to-end** — money values stay exact from typed
  input → SQLite storage → IPC → display. Sub-cent crypto, satoshi-scale
  quantities, and FX-converted multi-account totals are bit-exact across the
  app, with regression tests pinning the contract in CI.
- **Mizan Connect** — broker auth flow, sync state machine, recovery paths for
  partial / interrupted imports.
- **Addon SDK** — TypeScript-first, permissioned data access, hot reload, signed
  manifest, secrets vault.
- **Hardening sweeps** — chart period filters that snap to day boundaries,
  mutation toasts that surface real backend errors instead of generic "something
  went wrong", aria-labels on every icon-only button, plugin registration
  failures logged instead of swallowed, no double-toasts on activity-save
  errors, and more.
- **Device sync** — end-to-end encrypted, peer-to-peer; your phone and your
  laptop talk directly to each other, never through a server.

A focused changelog of every shipped improvement lives in
[CHANGES.md](./CHANGES.md).

## ✨ Key Features

- **📊 Portfolio Tracking** — multi-account, multi-asset (stocks, ETFs, crypto,
  options, alternative assets like property and vehicles)
- **📈 Performance Analytics** — TWR, MWR, simple return, max drawdown,
  volatility
- **💰 Activity Management** — buy / sell / dividend / transfer / split / fee /
  interest / tax, CSV import with precision-preserving parsing
- **🎯 Goal Planning** — retirement (FIRE), save-up, allocation targets
- **🔒 Local Data** — SQLite database on your device. Optional E2E-encrypted
  peer-to-peer sync between your own devices.
- **🧩 Extensible** — open Addon SDK with permissioned host APIs
- **🤖 Optional AI Assistant** — supports Ollama (free, local), OpenAI,
  Anthropic, Google, Groq, OpenRouter. You bring the key, or run locally for
  free.
- **🌍 Multi-Currency** — Decimal math across FX, configurable base currency
- **📱 Cross-Platform** — macOS, Windows, Linux desktop builds; server mode for
  headless / web use

## 🧩 Addon System

Mizan ships an addon system so the app can be extended without forking:

- **🔌 Easy Development** — TypeScript SDK with full type safety and hot reload
- **🔒 Secure** — explicit permission system with user consent at install
- **⚡ High Performance** — optimised for speed with minimal overhead
- **🎨 UI Integration** — custom pages, navigation items, and components
- **📡 Real-time Events** — portfolio updates, market sync, and user actions
- **🗄️ Full Data Access** — accounts, holdings, activities, and market data
  (subject to granted permissions)
- **🔐 Secrets Management** — secure storage for API keys and sensitive data

**Get started building addons:** [Addon Documentation Hub](docs/addons/index.md)

## Roadmap

See [ROADMAP.md](./ROADMAP.md).

## Getting Started

### Install (recommended)

Grab the installer for your OS from the
**[latest release](https://github.com/samisayyed1/mizan-4/releases/latest)**:

- **macOS (Apple Silicon)** — `Mizan_*_aarch64.dmg`
- **Windows (x64)** — `Mizan_*_x64_en-US.msi`
- **Linux (x64)** — `Mizan_*_amd64.AppImage`
- **Linux server / web mode** — `mizan-server-*-linux-amd64.tar.gz`

### Build from source

Prerequisites: [Node.js](https://nodejs.org/), [pnpm](https://pnpm.io/),
[Rust](https://www.rust-lang.org/), [Tauri](https://tauri.app/).

```bash
git clone https://github.com/samisayyed1/mizan-4.git
cd mizan-4
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # production binary
```

## Folder Structure

```
mizan/
├── apps/                        # Application packages
│   ├── frontend/                # React frontend application
│   ├── tauri/                   # Tauri desktop app (Rust IPC commands)
│   └── server/                  # Axum HTTP server for web mode
├── crates/                      # Rust crates (shared backend logic)
│   ├── core/                    # Core business logic, services, models
│   ├── ai/                      # AI assistant (rig-core, multi-provider)
│   ├── storage-sqlite/          # SQLite storage layer (Diesel ORM)
│   ├── market-data/             # Market data providers
│   ├── connect/                 # External service integrations
│   └── device-sync/             # End-to-end encrypted peer sync
├── addons/                      # Example addons
├── packages/                    # Shared TypeScript packages
│   ├── ui/                      # Component library
│   └── addon-sdk/               # Addon developer SDK
└── docs/                        # Documentation
```

## Contributing

Issues and pull requests welcome.

## License & Attribution

Mizan is licensed under **AGPL-3.0**. See [LICENSE](./LICENSE) for the full
text.

This project began as a fork of
[Wealthfolio](https://github.com/afadil/wealthfolio) by Teymz Inc., also
licensed under AGPL-3.0. The Mizan-specific work — the Decimal precision
pipeline, Mizan Connect, the extended addon SDK, end-to-end device sync, the AI
assistant integration, the regression-guard test suite, the release pipeline,
the landing page — is original to this repository. "Wealthfolio" and the
Wealthfolio logo are trademarks of Teymz Inc. and are not used by this fork. See
[NOTICE](./NOTICE) and [CHANGES.md](./CHANGES.md) for the full provenance
breakdown.
