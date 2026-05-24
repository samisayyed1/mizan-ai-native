//! Stream-level guard rails for the agentic loop.
//!
//! Implements rig's `StreamingPromptHook` to prevent three failure modes we see
//! with small open-source models (qwen, ministral, etc.):
//!
//! 1. **Tool-call storms** — the model re-emits the same tool call dozens of
//!    times per turn. Detected via a per-`(tool, args)` counter in
//!    [`MizanStreamHook::on_tool_call`]. Repeated calls are short-circuited
//!    with `ToolCallHookAction::Skip { reason }` — which rig feeds back to the
//!    model as the tool result, so the model sees the cached data it already
//!    received and a nudge to stop re-calling.
//!
//! 2. **Global runaway** — total tool calls across a turn can still blow up even
//!    with dedup. Hard cap via `ToolCallHookAction::Terminate` when over
//!    [`MAX_TOTAL_TOOL_CALLS`].
//!
//! 3. **Token-level repetition loops** — the model streams
//!    `"get_performance, get_performance, ..."` indefinitely without ever
//!    emitting a well-formed tool call (qwen thinking mode). Detected in
//!    [`MizanStreamHook::on_text_delta`] by looking for a short suffix
//!    that already appears many times in the trailing buffer, or by the
//!    stream exceeding [`MAX_STREAM_CHARS`].
//!
//! The hook is `Clone` per rig's trait bound; cheap because state is behind
//! an `Arc<Mutex<_>>`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use log::{debug, warn};
use rig::agent::{HookAction, StreamingPromptHook, ToolCallHookAction};
use rig::completion::CompletionModel;
use rig::message::Message;

use crate::safety::AiSafetyRuntime;

/// Maximum distinct identical `(tool_name, args_json)` calls we'll execute.
/// Beyond this count the hook returns a "stop calling this" skip. Two is
/// enough: first call executes, second re-request serves as a tolerance for
/// benign retries, third and beyond get the nudge.
const MAX_DUPLICATE_CALLS: usize = 2;

/// Absolute cap on tool calls across one streamed turn. Calibrated against
/// LibreChat's `recursionLimit` (default 25) — we're slightly tighter because
/// this is a desktop UX, not a server agent.
const MAX_TOTAL_TOOL_CALLS: usize = 20;

/// Maximum streamed text+reasoning characters before we assume runaway
/// generation and terminate. Jan's default `max_tokens` is 2048 (~8000 chars);
/// we allow significantly more so legitimate long answers pass through.
const MAX_STREAM_CHARS: usize = 80_000;

/// Trailing window inspected for repetition at the end of the stream.
const REPETITION_TAIL_WINDOW: usize = 1024;

/// Length of the suffix used as the repetition probe.
const REPETITION_PROBE_LEN: usize = 24;

/// How many times the probe must appear in the tail to count as a loop.
const REPETITION_MIN_REPEATS: usize = 8;

#[derive(Default)]
struct HookState {
    /// Per `(tool_name, args_json)` call counter.
    tool_call_counts: HashMap<String, usize>,
    /// Previously executed tool results, keyed the same way. Populated by
    /// `on_tool_result` so that a `Skip` response can feed the real data back
    /// to the model instead of a bare error.
    tool_result_cache: HashMap<String, String>,
    /// Total tool calls seen this stream.
    total_tool_calls: usize,
}

#[derive(Default, Clone)]
pub struct MizanStreamHook {
    state: Arc<Mutex<HookState>>,
    /// §A6 audit emitter. When set, every tool-call attempt + outcome is
    /// recorded to the structured audit log. None disables audit (used by
    /// tests where the runtime isn't wired).
    safety: Option<Arc<AiSafetyRuntime>>,
    /// Per-stream thread + turn id used as the audit row key.
    audit_thread_id: Arc<Mutex<String>>,
    audit_turn_index: Arc<Mutex<u32>>,
}

impl MizanStreamHook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a §A6 safety runtime so every tool-call attempt + result is
    /// audited. The runtime's per-turn cap is INFORMATIONAL here — the
    /// stream hook's own `MAX_TOTAL_TOOL_CALLS` is what stops the run.
    /// Both fire on excess; the runtime's cap exceeding is logged as
    /// `CapExceeded` in the audit trail.
    pub fn with_safety(mut self, safety: Arc<AiSafetyRuntime>) -> Self {
        self.safety = Some(safety);
        self
    }

    /// Set the thread id + turn index used to key audit rows.
    pub fn with_audit_keys(self, thread_id: impl Into<String>, turn_index: u32) -> Self {
        if let Ok(mut tid) = self.audit_thread_id.lock() {
            *tid = thread_id.into();
        }
        if let Ok(mut ti) = self.audit_turn_index.lock() {
            *ti = turn_index;
        }
        self
    }

    fn key(tool_name: &str, args: &str) -> String {
        format!("{}::{}", tool_name, args)
    }

    /// Snapshot of the current audit keys. Cheap clones.
    fn audit_keys(&self) -> (String, u32) {
        let tid = self
            .audit_thread_id
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let ti = self.audit_turn_index.lock().map(|t| *t).unwrap_or(0);
        (tid, ti)
    }
}

impl<M: CompletionModel> StreamingPromptHook<M> for MizanStreamHook {
    fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> impl std::future::Future<Output = ToolCallHookAction> + Send {
        let state = self.state.clone();
        let tool_name = tool_name.to_string();
        let args = args.to_string();
        async move {
            let key = Self::key(&tool_name, &args);
            let Ok(mut state) = state.lock() else {
                return ToolCallHookAction::Continue;
            };

            state.total_tool_calls += 1;
            let total_so_far = state.total_tool_calls;
            if total_so_far > MAX_TOTAL_TOOL_CALLS {
                // §A6 audit — emit a CapExceeded row before terminating
                // the run so the audit trail captures the wall-hit.
                if let Some(ref safety) = self.safety {
                    let (tid, ti) = self.audit_keys();
                    let _ = safety.register(&tid, ti, &tool_name).err().map(|entry| {
                        safety.emit(&entry);
                    });
                }
                warn!(
                    "Tool-call cap tripped: {} total calls this turn — terminating",
                    total_so_far
                );
                return ToolCallHookAction::terminate(
                    "The model exceeded the tool-call limit for a single turn. \
                     Ending the run; ask the user to rephrase or switch to a more capable model.",
                );
            }

            // §A6 audit — register + emit for every dispatched tool call.
            // The runtime's own per-turn counter parallels the stream
            // hook's counter; they may diverge if the runtime has a
            // tighter cap (DEFAULT 8 vs stream MAX 20), in which case
            // the runtime's `CapExceeded` row is still useful diagnostic.
            if let Some(ref safety) = self.safety {
                let (tid, ti) = self.audit_keys();
                match safety.register(&tid, ti, &tool_name) {
                    Ok(entry) | Err(entry) => safety.emit(&entry),
                }
            }

            let count = state.tool_call_counts.entry(key.clone()).or_insert(0);
            *count += 1;
            let hits = *count;

            if hits > MAX_DUPLICATE_CALLS {
                warn!(
                    "Duplicate tool-call guard tripped: {}({}) called {} times",
                    tool_name, args, hits
                );
                let reason = match state.tool_result_cache.get(&key) {
                    Some(cached) => format!(
                        "You have already called `{tool_name}` with these arguments {hits} times \
                         and received this result:\n\n{cached}\n\n\
                         Stop calling this tool. Write the final answer to the user using the data above.",
                    ),
                    None => format!(
                        "You have already called `{tool_name}` with these arguments {hits} times. \
                         Stop calling this tool. Write the final answer to the user using the data \
                         you already have in the conversation."
                    ),
                };
                return ToolCallHookAction::skip(reason);
            }

            debug!(
                "Tool call allowed: {}({}) [hit {}/{}]",
                tool_name, args, hits, MAX_DUPLICATE_CALLS
            );
            ToolCallHookAction::Continue
        }
    }

    fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
        result: &str,
    ) -> impl std::future::Future<Output = HookAction> + Send {
        let state = self.state.clone();
        let key = Self::key(tool_name, args);
        let result = result.to_string();
        async move {
            if let Ok(mut state) = state.lock() {
                state.tool_result_cache.insert(key, result);
            }
            HookAction::Continue
        }
    }

    fn on_text_delta(
        &self,
        _text_delta: &str,
        aggregated_text: &str,
    ) -> impl std::future::Future<Output = HookAction> + Send {
        let is_repetitive = is_repetitive(aggregated_text);
        let total = aggregated_text.chars().count();
        async move {
            if total > MAX_STREAM_CHARS {
                warn!(
                    "Stream length cap tripped ({} > {} chars) — terminating",
                    total, MAX_STREAM_CHARS
                );
                return HookAction::terminate(
                    "The model produced more text than allowed for a single turn. \
                     Ending the run; it was likely stuck.",
                );
            }
            if is_repetitive {
                warn!("Repetition guard tripped on streamed text — terminating");
                return HookAction::terminate(
                    "The model got stuck repeating itself. \
                     Ending the run; try rephrasing the question or switching to a more capable model.",
                );
            }
            HookAction::Continue
        }
    }

    async fn on_completion_call(&self, _prompt: &Message, _history: &[Message]) -> HookAction {
        HookAction::Continue
    }
}

/// Detects repetition at the tail of the aggregated text: look at the last
/// `REPETITION_PROBE_LEN` chars and count how many times that suffix appears
/// in the last `REPETITION_TAIL_WINDOW` chars of the stream. We only check
/// once the tail is long enough to be meaningful, and skip if the probe is
/// all whitespace (legitimate trailing newlines shouldn't trip the guard).
fn is_repetitive(aggregated_text: &str) -> bool {
    let total_bytes = aggregated_text.len();
    if total_bytes < REPETITION_PROBE_LEN * REPETITION_MIN_REPEATS {
        return false;
    }

    // Slice the trailing window on char boundaries.
    let tail_byte_start = aggregated_text
        .char_indices()
        .rev()
        .take(REPETITION_TAIL_WINDOW)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let tail = &aggregated_text[tail_byte_start..];

    if tail.len() < REPETITION_PROBE_LEN * REPETITION_MIN_REPEATS {
        return false;
    }

    // Probe = last REPETITION_PROBE_LEN chars of the tail.
    let probe_byte_start = tail
        .char_indices()
        .rev()
        .take(REPETITION_PROBE_LEN)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let probe = &tail[probe_byte_start..];
    if probe.trim().is_empty() {
        return false;
    }

    tail.matches(probe).count() >= REPETITION_MIN_REPEATS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_repetitive_text_passes() {
        let text = "This is a normal response about your portfolio. \
                    It includes several different sentences with distinct content. \
                    Market values, cost basis, and performance metrics are discussed.";
        assert!(!is_repetitive(text));
    }

    #[test]
    fn short_text_passes() {
        assert!(!is_repetitive("short"));
    }

    #[test]
    fn detects_qwen_style_tool_repetition() {
        let mut buf = String::from("Let me check the tools available: ");
        for _ in 0..30 {
            buf.push_str("get_performance, ");
        }
        assert!(is_repetitive(&buf));
    }

    #[test]
    fn detects_word_level_repetition() {
        let mut buf = String::from("The user asked about performance. ");
        for _ in 0..50 {
            buf.push_str("the same answer ");
        }
        assert!(is_repetitive(&buf));
    }

    #[test]
    fn long_diverse_text_passes() {
        let mut buf = String::new();
        for i in 0..200 {
            buf.push_str(&format!("Sentence number {} with unique content. ", i));
        }
        assert!(!is_repetitive(&buf));
    }
}
