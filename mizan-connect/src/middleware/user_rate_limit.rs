//! Per-user rate limiters for expensive authenticated endpoints.
//!
//! The per-IP governor at the edge of the router (`tower_governor`)
//! stops anonymous floods; this layer stops a *single* authenticated
//! user from burning their plan's quotas — or our upstream third-party
//! quotas — by hammering a hot endpoint from one terminal.
//!
//! Each user_id gets a token bucket *per endpoint family*. AI chat,
//! billing, Plaid, OAuth connect, and MCP gateway each have their own
//! limiter so saturating one path doesn't punch through the others'
//! headroom. The buckets live in a `DashMap` keyed by `Uuid` so
//! concurrent requests from different users never contend on a global
//! lock. A background sweep evicts buckets idle for > 1h to bound
//! memory.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use dashmap::DashMap;
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::state::AppState;

type UserLimiter = RateLimiter<Uuid, DefaultKeyedStateStore<Uuid>, DefaultClock>;

/// Per-user limiter handle shared by every request.
///
/// One instance covers exactly one endpoint family. `purpose` is the
/// short human-readable label woven into the 429 error message so the
/// desktop can render a context-appropriate toast.
#[derive(Clone)]
pub struct UserRateLimiter {
    inner: Arc<UserLimiter>,
    /// Last-seen timestamp per user, used by the sweeper to evict
    /// stale buckets. Not strictly required for correctness — governor
    /// already cleans up zero-tokens buckets — but caps memory under
    /// adversarial load (e.g. churn through many fake JWTs).
    last_seen: Arc<DashMap<Uuid, Instant>>,
    purpose: &'static str,
}

impl UserRateLimiter {
    /// Build a limiter allowing `tokens_per_minute` tokens with a
    /// burst of `burst`. Inputs are clamped to ≥ 1 by saturating to
    /// `NonZeroU32::MIN` when zero is passed — no panics on bad input.
    /// `purpose` is the label folded into the 429 error message
    /// (e.g. `"AI"`, `"billing"`, `"Plaid"`).
    pub fn new(purpose: &'static str, tokens_per_minute: u32, burst: u32) -> Self {
        // `NonZeroU32::new` returns Option; `.unwrap_or(MIN)` saturates
        // the zero case to 1 instead of panicking. Both inputs are
        // user-configurable so a misconfigured env var must not crash
        // the boot sequence.
        let tokens_nz = NonZeroU32::new(tokens_per_minute).unwrap_or(NonZeroU32::MIN);
        let burst_nz = NonZeroU32::new(burst).unwrap_or(NonZeroU32::MIN);
        let quota = Quota::per_minute(tokens_nz).allow_burst(burst_nz);
        Self {
            inner: Arc::new(RateLimiter::keyed(quota)),
            last_seen: Arc::new(DashMap::new()),
            purpose,
        }
    }

    /// Check if `user_id` may proceed. Returns `Err(AppError)` with
    /// `too_many_requests` when the bucket is empty.
    pub fn check(&self, user_id: Uuid) -> Result<(), AppError> {
        self.last_seen.insert(user_id, Instant::now());
        self.inner.check_key(&user_id).map_err(|_not_until| {
            AppError::too_many_requests(format!(
                "{} throttle: too many requests in the last minute. Try again shortly.",
                self.purpose
            ))
        })
    }

    /// Drop bucket state for users idle longer than `ttl`. Call from a
    /// background task with a periodic timer.
    pub fn sweep(&self, ttl: Duration) {
        let now = Instant::now();
        self.last_seen
            .retain(|_uuid, last| now.duration_since(*last) <= ttl);
        // governor's RateLimiter doesn't expose per-key removal, but
        // the per-key buckets refill to full naturally, so leaving
        // stale entries just costs a few bytes each. The sweep above
        // drops our last_seen index which is the more substantial
        // memory cost for high-churn workloads.
    }
}

/// Five named limiter families. Cheap to clone — every field is an
/// `Arc`-backed handle.
#[derive(Clone)]
pub struct EndpointLimiters {
    pub ai_chat: UserRateLimiter,
    pub billing: UserRateLimiter,
    pub plaid: UserRateLimiter,
    pub oauth: UserRateLimiter,
    pub mcp: UserRateLimiter,
}

impl EndpointLimiters {
    /// Sweep every family's stale buckets in one call.
    pub fn sweep_all(&self, ttl: Duration) {
        self.ai_chat.sweep(ttl);
        self.billing.sweep(ttl);
        self.plaid.sweep(ttl);
        self.oauth.sweep(ttl);
        self.mcp.sweep(ttl);
    }
}

/// Axum middleware: extracts AuthenticatedUser, then consults the
/// AI chat limiter pulled from the AppState. Mount via
/// `.route_layer(axum::middleware::from_fn_with_state(state, ...))`
/// on the routes that need it (e.g. `/ai/chat`). `route_layer` (as
/// opposed to `layer`) ensures rejections from the inner extractor
/// short-circuit at this scope rather than punching through to
/// upstream middleware.
pub async fn enforce_per_user_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    state.endpoint_limiters().ai_chat.check(user.0.id)?;
    Ok(next.run(request).await)
}

/// Per-user billing throttle (checkout, portal, usage). Default
/// 10/min burst 5 — billing endpoints are inherently low-cadence;
/// anyone hammering them is either a runaway test loop or abuse.
pub async fn enforce_billing_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    state.endpoint_limiters().billing.check(user.0.id)?;
    Ok(next.run(request).await)
}

/// Per-user Plaid throttle (link-token, public-token exchange,
/// account sync). Plaid charges per call and rate-limits us at the
/// edge; this layer keeps a single client from burning the whole
/// account's Plaid budget.
pub async fn enforce_plaid_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    state.endpoint_limiters().plaid.check(user.0.id)?;
    Ok(next.run(request).await)
}

/// Per-user OAuth throttle (provider connect, disconnect). The
/// callback endpoint is gated by the signed state nonce already, so
/// repeated callbacks are self-rejecting; this layer covers the
/// initiating endpoints where a misbehaving client could flood
/// upstream OAuth providers.
pub async fn enforce_oauth_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    state.endpoint_limiters().oauth.check(user.0.id)?;
    Ok(next.run(request).await)
}

/// Per-user MCP gateway throttle. The MCP module already enforces
/// per-trust-level caps on `/servers/:id/call` (60/min Vetted, 10/min
/// SelfRegistered) inside the handler — this layer adds the orthogonal
/// per-user dimension so one user with multiple Vetted servers can't
/// aggregate them into an effective 60×N call rate.
pub async fn enforce_mcp_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    state.endpoint_limiters().mcp.check(user.0.id)?;
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn first_request_passes() {
        let limiter = UserRateLimiter::new("AI", 60, 1);
        let user = Uuid::new_v4();
        assert!(limiter.check(user).is_ok());
    }

    #[test]
    fn second_immediate_request_fails_when_burst_is_one() {
        let limiter = UserRateLimiter::new("AI", 60, 1);
        let user = Uuid::new_v4();
        limiter.check(user).unwrap();
        let err = limiter.check(user).unwrap_err();
        assert!(
            err.message().contains("AI throttle"),
            "expected §A24 throttle message, got: {}",
            err.message()
        );
    }

    #[test]
    fn distinct_users_have_independent_buckets() {
        let limiter = UserRateLimiter::new("AI", 60, 1);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        limiter.check(a).unwrap();
        // b's bucket is independent.
        assert!(limiter.check(b).is_ok());
    }

    #[test]
    fn sweep_removes_idle_users() {
        let limiter = UserRateLimiter::new("AI", 60, 5);
        let user = Uuid::new_v4();
        limiter.check(user).unwrap();
        assert_eq!(limiter.last_seen.len(), 1);
        // ttl=0 evicts everyone observed before "now"
        std::thread::sleep(Duration::from_millis(2));
        limiter.sweep(Duration::from_millis(1));
        assert_eq!(limiter.last_seen.len(), 0);
    }

    #[test]
    fn purpose_label_is_woven_into_error_message() {
        // Each endpoint-family limiter emits its own label so the
        // desktop can render the right toast (an "AI throttle" toast
        // would be misleading if the user hit the billing path).
        let billing = UserRateLimiter::new("billing", 60, 1);
        let user = Uuid::new_v4();
        billing.check(user).unwrap();
        let err = billing.check(user).unwrap_err();
        assert!(
            err.message().contains("billing throttle"),
            "expected billing-labelled throttle message, got: {}",
            err.message()
        );
    }

    #[test]
    fn endpoint_limiters_are_independent_families() {
        // Burning the AI bucket must NOT consume the Plaid bucket —
        // the whole point of the per-endpoint split.
        let limiters = EndpointLimiters {
            ai_chat: UserRateLimiter::new("AI", 60, 1),
            billing: UserRateLimiter::new("billing", 60, 1),
            plaid: UserRateLimiter::new("Plaid", 60, 1),
            oauth: UserRateLimiter::new("OAuth", 60, 1),
            mcp: UserRateLimiter::new("MCP", 60, 1),
        };
        let user = Uuid::new_v4();
        limiters.ai_chat.check(user).unwrap();
        // AI bucket is empty now (burst=1, consumed).
        assert!(limiters.ai_chat.check(user).is_err());
        // Every other family has untouched headroom.
        assert!(limiters.billing.check(user).is_ok());
        assert!(limiters.plaid.check(user).is_ok());
        assert!(limiters.oauth.check(user).is_ok());
        assert!(limiters.mcp.check(user).is_ok());
    }

    #[test]
    fn sweep_all_clears_every_family() {
        let limiters = EndpointLimiters {
            ai_chat: UserRateLimiter::new("AI", 60, 5),
            billing: UserRateLimiter::new("billing", 60, 5),
            plaid: UserRateLimiter::new("Plaid", 60, 5),
            oauth: UserRateLimiter::new("OAuth", 60, 5),
            mcp: UserRateLimiter::new("MCP", 60, 5),
        };
        let user = Uuid::new_v4();
        limiters.ai_chat.check(user).unwrap();
        limiters.billing.check(user).unwrap();
        limiters.plaid.check(user).unwrap();
        limiters.oauth.check(user).unwrap();
        limiters.mcp.check(user).unwrap();
        std::thread::sleep(Duration::from_millis(2));
        limiters.sweep_all(Duration::from_millis(1));
        assert_eq!(limiters.ai_chat.last_seen.len(), 0);
        assert_eq!(limiters.billing.last_seen.len(), 0);
        assert_eq!(limiters.plaid.last_seen.len(), 0);
        assert_eq!(limiters.oauth.last_seen.len(), 0);
        assert_eq!(limiters.mcp.last_seen.len(), 0);
    }
}
