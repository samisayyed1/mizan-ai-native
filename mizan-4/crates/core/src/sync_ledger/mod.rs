//! Sync Run Ledger — §A4 first cut.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A4: every sync attempt (Plaid,
//! SnapTrade, Yahoo, TradingView, manual CSV import, AI tool-driven
//! create/update) writes an append-only audit row capturing what was
//! attempted, when, by what provider, in what mode, with what summary
//! and what error (if any). The user can replay the history in
//! Settings → Sync, and support can read it via the §A17 diagnostic
//! bundle.
//!
//! This module ships the model + trait + in-memory implementation +
//! tests. The SQLite-backed implementation + migration land in a
//! follow-on PR once the Diesel schema work is staged carefully.

mod model;
mod service;

pub use model::{SyncRunEntry, SyncRunMode, SyncRunOutcome, SyncRunProvider, SyncRunSummary};
pub use service::{InMemorySyncRunLedger, SyncRunLedger};
