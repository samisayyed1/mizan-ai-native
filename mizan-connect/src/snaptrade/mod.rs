//! SnapTrade brokerage integration — the second live-sync boundary
//! after Plaid. Mirrors the Plaid module's shape:
//!
//! - `client`  — typed HTTP client with HMAC-SHA256 request signing
//! - `config`  — extension of `mizan_connect::config::Config`
//! - `repository` — Postgres helpers (uses existing `broker_connections`)
//! - `types`   — request/response DTOs
//! - `handlers` — Axum routes mounted under `/v1/sync/snaptrade/*`
//!
//! WHY A SEPARATE MODULE (vs. just extending Plaid):
//!
//! - SnapTrade's auth model is different — every request carries a
//!   `Signature` header derived from the consumer key, instead of
//!   Plaid's `secret` body field.
//! - SnapTrade requires a per-user registration step that returns a
//!   `userSecret` we must store encrypted forever; Plaid hands you a
//!   long-lived access token after Link completes.
//! - The Mizan domain model is the same on the downstream side
//!   (accounts + holdings + activities), so we map both providers
//!   into the same target schema — but the *upstream* shape is
//!   vendor-specific.

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub mod client;
pub mod handlers;
pub mod repository;
pub mod signing;
pub mod types;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sync/snaptrade/health", get(handlers::health))
        .route(
            "/sync/snaptrade/login-portal",
            post(handlers::create_login_portal),
        )
        .route("/sync/snaptrade/connections", get(handlers::list_connections))
        .route(
            "/sync/snaptrade/connections/{authorization_id}",
            delete(handlers::disconnect_connection),
        )
        .route("/sync/snaptrade/accounts", get(handlers::list_accounts))
        .route(
            "/sync/snaptrade/accounts/{account_id}/positions",
            get(handlers::list_positions),
        )
        .route(
            "/sync/snaptrade/accounts/{account_id}/activities",
            get(handlers::list_activities),
        )
        .route("/sync/snaptrade/sync", post(handlers::sync_now))
}
