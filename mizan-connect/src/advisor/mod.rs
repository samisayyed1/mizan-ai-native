//! Track G — Advisor → Client linking + sign-off surface.
//!
//! Per spec §13.5 (Advisor tier) + migration 0014, an advisor can
//! see the balance sheets of clients who have explicitly granted
//! access. The link is client-issued (the advisor cannot create one
//! unilaterally), time-limited, revocable, and scope-gated
//! (`read_only` vs `read_write`).
//!
//! # Surface (this PR — PR-G5.b)
//!
//! Client side:
//! - `POST   /v1/advisor/links` — client issues a grant to an advisor;
//!   response includes the raw token once (then never again)
//! - `GET    /v1/advisor/grants` — client lists their outstanding grants
//! - `DELETE /v1/advisor/links/:id` — client revokes a grant
//!
//! Advisor side:
//! - `GET    /v1/advisor/clients` — advisor lists their active client grants
//! - `GET    /v1/advisor/clients/:id/sign-offs` — advisor reads the
//!   sign-offs they've logged on a client
//! - `POST   /v1/advisor/sign-offs` — advisor signs off on a holding;
//!   surfaces as the 'advisor-reviewed' Mizan Badge modifier
//! - `POST   /v1/advisor/sign-offs/:id/retract` — advisor retracts a sign-off
//!
//! Every advisor action writes to:
//!   - `advisor_access_log` (per-action context JSON)
//!   - `audit_log` (cross-system security event trail)
//!
//! # Read-only portfolio view
//!
//! Once the desktop wires the advisor's client picker, the
//! portfolio-read endpoint pulls the client's holdings via the
//! existing user-facing query path — scoped by the advisor link's
//! `scope` field. That endpoint (`GET /v1/advisor/clients/:id/portfolio`)
//! lands in PR-G5.c once the client portfolio API is stabilised; this
//! PR ships the link + sign-off scaffolding so the desktop can
//! render the client list immediately.

pub mod handlers;
pub mod repository;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // Client side
        .route("/advisor/links", post(handlers::issue_grant))
        .route("/advisor/grants", get(handlers::list_my_grants))
        .route("/advisor/links/:id", delete(handlers::revoke_grant))
        // Advisor side
        .route("/advisor/clients", get(handlers::list_my_clients))
        .route(
            "/advisor/clients/:client_id/sign-offs",
            get(handlers::list_sign_offs_for_client),
        )
        .route("/advisor/sign-offs", post(handlers::create_sign_off))
        .route(
            "/advisor/sign-offs/:id/retract",
            post(handlers::retract_sign_off),
        )
}
