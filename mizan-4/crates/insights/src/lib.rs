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
//!
//! # Crate boundary (Track H PR-H3.c)
//!
//! Extracted out of `mizan-core::insights` so the 95% coverage floor +
//! mutation-testing rules can be enforced per crate. The engine still
//! emits `mizan_core::notifications::Notification` values — that type
//! lives in mizan-core today. Lifting Notification into
//! `mizan-domain-types` would let mizan-insights become a true leaf
//! crate; tracked as PR-H3.c.1 follow-up.

mod input;
mod rules;
mod todays_signal;

#[cfg(test)]
mod tests;

pub use input::{
    BondMaturityCandidate, CashDragOpportunityCandidate, ConcentrationRiskFinding, DividendEvent,
    FxPairMove, GoalProgress, HawlAnchorCandidate, HoldingDayMove, InsightsInput,
    NetWorthHistoryPoint, ShariaStatusChange, SyncFailureInput, TaxOptimizationWindow,
};
pub use rules::evaluate;
pub use todays_signal::{pick_todays_signal, score_signal};

use thiserror::Error;

/// Engine-level error surface. Today the rules engine is infallible —
/// every `evaluate(&InsightsInput)` returns `Vec<Notification>` directly
/// — but the error type is declared at the crate boundary so future
/// rules that need fallible work (e.g. reading from a precomputed cache)
/// have a stable surface to grow into.
#[derive(Debug, Error)]
pub enum InsightsError {
    /// A rule received malformed input (e.g. negative price, NaN-like
    /// state) that the typed input couldn't have caught.
    #[error("invalid insights input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, InsightsError>;
