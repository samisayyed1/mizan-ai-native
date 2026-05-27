//! Deterministic wealth-insights engine (Notify-2).
//!
//! Pure functions that take a snapshot of "what changed" and emit a
//! `Vec<Notification>`. No LLM, no I/O, no database — the scheduler
//! hydrates the input by querying snapshot/quote/goal services, then
//! hands the bundle here.
//!
//! Why deterministic? Three reasons:
//!   1. **Truth.** Numbers that drive notifications come from the
//!      same valuation pipeline that drives the dashboard. We don't
//!      want a hallucinated "you're up 12%" line on the lock screen.
//!   2. **Cost.** LLM calls are billed per-token. Running rules on
//!      every snapshot tick would burn credits with no upside — the
//!      AI's job is to *narrate* the rule output once a day, not
//!      detect events.
//!   3. **Testability.** Pure functions get hard-coded fixtures +
//!      golden-output tests. The AI digest layer (Notify-4) wraps
//!      the deterministic output in natural language; if a rule
//!      changes the wrap follows automatically.
//!
//! The "AI-native" mental model: deterministic engine = the *senses*,
//! AI = the *voice*. The senses are never wrong; the voice describes
//! what the senses saw.

mod input;
mod rules;

#[cfg(test)]
mod tests;

pub use input::{
    DividendEvent, GoalProgress, HoldingDayMove, InsightsInput, NetWorthHistoryPoint,
    SyncFailureInput,
};
pub use rules::evaluate;
