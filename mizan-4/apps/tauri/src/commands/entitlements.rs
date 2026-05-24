//! Entitlements & feature-gating commands.
//!
//! `get_entitlements` exposes the resolved subscription matrix to the
//! frontend. The helpers ([`resolve_entitlements`], [`gated`]) let every other
//! premium command enforce limits in Rust — not just the UI — so gates can't be
//! bypassed by manipulating the webview.
//!
//! Gate failures are returned as a JSON-encoded [`GatedError`] in the command's
//! `Err(String)` channel. The frontend recognizes the `__gated` marker and
//! raises the matching contextual upgrade modal instead of a raw error toast.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::context::ServiceContext;
use mizan_connect::Entitlements;

/// Structured "upgrade required" error. Serialized into the command `Err`
/// string with a `__gated: true` marker so the frontend can branch on it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatedError {
    /// Always `true` — the discriminator the frontend matches on.
    #[serde(rename = "__gated")]
    pub gated: bool,
    /// Stable feature key (e.g. `"broker_sync"`, `"max_portfolios"`). Maps to a
    /// contextual upgrade modal on the frontend.
    pub feature: String,
    /// Lowest plan slug that unlocks the feature (for the upgrade CTA).
    pub required_tier: String,
    /// The user's current plan slug.
    pub current_plan: String,
    /// Human-readable fallback message.
    pub message: String,
}

impl GatedError {
    fn encode(feature: &str, required_tier: &str, current_plan: &str, message: &str) -> String {
        let err = GatedError {
            gated: true,
            feature: feature.to_string(),
            required_tier: required_tier.to_string(),
            current_plan: current_plan.to_string(),
            message: message.to_string(),
        };
        // Falls back to the plain message if (impossibly) serialization fails.
        serde_json::to_string(&err).unwrap_or_else(|_| message.to_string())
    }
}

/// Resolve the current entitlements, defaulting to Free for signed-out users or
/// builds without cloud sync (so the app degrades gracefully rather than
/// erroring). This is the entry point premium commands should use.
pub async fn resolve_entitlements(ctx: &ServiceContext) -> Entitlements {
    ctx.connect_service()
        .get_entitlements()
        .await
        .unwrap_or_default()
}

/// Gate helper: returns `Ok(())` when `allowed`, otherwise an encoded
/// [`GatedError`] ready to return from a command's `Err` channel.
pub fn gated(
    allowed: bool,
    feature: &str,
    required_tier: &str,
    current_plan: &str,
    message: &str,
) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err(GatedError::encode(
            feature,
            required_tier,
            current_plan,
            message,
        ))
    }
}

/// Return the current user's entitlements matrix (Free when signed out).
#[tauri::command]
pub async fn get_entitlements(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Entitlements, String> {
    Ok(resolve_entitlements(&state).await)
}

/// Fetch a Stripe Checkout URL for the given plan + interval. Caller opens
/// the URL in the default browser via the shell plugin; on return, focus
/// listeners invalidate the entitlements query so the unlocked plan takes
/// effect without a reload.
#[tauri::command]
pub async fn open_checkout(
    plan: String,
    interval: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<String, String> {
    state
        .connect_service()
        .create_checkout_url(&plan, &interval)
        .await
}

/// Fetch a Stripe Customer Portal URL for self-service plan management.
#[tauri::command]
pub async fn open_billing_portal(state: State<'_, Arc<ServiceContext>>) -> Result<String, String> {
    state.connect_service().create_billing_portal_url().await
}

/// Fire-and-forget usage report (`/api/v1/usage`). Returns `Ok(())`
/// regardless of cloud success so the caller's local action isn't blocked
/// by transient cloud unavailability.
#[tauri::command]
pub async fn report_usage(
    metric: String,
    units: i32,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state.connect_service().report_usage(&metric, units).await;
    Ok(())
}
