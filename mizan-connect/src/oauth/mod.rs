//! OAuth connector framework — Track J PR-J1 / Goal v3 §V Phase 10.
//!
//! Per ADR 0025 and the autonomous-loop directive
//! `Mizan_Continue_Autonomous_v3.md` lines 71-78: a generic OAuth
//! connector catalog so the desktop can light up Google Drive
//! (statement folder ingestion), Notion (Mizan-designated goals DB),
//! Slack (notification channel), GitHub (equity comp tracking),
//! Apple/Google/Outlook Calendar (deadline surfacing), and Zapier
//! (automation bridge) — without one bespoke flow per provider.
//!
//! # Scope
//!
//! PR-J1 (this PR) ships the **catalog + types** layer:
//!
//! - `Provider` enum + `ProviderDescriptor` const catalog
//! - `AuthorizationRequest` (builds the OAuth provider URL)
//! - `TokenSet` (the access + refresh + expiry triple)
//! - `OAuthScopes` per-provider scope enum
//! - No DB writes, no Stripe-like Connect Account setup — all the
//!   provider-specific URLs + scope lists hard-coded here so PR-J1.b
//!   can wire endpoints without re-deriving them
//!
//! PR-J1.b adds:
//! - `POST /v1/oauth/connect/{provider}` — issue authorization URL
//! - `GET /v1/oauth/callback/{provider}` — exchange code for tokens
//! - `POST /v1/oauth/disconnect/{provider}` — revoke + delete row
//! - `GET /v1/oauth/connections` — list user's connected providers
//! - Token vaulting via SecretCipher (the existing token-encryption
//!   pattern used for Plaid + SnapTrade)
//! - Background refresh worker (PR-J1.c) that refreshes tokens within
//!   the next 24h window
//!
//! # Security discipline (CLAUDE.md §16)
//!
//! - Tokens never logged in plaintext
//! - Tokens encrypted at rest via per-provider encryption keys
//!   (OAUTH_TOKEN_ENCRYPTION_KEY env, audited in PR-PROD-1)
//! - `state` param on the OAuth flow is HMAC-signed by Mizan Connect
//!   to prevent CSRF
//! - Scope list capped at the minimum required for each integration's
//!   stated use case
//!
//! # Out of scope (deferred)
//!
//! - PR-J1.b: endpoint handlers + DB row writes
//! - PR-J1.c: background refresh worker
//! - PR-J1.d: per-provider statement-ingestion / DB-fetch / channel-
//!   delivery integration code

pub mod catalog;
pub mod types;

pub use catalog::{provider_descriptor, ProviderDescriptor, PROVIDERS};
pub use types::{AuthorizationRequest, OAuthError, OAuthScopes, Provider, TokenSet};
