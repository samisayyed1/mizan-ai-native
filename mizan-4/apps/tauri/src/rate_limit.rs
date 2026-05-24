//! Per-command rate limiter for the Tauri IPC surface.
//!
//! # Threat model
//!
//! Mizan is local-first — there's no remote attacker calling the Tauri
//! commands directly. But the IPC layer is exposed to:
//!
//!   * **Buggy / runaway frontend code** in a tight loop — e.g. a
//!     useEffect missing dependencies that fires
//!     `recalculate_portfolio` on every render. Today this would
//!     queue thousands of recalcs and starve the DB writer.
//!   * **Malicious addons** running in the same WebView. Addons can
//!     invoke arbitrary Tauri commands via the runtime context shim.
//!     Without a rate limit, an addon can DoS the host app or
//!     starve other addons.
//!
//! A simple sliding-window limiter per command name is enough to
//! shrug both off without impacting legitimate user-driven traffic
//! (no user clicks a button 100×/sec).
//!
//! # Design
//!
//! - **Per-command window**: each unique `command` string gets its
//!   own bucket. Cheap commands (`get_settings`) and expensive ones
//!   (`recalculate_portfolio`) can have independent thresholds, but
//!   for v1 we use a single global limit.
//! - **Sliding window** based on timestamps in a small `VecDeque`,
//!   not a token bucket — easier to reason about, no fractional
//!   token math, and reset semantics are obvious.
//! - **Default**: 100 calls per 5-second window. Generous enough to
//!   never bother a real user; tight enough to catch a tight-loop
//!   regression on first occurrence.
//! - **Mutex** around the registry — contention is non-issue because
//!   each `check` is O(window-size) and we hold the lock only for
//!   the duration of the check.
//! - **Per-command override**: a `with_override` builder lets future
//!   code tighten or loosen specific commands without touching the
//!   default (e.g. `recalculate_portfolio` might want 5 / 30s).
//!
//! Returns `RateLimitDecision::Allow` or `Deny { retry_after }` so
//! callers can format a helpful error message ("Too many requests,
//! retry in 1.5s") instead of opaque errors.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Decision returned from [`RateLimiter::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allow,
    Deny { retry_after: Duration },
}

#[derive(Debug, Clone, Copy)]
struct WindowConfig {
    max_calls: usize,
    window: Duration,
}

impl WindowConfig {
    const DEFAULT: WindowConfig = WindowConfig {
        max_calls: 100,
        window: Duration::from_secs(5),
    };
}

/// Per-command rate limiter. Hold a single instance in Tauri state.
pub struct RateLimiter {
    /// Default window applied to every command not in `overrides`.
    default: WindowConfig,
    /// Optional per-command tighter / looser limits. e.g.
    /// `"recalculate_portfolio" -> (5, 30s)` if we ever want to
    /// throttle expensive ops harder than the default.
    overrides: HashMap<&'static str, WindowConfig>,
    /// Per-command sliding-window timestamps. Each entry's deque
    /// holds the timestamps of the most-recent `max_calls` invocations.
    buckets: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            default: WindowConfig::DEFAULT,
            overrides: HashMap::new(),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Add or replace a per-command override. Builder-style so the
    /// limiter can be constructed in one expression in `lib.rs`.
    #[allow(dead_code)]
    pub fn with_override(
        mut self,
        command: &'static str,
        max_calls: usize,
        window: Duration,
    ) -> Self {
        self.overrides
            .insert(command, WindowConfig { max_calls, window });
        self
    }

    fn config_for(&self, command: &str) -> WindowConfig {
        self.overrides.get(command).copied().unwrap_or(self.default)
    }

    /// Check (and record) one call to `command`. Allowed → returns
    /// `Allow` and the call's timestamp is appended to the bucket.
    /// Denied → returns `Deny { retry_after }` describing how long
    /// the caller should wait; nothing is appended.
    pub fn check(&self, command: &str) -> RateLimitDecision {
        let cfg = self.config_for(command);
        let now = Instant::now();
        let mut buckets = match self.buckets.lock() {
            Ok(g) => g,
            // If the mutex is poisoned (a previous holder panicked),
            // we fall through to Allow. Rate-limiting failing open is
            // strictly better than locking out the entire IPC surface.
            Err(poisoned) => poisoned.into_inner(),
        };
        let bucket = buckets.entry(command.to_string()).or_default();

        // Evict timestamps older than the window. Cheap: at most
        // `max_calls` entries total, walked front-to-back.
        let cutoff = now.checked_sub(cfg.window).unwrap_or(now);
        while let Some(&front) = bucket.front() {
            if front < cutoff {
                bucket.pop_front();
            } else {
                break;
            }
        }

        if bucket.len() < cfg.max_calls {
            bucket.push_back(now);
            RateLimitDecision::Allow
        } else {
            // Caller has to wait at least until the oldest timestamp
            // in the bucket falls outside the window. That's the
            // soonest a new slot opens up. Saturating-sub guards
            // against the (extremely rare) case where Instant math
            // returns a negative duration.
            let oldest = bucket.front().copied().unwrap_or(now);
            let retry_after = (oldest + cfg.window).saturating_duration_since(now);
            RateLimitDecision::Deny { retry_after }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn allows_up_to_max_calls_in_window() {
        let rl = RateLimiter::new();
        // First 100 calls allowed.
        for i in 0..100 {
            assert!(
                matches!(rl.check("foo"), RateLimitDecision::Allow),
                "call {} should be allowed",
                i
            );
        }
        // 101st is denied.
        let decision = rl.check("foo");
        assert!(
            matches!(decision, RateLimitDecision::Deny { .. }),
            "got {:?}",
            decision
        );
    }

    #[test]
    fn different_commands_have_independent_buckets() {
        let rl = RateLimiter::new();
        // Saturate `foo`.
        for _ in 0..100 {
            rl.check("foo");
        }
        assert!(matches!(rl.check("foo"), RateLimitDecision::Deny { .. }));
        // `bar` still untouched.
        assert!(matches!(rl.check("bar"), RateLimitDecision::Allow));
    }

    #[test]
    fn window_evicts_old_timestamps() {
        let rl = RateLimiter::new().with_override("tight", 2, Duration::from_millis(150));
        assert!(matches!(rl.check("tight"), RateLimitDecision::Allow));
        assert!(matches!(rl.check("tight"), RateLimitDecision::Allow));
        // Saturated.
        assert!(matches!(rl.check("tight"), RateLimitDecision::Deny { .. }));
        // Wait for the window to slide past, then we're allowed again.
        sleep(Duration::from_millis(200));
        assert!(matches!(rl.check("tight"), RateLimitDecision::Allow));
    }

    #[test]
    fn deny_reports_plausible_retry_after() {
        let rl = RateLimiter::new().with_override("tight", 1, Duration::from_millis(500));
        rl.check("tight");
        match rl.check("tight") {
            RateLimitDecision::Deny { retry_after } => {
                // Must be > 0 and <= window.
                assert!(retry_after > Duration::ZERO);
                assert!(retry_after <= Duration::from_millis(500));
            }
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn override_takes_precedence_over_default() {
        let rl = RateLimiter::new().with_override("strict", 1, Duration::from_secs(60));
        assert!(matches!(rl.check("strict"), RateLimitDecision::Allow));
        assert!(matches!(rl.check("strict"), RateLimitDecision::Deny { .. }));
        // Default-config commands still get 100 / 5s.
        for _ in 0..100 {
            assert!(matches!(rl.check("lenient"), RateLimitDecision::Allow));
        }
    }

    #[test]
    fn denied_calls_do_not_consume_bucket_slots() {
        // Important invariant: a denied call should NOT extend the
        // window further into the future by appending its timestamp.
        // Otherwise a flapping caller in tight retry would never
        // recover.
        let rl = RateLimiter::new().with_override("tight", 2, Duration::from_millis(150));
        rl.check("tight"); // 1
        rl.check("tight"); // 2
                           // Hammer with denied calls — should NOT push back the recovery.
        for _ in 0..50 {
            assert!(matches!(rl.check("tight"), RateLimitDecision::Deny { .. }));
        }
        sleep(Duration::from_millis(200));
        assert!(matches!(rl.check("tight"), RateLimitDecision::Allow));
    }
}
