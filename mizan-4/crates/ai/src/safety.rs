//! AI Safety Runtime — §A6 first cut.
//!
//! Per `MIZAN_AI_NATIVE_PLAN.md` v3.1 §A6, the assistant needs hard
//! guardrails *outside* the system prompt:
//!
//! 1. Tools that mutate must return drafts only. Already enforced by
//!    Phase B: every write tool builds a draft and never calls the
//!    persistence service. Nothing for this module to do.
//!
//! 2. Reject AI-claimed outputs that contradict service data. Each
//!    tool's `build_output` re-reads the underlying state on every
//!    call, so the AI's claim is replaced by ground truth. Out of
//!    scope for this module.
//!
//! 3. Hard cap on tool calls per turn (THIS MODULE).
//! 4. Numeric bounds on writes (THIS MODULE — helpers).
//! 5. All tool calls logged to an audit trail (THIS MODULE — emit hook).
//!
//! This module ships the data structures + assertion helpers + audit
//! emit hook so the chat-runtime layer can adopt them site-by-site.
//! Wiring into the actual dispatcher lands in a follow-on PR.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Default per-turn cap. Eight is generous enough for a real "build my
/// whole portfolio" conversation (account + 3 holdings + 3 alt assets +
/// a goal) but tight enough to catch a runaway LLM loop in the first
/// extra call. Bump only with a strong reason — every extra unit of
/// headroom is one more unit of damage a misbehaving turn can cause.
pub const DEFAULT_TOOL_CALLS_PER_TURN: u32 = 8;

/// Outcome of a tool call attempt — emitted to the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallOutcome {
    /// Tool call dispatched normally.
    Dispatched,
    /// Cap exceeded — call refused.
    CapExceeded,
    /// Tool returned an error.
    Failed,
}

/// One row in the audit trail. Kept structurally so a later persistence
/// layer can just `Serialize` and write to a `tool_call_audit` table.
#[derive(Debug, Clone)]
pub struct ToolCallAuditEntry {
    pub thread_id: String,
    pub turn_index: u32,
    pub call_index_in_turn: u32,
    pub tool_name: String,
    pub outcome: ToolCallOutcome,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Process-wide tool-call governor. Tracks per-thread per-turn counts
/// in memory + emits structured `log::warn!` lines for the audit trail.
/// Persistent storage of audit rows lands in a follow-on PR once §A4
/// (Sync Run Ledger) defines the storage shape.
pub struct AiSafetyRuntime {
    cap_per_turn: u32,
    counters: Mutex<HashMap<(String, u32), u32>>,
}

impl AiSafetyRuntime {
    pub fn new() -> Self {
        Self::with_cap(DEFAULT_TOOL_CALLS_PER_TURN)
    }

    pub fn with_cap(cap_per_turn: u32) -> Self {
        Self {
            cap_per_turn,
            counters: Mutex::new(HashMap::new()),
        }
    }

    pub fn cap(&self) -> u32 {
        self.cap_per_turn
    }

    /// Register that a tool call is about to be dispatched. Returns the
    /// audit row to log, or an error variant if the cap was hit.
    ///
    /// Side effect: increments the in-memory counter for `(thread_id, turn_index)`.
    /// Call this exactly once per attempted tool call. The runtime should
    /// emit the returned audit row to its logger.
    pub fn register(
        self: &Arc<Self>,
        thread_id: &str,
        turn_index: u32,
        tool_name: &str,
    ) -> Result<ToolCallAuditEntry, ToolCallAuditEntry> {
        let mut counters = self.counters.lock().expect("safety counters mutex poisoned");
        let counter = counters
            .entry((thread_id.to_string(), turn_index))
            .or_insert(0);
        *counter += 1;
        let call_index_in_turn = *counter;
        let allowed = call_index_in_turn <= self.cap_per_turn;
        drop(counters);

        let entry = ToolCallAuditEntry {
            thread_id: thread_id.to_string(),
            turn_index,
            call_index_in_turn,
            tool_name: tool_name.to_string(),
            outcome: if allowed {
                ToolCallOutcome::Dispatched
            } else {
                ToolCallOutcome::CapExceeded
            },
            timestamp: chrono::Utc::now(),
        };

        if allowed {
            Ok(entry)
        } else {
            Err(entry)
        }
    }

    /// Record the final outcome of a dispatched tool call (success/failure).
    /// Returns an updated audit row with the right outcome stamped in.
    /// Does NOT modify the per-turn counter — that's already incremented
    /// by `register`.
    pub fn record_outcome(
        self: &Arc<Self>,
        thread_id: &str,
        turn_index: u32,
        tool_name: &str,
        call_index_in_turn: u32,
        succeeded: bool,
    ) -> ToolCallAuditEntry {
        ToolCallAuditEntry {
            thread_id: thread_id.to_string(),
            turn_index,
            call_index_in_turn,
            tool_name: tool_name.to_string(),
            outcome: if succeeded {
                ToolCallOutcome::Dispatched
            } else {
                ToolCallOutcome::Failed
            },
            timestamp: chrono::Utc::now(),
        }
    }

    /// Emit an audit row to the logger. Format is stable so support /
    /// release engineers can grep for `[mizan.ai.audit]` in app logs.
    pub fn emit(&self, entry: &ToolCallAuditEntry) {
        log::warn!(
            "[mizan.ai.audit] thread={} turn={} call={} tool={} outcome={:?} at={}",
            entry.thread_id,
            entry.turn_index,
            entry.call_index_in_turn,
            entry.tool_name,
            entry.outcome,
            entry.timestamp.to_rfc3339()
        );
    }

    /// Reset a thread's per-turn counter — call when a new user message
    /// starts a fresh turn. Cheap O(1) hash drop.
    pub fn end_turn(self: &Arc<Self>, thread_id: &str, turn_index: u32) {
        let mut counters = self.counters.lock().expect("safety counters mutex poisoned");
        counters.remove(&(thread_id.to_string(), turn_index));
    }
}

impl Default for AiSafetyRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Numeric bounds (§A6 item 4) ───────────────────────────────────
//
// Pure helpers — no state. Each tool's `build_output` can call these
// before constructing the draft so the LLM can never push obviously-bad
// numbers into a draft the user might confirm by accident.

/// Reject non-positive amounts. Returns `Err` with the bound that was
/// violated so call sites can `.map_err(|msg| MizanError::new("X", msg))`
/// directly.
pub fn require_positive(field: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{} must be a finite number (got {})", field, value));
    }
    if value <= 0.0 {
        return Err(format!("{} must be greater than 0 (got {})", field, value));
    }
    Ok(())
}

/// Reject percentages outside the realistic 0-100 band. Bounds are
/// inclusive on both sides. Used by `create_liability` (rate_pct),
/// `update_liability`, and any future tool that takes a rate.
pub fn require_percentage(field: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{} must be a finite number (got {})", field, value));
    }
    if !(0.0..=100.0).contains(&value) {
        return Err(format!(
            "{} must be between 0 and 100 (got {})",
            field, value
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_lets_first_n_calls_through() {
        let r = Arc::new(AiSafetyRuntime::with_cap(3));
        for i in 1..=3 {
            let ok = r.register("t1", 0, "create_account").unwrap();
            assert_eq!(ok.call_index_in_turn, i);
            assert_eq!(ok.outcome, ToolCallOutcome::Dispatched);
        }
    }

    #[test]
    fn cap_blocks_the_n_plus_first_call() {
        let r = Arc::new(AiSafetyRuntime::with_cap(2));
        let _ = r.register("t1", 0, "a").unwrap();
        let _ = r.register("t1", 0, "b").unwrap();
        let blocked = r.register("t1", 0, "c").unwrap_err();
        assert_eq!(blocked.outcome, ToolCallOutcome::CapExceeded);
        assert_eq!(blocked.call_index_in_turn, 3);
    }

    #[test]
    fn cap_is_per_turn_not_global() {
        let r = Arc::new(AiSafetyRuntime::with_cap(2));
        let _ = r.register("t1", 0, "a").unwrap();
        let _ = r.register("t1", 0, "b").unwrap();
        // New turn — counter resets.
        let ok = r.register("t1", 1, "c").unwrap();
        assert_eq!(ok.call_index_in_turn, 1);
        assert_eq!(ok.outcome, ToolCallOutcome::Dispatched);
    }

    #[test]
    fn cap_is_per_thread() {
        let r = Arc::new(AiSafetyRuntime::with_cap(1));
        let _ = r.register("t1", 0, "a").unwrap();
        let blocked = r.register("t1", 0, "b").unwrap_err();
        assert_eq!(blocked.outcome, ToolCallOutcome::CapExceeded);
        // Different thread — its own quota.
        let ok = r.register("t2", 0, "a").unwrap();
        assert_eq!(ok.call_index_in_turn, 1);
    }

    #[test]
    fn end_turn_frees_the_counter() {
        let r = Arc::new(AiSafetyRuntime::with_cap(1));
        let _ = r.register("t1", 0, "a").unwrap();
        let blocked = r.register("t1", 0, "b").unwrap_err();
        assert_eq!(blocked.outcome, ToolCallOutcome::CapExceeded);
        r.end_turn("t1", 0);
        // After end_turn, the same turn_index is fresh again.
        let ok = r.register("t1", 0, "a").unwrap();
        assert_eq!(ok.call_index_in_turn, 1);
    }

    #[test]
    fn record_outcome_does_not_increment_counter() {
        let r = Arc::new(AiSafetyRuntime::with_cap(1));
        let dispatched = r.register("t1", 0, "a").unwrap();
        assert_eq!(dispatched.call_index_in_turn, 1);
        // Record a failure for that call — does not bump the counter.
        let _ = r.record_outcome("t1", 0, "a", 1, false);
        // Next register would be call #2 — still cap-blocked.
        let blocked = r.register("t1", 0, "b").unwrap_err();
        assert_eq!(blocked.call_index_in_turn, 2);
    }

    #[test]
    fn require_positive_validates() {
        assert!(require_positive("principal", 100.0).is_ok());
        assert!(require_positive("principal", 0.0).is_err());
        assert!(require_positive("principal", -1.0).is_err());
        assert!(require_positive("principal", f64::NAN).is_err());
        assert!(require_positive("principal", f64::INFINITY).is_err());
    }

    #[test]
    fn require_percentage_validates() {
        assert!(require_percentage("rate", 5.2).is_ok());
        assert!(require_percentage("rate", 0.0).is_ok());
        assert!(require_percentage("rate", 100.0).is_ok());
        assert!(require_percentage("rate", -0.1).is_err());
        assert!(require_percentage("rate", 100.1).is_err());
        assert!(require_percentage("rate", f64::NAN).is_err());
    }
}
