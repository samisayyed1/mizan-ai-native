//! `/api/v1/...` legacy Connect-API stubs.
//!
//! Active live sync now lives under [`crate::plaid::router`], and the
//! `/subscription/plans` stub has been replaced by the real handler in
//! [`crate::billing::handlers::list_plans`] (mounted via
//! [`crate::billing::plans_router`]).
//!
//! Module path kept so legacy imports don't break; router is intentionally
//! empty.

pub mod handlers;

use axum::Router;

use crate::state::AppState;

/// Empty router. Kept so callers that still reference it merge a no-op.
pub fn router() -> Router<AppState> {
    Router::new()
}
