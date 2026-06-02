//! `GET /v1/config/public` — runtime self-discovery of the public
//! Supabase config the desktop needs to render the sign-in flow.
//!
//! Why this exists:
//!   - Pre-built Mizan AI desktop binaries shouldn't need to be
//!     rebuilt every time a Supabase project is rotated. Shipping the
//!     anon key as a build-time env baked into the bundle would
//!     require a fresh installer for every key rotation.
//!   - Supabase URL + anon (publishable) key are PUBLIC by design.
//!     Supabase documents the anon key as "safe to embed in clients";
//!     RLS policies are what actually protect tenant data.
//!   - Letting the desktop fetch these from the cloud on first launch
//!     means a single Fly secret update propagates to every installed
//!     client — the "Coming soon / Mizan Connect offline" message
//!     disappears worldwide the moment the secret is set.
//!
//! Endpoint is unauthenticated (the values are public) and cheap
//! enough to call once per app launch.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfigResponse {
    /// Supabase project URL (e.g. `https://xxxxx.supabase.co`).
    pub supabase_url: String,
    /// Supabase publishable (anon) key. `None` if the operator hasn't
    /// configured it via `SUPABASE_PUBLISHABLE_KEY` — the desktop then
    /// surfaces its existing "offline build" message.
    pub supabase_publishable_key: Option<String>,
    /// Stripe checkout return URL the cloud will use after billing.
    /// Surfaced so the desktop can deep-link back consistently.
    pub billing_return_url: Option<String>,
    /// Whether Plaid is configured server-side. Lets the desktop hide
    /// "Connect a broker" CTAs on builds whose backend doesn't have
    /// Plaid keys (e.g. self-hosted Free-tier-only instances).
    pub plaid_enabled: bool,
    /// Whether Stripe billing is configured server-side. Hides
    /// upgrade UI on backends running without billing.
    pub billing_enabled: bool,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/config/public", get(public_config))
}

async fn public_config(State(state): State<AppState>) -> Json<PublicConfigResponse> {
    let cfg = state.config();
    Json(PublicConfigResponse {
        supabase_url: cfg.supabase_url.clone(),
        supabase_publishable_key: cfg.supabase_publishable_key.clone(),
        billing_return_url: state.billing().map(|b| b.return_url.clone()),
        plaid_enabled: state.plaid().is_some(),
        billing_enabled: state.billing().is_some(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::PublicConfigResponse;

    #[test]
    fn public_config_serialises_camel_case() {
        let p = PublicConfigResponse {
            supabase_url: "https://abc.supabase.co".into(),
            supabase_publishable_key: Some("eyJhbG...".into()),
            billing_return_url: Some("https://mizan.app/billing/return".into()),
            plaid_enabled: true,
            billing_enabled: true,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("supabaseUrl").is_some());
        assert!(v.get("supabasePublishableKey").is_some());
        assert!(v.get("billingReturnUrl").is_some());
        assert_eq!(v.get("plaidEnabled").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(v.get("billingEnabled").and_then(|v| v.as_bool()), Some(true));
    }
}
