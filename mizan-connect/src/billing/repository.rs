//! SQL queries for subscription + usage state.
//!
//! All mutations of `subscriptions` go through here so the audit trail
//! (`ai_credits_used`, period anchors, status transitions) stays consistent
//! with the webhook handler.

use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use super::credits::credits_for_tokens;

/// Row shape used by the /v1/me handler.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubscriptionRow {
    pub user_id: Uuid,
    pub stripe_customer_id: Option<String>,
    pub stripe_subscription_id: Option<String>,
    /// Tier slug as stored in the ENUM. Cast to TEXT in the SELECT.
    pub tier: String,
    pub status: String,
    pub current_period_end: Option<OffsetDateTime>,
    pub cancel_at_period_end: bool,
    pub ai_credits_used: i32,
    pub ai_credits_period_start: Option<OffsetDateTime>,
}

/// Fetch the user's currently-billable subscription, if any.
///
/// "Currently billable" = the row covered by the partial unique index on
/// `(user_id) WHERE status IN ('active','trialing','past_due')`. Returns
/// `None` when the user has never subscribed (Free).
pub async fn fetch_active(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<SubscriptionRow>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionRow>(
        r#"
        SELECT user_id,
               stripe_customer_id,
               stripe_subscription_id,
               tier::TEXT  AS tier,
               status::TEXT AS status,
               current_period_end,
               cancel_at_period_end,
               ai_credits_used,
               ai_credits_period_start
          FROM subscriptions
         WHERE user_id = $1
           AND status IN ('active', 'trialing', 'past_due')
         ORDER BY updated_at DESC
         LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Ensure the user has a solo team. Migration 0005 backfilled existing
/// users with `team_id == user_id`, but users that sign up via Supabase
/// JWT *after* the migration ran don't have a team row yet — so the very
/// first subscription INSERT (which requires NOT NULL `team_id`) would
/// fail with a constraint violation. This lazy-create reuses the
/// migration's UUID invariant so the team is fully interchangeable with
/// a backfilled one, and is idempotent across reruns.
pub async fn ensure_solo_team(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    let name = if display_name.trim().is_empty() {
        "Personal".to_string()
    } else {
        display_name.trim().to_string()
    };
    sqlx::query(
        r#"
        INSERT INTO teams (id, name, owner_user_id, created_at, updated_at)
        VALUES ($1, $2, $1, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(&name)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO team_members (team_id, user_id, role, joined_at)
        VALUES ($1, $1, 'owner', NOW())
        ON CONFLICT (team_id, user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Persist or update the user's Stripe customer id at checkout-create time
/// (before any subscription row exists).
///
/// Inserts a placeholder subscription row in `incomplete` status so the FK
/// from later webhook upserts has somewhere to land even before checkout
/// completes. The webhook then promotes status/tier/period_end.
///
/// **Pre-condition:** the user must already have a `teams` row (with
/// id == user_id) — call [`ensure_solo_team`] in the handler before this.
/// Without it the INSERT trips the NOT NULL constraint on `team_id`.
pub async fn upsert_customer_stub(
    pool: &PgPool,
    user_id: Uuid,
    stripe_customer_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO subscriptions (user_id, team_id, stripe_customer_id, tier, status)
        VALUES ($1, $1, $2, 'silver', 'incomplete')
        ON CONFLICT (stripe_customer_id) DO UPDATE
          SET updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(stripe_customer_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Look up the user's Stripe customer id (created on first checkout).
pub async fn fetch_customer_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        r#"
        SELECT stripe_customer_id
          FROM subscriptions
         WHERE user_id = $1
           AND stripe_customer_id IS NOT NULL
         ORDER BY updated_at DESC
         LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| r.0))
}

/// Webhook subscription upsert payload (subset of the Stripe object).
pub struct WebhookSubscriptionUpsert<'a> {
    pub user_id: Uuid,
    pub stripe_customer_id: &'a str,
    pub stripe_subscription_id: &'a str,
    pub tier: &'a str,
    pub status: &'a str,
    pub current_period_start: Option<OffsetDateTime>,
    pub current_period_end: Option<OffsetDateTime>,
    pub cancel_at_period_end: bool,
}

/// Upsert by `stripe_subscription_id`. Called from the webhook handler;
/// caller is responsible for opening the tx (so the `stripe_events`
/// idempotency row + this write commit together).
///
/// Inserts `team_id = user_id` so the NOT NULL constraint (added by
/// migration 0005) is satisfied. This matches the invariant
/// [`ensure_solo_team`] establishes — the webhook handler can also
/// hit this path for users who created a Stripe subscription via
/// some external flow without going through `create_checkout_session`
/// first, so the upsert idempotently re-asserts the same shape.
pub async fn upsert_from_webhook(
    tx: &mut PgConnection,
    p: WebhookSubscriptionUpsert<'_>,
) -> Result<(), sqlx::Error> {
    // Belt-and-braces: ensure the solo team exists *inside* the same tx
    // as the subscription upsert so the FK is satisfied even if the
    // webhook beat the checkout-session call (Stripe retries can land
    // events out of order under load).
    sqlx::query(
        r#"
        INSERT INTO teams (id, name, owner_user_id, created_at, updated_at)
        VALUES ($1, 'Personal', $1, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(p.user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO team_members (team_id, user_id, role, joined_at)
        VALUES ($1, $1, 'owner', NOW())
        ON CONFLICT (team_id, user_id) DO NOTHING
        "#,
    )
    .bind(p.user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO subscriptions (
            user_id, team_id, stripe_customer_id, stripe_subscription_id,
            tier, status,
            current_period_start, current_period_end,
            cancel_at_period_end
        ) VALUES ($1, $1, $2, $3, $4, $5::subscription_status, $6, $7, $8)
        ON CONFLICT (stripe_subscription_id) DO UPDATE
          SET status               = EXCLUDED.status,
              tier                 = EXCLUDED.tier,
              current_period_start = EXCLUDED.current_period_start,
              current_period_end   = EXCLUDED.current_period_end,
              cancel_at_period_end = EXCLUDED.cancel_at_period_end,
              updated_at           = NOW()
        "#,
    )
    .bind(p.user_id)
    .bind(p.stripe_customer_id)
    .bind(p.stripe_subscription_id)
    .bind(p.tier)
    .bind(p.status)
    .bind(p.current_period_start)
    .bind(p.current_period_end)
    .bind(p.cancel_at_period_end)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Reset AI credits to 0 at the start of each invoice period. Called from the
/// `invoice.paid` webhook handler in the same tx as the event-id insert.
pub async fn reset_ai_credits(
    tx: &mut PgConnection,
    stripe_subscription_id: &str,
    period_start: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscriptions
           SET ai_credits_used = 0,
               ai_credits_period_start = $2,
               updated_at = NOW()
         WHERE stripe_subscription_id = $1
        "#,
    )
    .bind(stripe_subscription_id)
    .bind(period_start)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Stamp `last_payment_failure_at = NOW()` on the subscription so the
/// desktop can surface "card declined — update payment method" UX even
/// before Stripe's smart-retry policy ages the subscription out to
/// `past_due`. Idempotent: re-stamping is fine, Stripe sends multiple
/// `invoice.payment_failed` during the retry window.
///
/// Returns Ok(()) when the row exists; if there's no matching
/// subscription locally (out-of-order webhook delivery, race with
/// checkout.session.completed) the UPDATE simply matches 0 rows and
/// the caller logs + continues.
pub async fn mark_payment_failed(
    tx: &mut PgConnection,
    stripe_subscription_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscriptions
           SET last_payment_failure_at = NOW(),
               updated_at = NOW()
         WHERE stripe_subscription_id = $1
        "#,
    )
    .bind(stripe_subscription_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Stamp `trial_end_at` from the `customer.subscription.trial_will_end`
/// 7-day-warning webhook so the desktop can render a "Your trial ends
/// on YYYY-MM-DD" banner. No tier mutation here — the subscription is
/// still on trial; we just surface the deadline.
pub async fn mark_trial_will_end(
    tx: &mut PgConnection,
    stripe_subscription_id: &str,
    trial_end: Option<OffsetDateTime>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscriptions
           SET trial_end_at = $2,
               updated_at = NOW()
         WHERE stripe_subscription_id = $1
        "#,
    )
    .bind(stripe_subscription_id)
    .bind(trial_end)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Count AI chat invocations made by `user_id` in the trailing 24h.
/// Used by the preflight gate in `ai_proxy` to enforce the daily
/// per-tier message cap (Silver = 50/day, Gold = unlimited).
///
/// Uses `created_at >= NOW() - INTERVAL '24 hours'` rather than a
/// calendar-day boundary so the cap is a true rolling window — the
/// user's 51st message at 23:59 doesn't unlock a fresh 50 at midnight
/// in their server-side wall clock, which would be the obvious abuse
/// vector. usage_ledger is keyed by `metric = 'ai_chat'` so we can
/// add other rate-limited metrics later without re-shaping this query.
pub async fn count_ai_chats_last_24h(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM usage_ledger
        WHERE user_id = $1
          AND metric = 'ai_chat'
          AND created_at >= NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Append a usage_ledger row.
pub async fn record_usage(
    tx: &mut PgConnection,
    user_id: Uuid,
    metric: &str,
    units: i32,
    cost_credits: i32,
    model: Option<&str>,
    kind: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO usage_ledger (user_id, metric, units, cost_credits, model, kind)
        VALUES ($1, $2::usage_metric, $3, $4, $5, $6)
        "#,
    )
    .bind(user_id)
    .bind(metric)
    .bind(units)
    .bind(cost_credits)
    .bind(model)
    .bind(kind)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Increment `ai_credits_used` by the row count; called after a managed-AI
/// reply with the *actual* token-derived credit charge. Returns the new total
/// so the caller can decide whether to throttle subsequent requests.
pub async fn add_ai_credits_used(
    tx: &mut PgConnection,
    user_id: Uuid,
    credits: i32,
) -> Result<i32, sqlx::Error> {
    let row: (i32,) = sqlx::query_as(
        r#"
        UPDATE subscriptions
           SET ai_credits_used = ai_credits_used + $2,
               updated_at = NOW()
         WHERE user_id = $1
           AND status IN ('active', 'trialing', 'past_due')
        RETURNING ai_credits_used
        "#,
    )
    .bind(user_id)
    .bind(credits)
    .fetch_one(&mut *tx)
    .await?;
    Ok(row.0)
}

/// Idempotency check for webhook events. Inserts the event id; returns
/// `false` if it already existed (caller should short-circuit).
pub async fn try_record_stripe_event(
    tx: &mut PgConnection,
    event_id: &str,
    event_type: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        r#"
        INSERT INTO stripe_events (stripe_event_id, event_type)
        VALUES ($1, $2)
        ON CONFLICT (stripe_event_id) DO NOTHING
        "#,
    )
    .bind(event_id)
    .bind(event_type)
    .execute(&mut *tx)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// Convenience: credits to charge for an actual AI reply (token reconciliation
/// after streaming). Centralizes the dependency on the credit table so other
/// callers don't reimplement it.
pub fn credits_for_reply(prompt_tokens: u32, completion_tokens: u32) -> i32 {
    credits_for_tokens(prompt_tokens, completion_tokens)
}
