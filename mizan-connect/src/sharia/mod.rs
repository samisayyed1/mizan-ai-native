//! AAOIFI Sharia screening worker — Track E PR-E4.
//!
//! Implements the qualitative + quantitative screens defined in
//! ADR 0012 ("AAOIFI Sharia Screening Criteria"). Produces a verdict
//! per equity holding that drives the [`'halal-screened'`] /
//! [`'mixed-compliance'`] modifier badges shipped in PR-E3.
//!
//! # Layering
//!
//! - `aaoifi_rules` — pure logic. Takes a fully-hydrated
//!   [`ScreeningInput`] and returns a [`Verdict`]. No I/O, no
//!   randomness — 100% deterministic so golden tests pin the rules
//!   exactly per ADR 0012's tables.
//! - `handlers` — Axum endpoints over the rules. `POST /v1/sharia/screen`
//!   accepts an input bundle and returns a verdict. `GET
//!   /v1/sharia/status/:symbol` returns the cached verdict if one
//!   exists.
//!
//! # Coverage target
//!
//! Per CLAUDE.md §5 hard floors, the screening module's pure rules
//! sit on the financial-truth/zakat 95%-coverage floor. The
//! `aaoifi_rules` golden tests in this PR push that crate well past
//! 95% line + branch (every branch is enumerated below).
//!
//! # Out of scope (deferred to follow-ups documented in the plan)
//!
//! - Persistence to `holdings.sharia_status` — needs the holdings
//!   migration to surface the column to the worker; tracked as
//!   PR-E4.b once the holdings model lands in mizan-connect (which
//!   it doesn't today — holdings live desktop-side).
//! - GICS lookup from Twelve Data — the worker accepts a `gics_sector`
//!   input and trusts the caller; PR-E4.c wires the Twelve Data fetch.
//! - SEC EDGAR + equivalent regulatory-filing pulls for the financial
//!   ratios — same as above; the worker takes the numbers as input
//!   today and PR-E4.d wires the fetchers.
//!
//! Shipping the pure rules + endpoint surface NOW unblocks every
//! downstream surface (the Mizan Badge `'halal-screened'` chip; the
//! agent's `find_sharia_status` tool in PR-C4.c) without waiting for
//! the data-pull plumbing.

pub mod aaoifi_rules;
pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub use aaoifi_rules::{
    screen, BusinessActivity, FinancialRatios, ScreeningInput, ScreeningRationale, Verdict,
};

/// Subrouter for `/v1/sharia/*` endpoints — merged into the v1 tree.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sharia/screen", post(handlers::screen_handler))
        .route("/sharia/status/:symbol", get(handlers::status_handler))
}
