//! Stripe webhook handler.
//!
//! `POST /v1/stripe/webhook` — anonymous, signature-verified. Idempotent: the
//! event id is recorded in `stripe_events` in the same transaction as any
//! subscription mutation, so a Stripe redelivery is a no-op.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use secrecy::ExposeSecret;
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::audit;
use crate::error::AppError;
use crate::state::AppState;

use super::prices::CheckoutPlan;
use super::repository::{self, WebhookSubscriptionUpsert};
use super::stripe_client::StripeClient;

const STRIPE_SIG_HEADER: &str = "Stripe-Signature";

pub async fn receive_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let billing = state
        .billing()
        .ok_or_else(|| AppError::not_implemented("billing not configured"))?;

    let sig = headers
        .get(STRIPE_SIG_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::bad_request("missing Stripe-Signature header"))?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    StripeClient::verify_webhook(billing.webhook_secret.expose_secret(), sig, &body, now)
        .map_err(|_| AppError::unauthorized("invalid webhook signature"))?;

    let envelope: Envelope = serde_json::from_slice(&body)
        .map_err(|e| AppError::bad_request(format!("malformed webhook payload: {e}")))?;

    let mut tx = state.db().begin().await?;

    // Idempotency: short-circuit if we've processed this event id.
    if !repository::try_record_stripe_event(&mut tx, &envelope.id, &envelope.event_type).await? {
        tx.commit().await?;
        tracing::info!(event_id = %envelope.id, "stripe webhook replay ignored");
        return Ok(StatusCode::OK);
    }

    // Audit-log breadcrumbs accumulated while processing this event.
    // Written AFTER tx.commit() so an audit write failure can never
    // roll back a successful subscription mutation (the user-facing
    // path comes first).
    let mut audit_breadcrumbs: Vec<(String, Option<Uuid>, serde_json::Value)> = Vec::new();

    match envelope.event_type.as_str() {
        "customer.subscription.created"
        | "customer.subscription.updated"
        | "customer.subscription.deleted" => {
            let sub: SubscriptionObject = serde_json::from_value(envelope.data.object.clone())
                .map_err(|e| AppError::bad_request(format!("subscription payload: {e}")))?;

            let Some(user_id) = resolve_user_id(state.db(), &sub).await? else {
                // Race: `subscription.created` arrived BEFORE the
                // `checkout.session.completed` row that links the
                // stripe_customer_id to our user_id (Stripe delivers
                // these out-of-order on a small percentage of requests).
                // We MUST NOT commit the stripe_events row here, or the
                // redelivery once the link exists will be silently
                // dropped as a duplicate. Roll back + return 5xx so
                // Stripe retries with exponential backoff (3d window).
                tracing::warn!(
                    stripe_subscription_id = %sub.id,
                    stripe_customer_id = %sub.customer,
                    "stripe webhook references unknown customer — rolling back so Stripe retries",
                );
                tx.rollback().await?;
                return Err(AppError::internal(
                    "customer link not yet established; please retry",
                ));
            };

            let tier = resolve_tier(&sub, &billing.prices)
                .map(|p| p.slug())
                .unwrap_or("silver");

            // 'subscription.deleted' uses 'canceled' status per Stripe docs;
            // already in our ENUM so no mapping needed.
            let status = sub.status.as_str();

            let period_start = ts(sub.current_period_start);
            let period_end = ts(sub.current_period_end);

            repository::upsert_from_webhook(
                &mut tx,
                WebhookSubscriptionUpsert {
                    user_id,
                    stripe_customer_id: &sub.customer,
                    stripe_subscription_id: &sub.id,
                    tier,
                    status,
                    current_period_start: period_start,
                    current_period_end: period_end,
                    cancel_at_period_end: sub.cancel_at_period_end,
                },
            )
            .await?;

            audit_breadcrumbs.push((
                format!("billing.{}", envelope.event_type.replace('.', "_")),
                Some(user_id),
                serde_json::json!({
                    "stripe_subscription_id": sub.id,
                    "stripe_customer_id_suffix": sub.customer.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>(),
                    "tier": tier,
                    "status": status,
                    "cancel_at_period_end": sub.cancel_at_period_end,
                }),
            ));
        }

        "invoice.paid" => {
            let invoice: InvoiceObject = serde_json::from_value(envelope.data.object.clone())
                .map_err(|e| AppError::bad_request(format!("invoice payload: {e}")))?;
            if let (Some(sub_id), Some(period_start)) =
                (invoice.subscription, ts(invoice.period_start))
            {
                repository::reset_ai_credits(&mut tx, &sub_id, period_start).await?;
                audit_breadcrumbs.push((
                    "billing.invoice_paid".to_string(),
                    None,
                    serde_json::json!({
                        "stripe_subscription_id": sub_id,
                        "period_start": period_start.to_string(),
                    }),
                ));
            }
        }

        // Stripe-1: payment failed. The subscription stays "active" until the
        // smart-retry policy ages out (3–5 attempts over a week), then Stripe
        // moves it to "past_due" → "unpaid" → "canceled". We mirror the
        // intermediate status flips locally so the desktop's entitlements
        // can downgrade Gold features the moment Stripe says "past_due" —
        // we don't wait for the eventual cancel. Stripe sends a separate
        // customer.subscription.updated when the status changes, but we
        // also stamp last_payment_failure on the row so support can see
        // why a downgrade fired even before the subsequent update arrives.
        "invoice.payment_failed" => {
            let invoice: InvoiceObject = serde_json::from_value(envelope.data.object.clone())
                .map_err(|e| AppError::bad_request(format!("invoice payload: {e}")))?;
            if let Some(sub_id) = invoice.subscription.as_deref() {
                if let Err(err) = repository::mark_payment_failed(&mut tx, sub_id).await {
                    tracing::warn!(
                        stripe_subscription_id = %sub_id,
                        error = %err,
                        "stripe webhook: mark_payment_failed soft-failed"
                    );
                }
                tracing::info!(
                    stripe_subscription_id = %sub_id,
                    "stripe webhook: invoice.payment_failed recorded"
                );
                audit_breadcrumbs.push((
                    "billing.invoice_payment_failed".to_string(),
                    None,
                    serde_json::json!({ "stripe_subscription_id": sub_id }),
                ));
            }
        }

        // Stripe-1: 7-day heads-up that a trial is about to convert. We
        // record the date on the subscription row so the desktop UI can
        // surface a "trial ends in N days" banner ahead of the charge.
        // No tier change here — the subscription is still on the trial.
        "customer.subscription.trial_will_end" => {
            let sub: SubscriptionObject = serde_json::from_value(envelope.data.object.clone())
                .map_err(|e| AppError::bad_request(format!("subscription payload: {e}")))?;
            let trial_end = ts(sub.trial_end);
            if let Err(err) = repository::mark_trial_will_end(&mut tx, &sub.id, trial_end).await {
                tracing::warn!(
                    stripe_subscription_id = %sub.id,
                    error = %err,
                    "stripe webhook: mark_trial_will_end soft-failed"
                );
            }
            tracing::info!(
                stripe_subscription_id = %sub.id,
                trial_end = ?trial_end,
                "stripe webhook: trial_will_end recorded"
            );
        }

        "checkout.session.completed" => {
            // The subscription.created event is delivered alongside; nothing
            // extra to do here besides idempotent acknowledgment.
        }

        other => {
            tracing::debug!(event_type = %other, "stripe webhook ignored");
        }
    }

    tx.commit().await?;

    // Audit log writes happen AFTER the user-facing transaction commits.
    // They're fire-and-forget — an audit write failure logs a warning
    // but never unwinds a successfully-processed Stripe event. The
    // audit gate requires *attempted* writes for every state mutation,
    // not strict at-least-once delivery (those are separate concerns).
    for (event_type, user_id, payload) in audit_breadcrumbs {
        let mut event = audit::AuditEvent::new(&event_type).data(&payload);
        if let Some(uid) = user_id {
            event = event.user(uid);
        }
        if let Err(err) = audit::record_event(state.db(), event).await {
            tracing::warn!(
                error = %err,
                event_type = %event_type,
                "audit log write failed for stripe webhook event",
            );
        }
    }

    Ok(StatusCode::OK)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn ts(unix: Option<i64>) -> Option<OffsetDateTime> {
    unix.and_then(|s| OffsetDateTime::from_unix_timestamp(s).ok())
}

async fn resolve_user_id(
    pool: &sqlx::PgPool,
    sub: &SubscriptionObject,
) -> Result<Option<Uuid>, AppError> {
    // Look up by Stripe customer id. We persist (user_id, stripe_customer_id)
    // at checkout-creation time, so by the time `subscription.created` fires
    // there's a placeholder row to join against. The subscription event
    // doesn't carry our internal user id verbatim — `client_reference_id` is
    // on the *checkout session* object, not the subscription.
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM subscriptions WHERE stripe_customer_id = $1 LIMIT 1")
            .bind(&sub.customer)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

fn resolve_tier(
    sub: &SubscriptionObject,
    prices: &super::prices::StripePrices,
) -> Option<CheckoutPlan> {
    // 1) Price.metadata.plan (recommended setup — operator sets it in the Stripe Dashboard).
    if let Some(plan) = sub
        .items
        .data
        .iter()
        .filter_map(|item| item.price.metadata.plan.as_deref())
        .next()
    {
        if let Ok(p) = plan.parse::<CheckoutPlan>() {
            return Some(p);
        }
    }
    // 2) Fall back to matching the Price ID against our env-configured map.
    sub.items
        .data
        .iter()
        .find_map(|item| prices.plan_for(&item.price.id))
}

// ─────────────────────────────────────────────────────────────────────────────
// Stripe event payload shapes (only the fields we read)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Envelope {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: EnvelopeData,
}

#[derive(Debug, Deserialize)]
struct EnvelopeData {
    object: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct SubscriptionObject {
    id: String,
    customer: String,
    status: String,
    cancel_at_period_end: bool,
    #[serde(default)]
    current_period_start: Option<i64>,
    #[serde(default)]
    current_period_end: Option<i64>,
    /// Unix-timestamp of when the trial converts to a paid charge.
    /// Populated on `customer.subscription.trial_will_end` (7-day
    /// heads-up) so the desktop can render a banner.
    #[serde(default)]
    trial_end: Option<i64>,
    items: SubscriptionItems,
}

#[derive(Debug, Deserialize)]
struct SubscriptionItems {
    data: Vec<SubscriptionItem>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionItem {
    price: Price,
}

#[derive(Debug, Deserialize)]
struct Price {
    id: String,
    #[serde(default)]
    metadata: PriceMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct PriceMetadata {
    #[serde(default)]
    plan: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InvoiceObject {
    #[serde(default)]
    subscription: Option<String>,
    #[serde(default)]
    period_start: Option<i64>,
}
