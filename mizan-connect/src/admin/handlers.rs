//! Handlers for `/v1/admin/*`.
//!
//! See [`super`] for context.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

/// Constant-time bearer-token check. Returns Ok on success, or an
/// `AppError` representing the right HTTP status:
///  - 503 when no admin token is configured (env unset).
///  - 401 when the header is missing / malformed / wrong.
fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let configured = state.config().admin_token.as_ref().ok_or_else(|| {
        AppError::not_implemented("admin surface is disabled — set MIZAN_ADMIN_TOKEN to enable")
    })?;

    let presented = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthorized("missing Authorization: Bearer <token>"))?;

    // Constant-time compare so timing leaks can't be used to brute-force.
    use subtle::ConstantTimeEq;
    let expected = configured.expose_secret();
    if presented.as_bytes().ct_eq(expected.as_bytes()).into() {
        Ok(())
    } else {
        Err(AppError::unauthorized("invalid admin token"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /v1/admin/user/:user_id
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UserStateDto {
    pub user_id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub team_id: Option<Uuid>,
    pub team_name: Option<String>,
    pub subscription: Option<SubscriptionDto>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionDto {
    pub stripe_customer_id: Option<String>,
    pub stripe_subscription_id: Option<String>,
    pub tier: String,
    pub status: String,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
}

pub async fn get_user_state(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<UserStateDto>, AppError> {
    require_admin(&state, &headers)?;

    // One round-trip to Postgres pulling the user, their (maybe-existing)
    // solo team, and their MOST-RECENT subscription. We deliberately use
    // a subquery for the latest sub instead of LEFT JOIN + LIMIT 1
    // because LIMIT-1 over a join is non-deterministic when there are
    // multiple subscription rows for one user (a real possibility:
    // earlier incomplete row + later active row from a Stripe upgrade).
    // ORDER BY updated_at DESC NULLS LAST guarantees we surface the
    // newest, which is what callers actually want to see.
    let row = sqlx::query(
        r#"
        SELECT
            u.id            AS user_id,
            u.email         AS email,
            u.display_name  AS display_name,
            t.id            AS team_id,
            t.name          AS team_name,
            s.stripe_customer_id     AS stripe_customer_id,
            s.stripe_subscription_id AS stripe_subscription_id,
            s.tier::text             AS tier,
            s.status::text           AS status,
            s.current_period_end     AS current_period_end,
            s.cancel_at_period_end   AS cancel_at_period_end
        FROM users u
        LEFT JOIN teams t ON t.id = u.id
        LEFT JOIN LATERAL (
            SELECT *
            FROM subscriptions
            WHERE user_id = u.id
            ORDER BY updated_at DESC NULLS LAST, created_at DESC NULLS LAST
            LIMIT 1
        ) s ON true
        WHERE u.id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(state.db())
    .await?;

    let row = row.ok_or_else(|| AppError::not_found("user not found"))?;

    let subscription = row
        .try_get::<Option<String>, _>("tier")
        .ok()
        .flatten()
        .map(|tier| {
            let status: String = row.try_get("status").unwrap_or_default();
            let stripe_customer_id: Option<String> =
                row.try_get("stripe_customer_id").ok().flatten();
            let stripe_subscription_id: Option<String> =
                row.try_get("stripe_subscription_id").ok().flatten();
            let current_period_end: Option<time::OffsetDateTime> =
                row.try_get("current_period_end").ok().flatten();
            let cancel_at_period_end: bool = row.try_get("cancel_at_period_end").unwrap_or(false);
            SubscriptionDto {
                stripe_customer_id,
                stripe_subscription_id,
                tier,
                status,
                current_period_end: current_period_end.and_then(|t| {
                    t.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                cancel_at_period_end,
            }
        });

    Ok(Json(UserStateDto {
        user_id,
        email: row.try_get("email").ok().flatten(),
        display_name: row.try_get("display_name").ok().flatten(),
        team_id: row.try_get("team_id").ok().flatten(),
        team_name: row.try_get("team_name").ok().flatten(),
        subscription,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /v1/admin/user/:user_id/subscription
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ForceGrantRequest {
    /// Tier slug to grant: `free`, `silver`, `gold`, `enterprise`, etc.
    pub tier: String,
    /// Subscription status: `active`, `trialing`, `past_due`, `canceled`,
    /// `incomplete`. Defaults to `active` if omitted.
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ForceGrantResponse {
    pub ok: bool,
    pub message: String,
}

pub async fn force_grant_subscription(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<ForceGrantRequest>,
) -> Result<Json<ForceGrantResponse>, AppError> {
    require_admin(&state, &headers)?;

    let status = req
        .status
        .as_deref()
        .map(str::to_lowercase)
        .unwrap_or_else(|| "active".to_string());
    let tier = req.tier.to_lowercase();

    if !["free", "silver", "gold", "enterprise", "advisor"].contains(&tier.as_str()) {
        return Err(AppError::bad_request(format!("unknown tier: {tier}")));
    }
    if !["active", "trialing", "past_due", "canceled", "incomplete"].contains(&status.as_str()) {
        return Err(AppError::bad_request(format!("unknown status: {status}")));
    }

    let mut tx = state.db().begin().await?;

    // Ensure user exists.
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1 LIMIT 1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("user not found"));
    }

    // Ensure solo team — same invariant as the regular checkout path.
    sqlx::query(
        r#"
        INSERT INTO teams (id, name, owner_user_id, created_at, updated_at)
        VALUES ($1, 'Personal', $1, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO team_members (team_id, user_id, role, joined_at)
        VALUES ($1, $1, 'owner', NOW())
        ON CONFLICT (team_id, user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // Force-grant semantics: wipe any prior rows for this team (incomplete
    // / canceled / etc.) and INSERT a fresh row at the requested tier.
    // The ON CONFLICT approach didn't work cleanly because of the partial
    // unique index `idx_subscriptions_team_active` (only enforces
    // uniqueness when status ∈ active/trialing/past_due) — an existing
    // `incomplete` row falls outside the index, so a conflict-target
    // upsert would silently add a second row instead of replacing the
    // first. DELETE + INSERT in the same tx is unambiguous and gives
    // QA the deterministic post-condition they need.
    sqlx::query("DELETE FROM subscriptions WHERE team_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO subscriptions (
            user_id, team_id, tier, status,
            stripe_customer_id, stripe_subscription_id
        ) VALUES ($1, $1, $2, $3::subscription_status, NULL, NULL)
        "#,
    )
    .bind(user_id)
    .bind(&tier)
    .bind(&status)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        user_id = %user_id,
        tier = %tier,
        status = %status,
        "admin: force-granted subscription",
    );

    Ok(Json(ForceGrantResponse {
        ok: true,
        message: format!("granted {tier}/{status} to {user_id}"),
    }))
}
