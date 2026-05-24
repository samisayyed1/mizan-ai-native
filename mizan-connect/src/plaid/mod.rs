//! Plaid-first live sync for Mizan Gold.
//!
//! This module replaces the old multi-vendor broker surface with a single
//! Plaid boundary: Link token creation, public-token exchange, encrypted
//! access-token storage, cursor-based transaction sync, liabilities,
//! investments/holdings, and webhooks.

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub mod client;
pub mod handlers;
pub mod repository;
pub mod types;
pub mod webhook_verifier;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sync/plaid/health", get(handlers::health))
        .route("/sync/plaid/link-token", post(handlers::create_link_token))
        .route(
            "/sync/plaid/exchange-public-token",
            post(handlers::exchange_public_token),
        )
        .route("/sync/plaid/connections", get(handlers::list_connections))
        .route(
            "/sync/plaid/connections/{item_id}",
            delete(handlers::disconnect_connection),
        )
        .route("/sync/plaid/accounts", get(handlers::list_accounts))
        .route("/sync/plaid/sync", post(handlers::sync_now))
        .route("/sync/plaid/webhook", post(handlers::webhook))
}
