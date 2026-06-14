//! Shared application state passed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::jwks::JwksCache;
use crate::billing::BillingContext;
use crate::config::Config;
use crate::middleware::user_rate_limit::{EndpointLimiters, UserRateLimiter};
use crate::plaid::types::PlaidContext;

/// Application state cloned into every handler.
///
/// Cheap to clone — all heavy resources are behind `Arc`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    config: Config,
    db: PgPool,
    jwks: JwksCache,
    plaid: Option<PlaidContext>,
    /// Optional — present only when Stripe + billing env vars are configured.
    /// Handlers fall back to `not_implemented` when this is `None`.
    billing: Option<BillingContext>,
    /// Per-authenticated-user throttle families. AI chat, billing,
    /// Plaid, OAuth, and MCP each get their own bucket so saturating
    /// one path doesn't punch through the others' headroom.
    endpoint_limiters: EndpointLimiters,
}

impl AppState {
    pub fn new(
        config: Config,
        db: PgPool,
        jwks: JwksCache,
        plaid: Option<PlaidContext>,
        billing: Option<BillingContext>,
    ) -> Self {
        // Per-endpoint quotas — each family gets its own bucket so
        // saturating one path leaves the others untouched. Defaults
        // are tuned to the workload shape: AI chat is conversational
        // (60/20 burst); billing + OAuth are inherently low-cadence
        // (10/5); Plaid is medium (30/10); MCP is high (60/20) because
        // a power user with several Vetted servers will legitimately
        // proxy at that rate.
        let ai_per_min = config.user_rate_limit_per_minute.unwrap_or(60);
        let ai_burst = config.user_rate_limit_burst.unwrap_or(20);
        let endpoint_limiters = EndpointLimiters {
            ai_chat: UserRateLimiter::new("AI", ai_per_min, ai_burst),
            billing: UserRateLimiter::new(
                "billing",
                config.billing_rate_limit_per_minute.unwrap_or(10),
                config.billing_rate_limit_burst.unwrap_or(5),
            ),
            plaid: UserRateLimiter::new(
                "Plaid",
                config.plaid_rate_limit_per_minute.unwrap_or(30),
                config.plaid_rate_limit_burst.unwrap_or(10),
            ),
            oauth: UserRateLimiter::new(
                "OAuth",
                config.oauth_rate_limit_per_minute.unwrap_or(10),
                config.oauth_rate_limit_burst.unwrap_or(5),
            ),
            mcp: UserRateLimiter::new(
                "MCP",
                config.mcp_rate_limit_per_minute.unwrap_or(60),
                config.mcp_rate_limit_burst.unwrap_or(20),
            ),
        };
        Self {
            inner: Arc::new(Inner {
                config,
                db,
                jwks,
                plaid,
                billing,
                endpoint_limiters,
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }

    pub fn jwks(&self) -> &JwksCache {
        &self.inner.jwks
    }

    pub fn plaid(&self) -> Option<&PlaidContext> {
        self.inner.plaid.as_ref()
    }

    /// Billing context, when Stripe is configured. `None` collapses every
    /// billing endpoint to a clean `not_implemented` response.
    pub fn billing(&self) -> Option<&BillingContext> {
        self.inner.billing.as_ref()
    }

    /// Per-authenticated-user rate-limiter families. Cloning is cheap
    /// — every field is `Arc`-backed.
    pub fn endpoint_limiters(&self) -> &EndpointLimiters {
        &self.inner.endpoint_limiters
    }
}
