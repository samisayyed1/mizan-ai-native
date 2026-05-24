//! Shared application state passed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::jwks::JwksCache;
use crate::billing::BillingContext;
use crate::config::Config;
use crate::middleware::user_rate_limit::UserRateLimiter;
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
    /// Per-authenticated-user throttle, used by `/ai/chat` (and any
    /// future expensive endpoint that should not be hammerable by a
    /// single client even when the global IP bucket has headroom).
    user_rate_limiter: UserRateLimiter,
}

impl AppState {
    pub fn new(
        config: Config,
        db: PgPool,
        jwks: JwksCache,
        plaid: Option<PlaidContext>,
        billing: Option<BillingContext>,
    ) -> Self {
        // 60/min per user with a burst of 20 by default — generous
        // enough for a conversational AI session but firm enough to
        // stop a single client from churning OpenAI quota.
        let user_rate_limiter = UserRateLimiter::new(
            config.user_rate_limit_per_minute.unwrap_or(60),
            config.user_rate_limit_burst.unwrap_or(20),
        );
        Self {
            inner: Arc::new(Inner {
                config,
                db,
                jwks,
                plaid,
                billing,
                user_rate_limiter,
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

    /// Per-authenticated-user rate limiter for expensive endpoints
    /// (notably `/ai/chat`). Cloning is cheap — buckets are behind Arc.
    pub fn user_rate_limiter(&self) -> &UserRateLimiter {
        &self.inner.user_rate_limiter
    }
}
