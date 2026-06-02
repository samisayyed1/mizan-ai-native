//! Liveness (`/health`) and readiness (`/ready`) endpoints.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::state::AppState;

/// `/health` payload.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    build_time: &'static str,
    commit_sha: &'static str,
}

/// `/ready` payload — surfaces dependency state to operators.
///
/// Plaid + Stripe are reported as "configured" or "not configured" (per
/// the Mizan Connect Environment Matrix in v3.1: both are optional —
/// running without them is a supported deployment mode for Free-tier-
/// only instances). They never block readiness; the field is reported
/// so a release engineer can spot a "expected Plaid prod, didn't get
/// it" misconfiguration in one glance.
#[derive(Debug, Serialize)]
struct ReadyResponse {
    status: &'static str,
    db: ComponentStatus,
    jwks: ComponentStatus,
    plaid: ComponentStatus,
    billing: ComponentStatus,
}

#[derive(Debug, Serialize)]
struct ComponentStatus {
    healthy: bool,
    /// Optional one-line cause. Free-form — for support diagnostics only.
    detail: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: crate::VERSION,
        build_time: crate::BUILD_TIME,
        commit_sha: crate::GIT_COMMIT,
    })
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    // DB ping with a short timeout so a hung pool doesn't pin the request.
    let db_status = match tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(state.db()),
    )
    .await
    {
        Ok(Ok(_)) => ComponentStatus {
            healthy: true,
            detail: None,
        },
        Ok(Err(err)) => ComponentStatus {
            healthy: false,
            detail: Some(format!("query: {err}")),
        },
        Err(_) => ComponentStatus {
            healthy: false,
            detail: Some("timed out after 2s".into()),
        },
    };

    let snapshot = state.jwks().snapshot();
    let jwks_status = if snapshot.key_count > 0 && !snapshot.stale {
        ComponentStatus {
            healthy: true,
            detail: None,
        }
    } else {
        ComponentStatus {
            healthy: false,
            detail: snapshot.last_error.or_else(|| {
                Some(if snapshot.key_count == 0 {
                    "no keys cached".into()
                } else {
                    "cache stale".into()
                })
            }),
        }
    };

    // Plaid + Billing are *informational* — a Free-tier-only deployment
    // is a supported mode, so missing config is reported but never
    // flips the overall readiness flag.
    let plaid_status = if state.plaid().is_some() {
        ComponentStatus {
            healthy: true,
            detail: None,
        }
    } else {
        ComponentStatus {
            healthy: true,
            detail: Some("not configured (PLAID_CLIENT_ID / PLAID_SECRET unset)".into()),
        }
    };

    let billing_status = if state.billing().is_some() {
        ComponentStatus {
            healthy: true,
            detail: None,
        }
    } else {
        ComponentStatus {
            healthy: true,
            detail: Some("not configured (STRIPE_SECRET_KEY / STRIPE_WEBHOOK_SECRET unset)".into()),
        }
    };

    // Only DB + JWKS gate readiness. Plaid + billing are informational.
    let healthy = db_status.healthy && jwks_status.healthy;
    let body = ReadyResponse {
        status: if healthy { "ready" } else { "not_ready" },
        db: db_status,
        jwks: jwks_status,
        plaid: plaid_status,
        billing: billing_status,
    };

    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(body))
}
