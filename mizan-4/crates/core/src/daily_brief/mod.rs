//! Investor Daily Brief — §A22 first cut.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A22: each morning generate a small
//! structured "what changed" summary the user can read in Settings →
//! Notifications, and eventually receive by email (Gold tier).
//!
//! Brief contents:
//!   - Net worth delta vs yesterday (absolute + %)
//!   - Top three holding movers in the period
//!   - Allocation drift > 5% from the user's target
//!   - Any newly-stale FX / quote provider
//!   - Any pending AI drafts the user didn't confirm
//!
//! This module ships the model + computation + persistence trait +
//! in-memory implementation + tests. Email transport (SendGrid)
//! requires deployment + secrets and is stubbed; the brief is
//! still useful in the UI without it.

mod model;
mod service;

pub use model::{
    AllocationDriftEntry, DailyBrief, HoldingMover, NetWorthDelta, PendingDraftSummary,
    StaleSignal,
};
pub use service::{InMemoryDailyBriefService, DailyBriefService};
