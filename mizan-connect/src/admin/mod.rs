//! Admin endpoints — operator + QA surface.
//!
//! Mounted at `/v1/admin/*`. Each handler validates `Authorization: Bearer
//! <token>` against `MIZAN_ADMIN_TOKEN` (constant-time compare). Returns
//! 503 if the env var is unset, so accidentally-exposed routes can't be
//! used in environments where the admin surface wasn't deliberately turned
//! on. 401 on missing / wrong token.
//!
//! These exist for two real needs:
//!  1. **Operator emergencies** — Stripe webhook can't land (broken signing
//!     secret, prod DB lock, ...) and a paying customer is stuck on Free.
//!     Ops can bump their tier by hand without redeploy or psql.
//!  2. **QA / smoke tests** — being able to verify the cloud's view of a
//!     specific user's state (subscription, team, customer id) without
//!     standing up psql against Supabase Postgres.

pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// `/v1/admin/*` router. Always mounts; each handler self-checks the
/// admin token so an env-unset deployment 503s the surface rather than
/// quietly serving it.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/user/:user_id", get(handlers::get_user_state))
        .route(
            "/admin/user/:user_id/subscription",
            post(handlers::force_grant_subscription),
        )
}
