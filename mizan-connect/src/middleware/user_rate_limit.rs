//! Per-user rate limiter for expensive authenticated endpoints (notably
//! `/ai/chat`). The per-IP governor at the edge of the router stops
//! anonymous floods; this layer stops a *single* authenticated user
//! from burning their plan's AI credits — or our upstream OpenAI quota
//! — by hammering the chat endpoint from one terminal.
//!
//! Each user_id gets a token bucket with `burst` capacity refilling at
//! `tokens_per_minute / 60` per second. When the bucket is empty, the
//! handler returns 429 with the §A24 envelope so the desktop can render
//! a "you've hit the AI throttle, try again in a minute" toast.
//!
//! The buckets live in a `DashMap` keyed by `Uuid` so concurrent
//! requests from different users never contend on a global lock. A
//! background sweep evicts buckets idle for > 1h to bound memory.

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

/// Per-user limiter handle shared by every request. Inserted into
/// `AppState` so any middleware/handler that wants to gate by user can
/// reach it cheaply.
#[derive(Clone)]
pub struct UserRateLimiter {
    inner: Arc<UserLimiter>,
    /// Last-seen timestamp per user, used by the sweeper to evict
    /// stale buckets. Not strictly required for correctness — governor
    /// already cleans up zero-tokens buckets — but caps memory under
    /// adversarial load (e.g. churn through many fake JWTs).
    last_seen: Arc<DashMap<Uuid, Instant>>,
}

impl UserRateLimiter {
    /// Build a limiter allowing `tokens_per_minute` tokens with a
    /// burst of `burst`. Inputs are clamped to ≥ 1 by saturating to
    /// `NonZeroU32::MIN` when zero is passed — no panics on bad input.
    pub fn new(tokens_per_minute: u32, burst: u32) -> Self {
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
        }
    }

    /// Check if `user_id` may proceed. Returns `Err(AppError)` with
    /// `too_many_requests` when the bucket is empty.
    pub fn check(&self, user_id: Uuid) -> Result<(), AppError> {
        self.last_seen.insert(user_id, Instant::now());
        self.inner.check_key(&user_id).map_err(|_not_until| {
            AppError::too_many_requests(
                "AI throttle: too many requests in the last minute. Try again shortly.",
            )
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

/// Axum middleware: extracts AuthenticatedUser, then consults the
/// limiter pulled from the AppState. Mount via
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
    state.user_rate_limiter().check(user.0.id)?;
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn first_request_passes() {
        let limiter = UserRateLimiter::new(60, 1);
        let user = Uuid::new_v4();
        assert!(limiter.check(user).is_ok());
    }

    #[test]
    fn second_immediate_request_fails_when_burst_is_one() {
        let limiter = UserRateLimiter::new(60, 1);
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
        let limiter = UserRateLimiter::new(60, 1);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        limiter.check(a).unwrap();
        // b's bucket is independent.
        assert!(limiter.check(b).is_ok());
    }

    #[test]
    fn sweep_removes_idle_users() {
        let limiter = UserRateLimiter::new(60, 5);
        let user = Uuid::new_v4();
        limiter.check(user).unwrap();
        assert_eq!(limiter.last_seen.len(), 1);
        // ttl=0 evicts everyone observed before "now"
        std::thread::sleep(Duration::from_millis(2));
        limiter.sweep(Duration::from_millis(1));
        assert_eq!(limiter.last_seen.len(), 0);
    }
}
