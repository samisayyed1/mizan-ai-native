//! Net Worth Snapshot — §A12 first cut.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A12: capture a daily net worth
//! point per user so the dashboard can show a real history line
//! instead of a static today-only number. Snapshots are write-once
//! per day (a re-run on the same day updates the existing row); the
//! history is read by both the existing dashboard chart and the §A22
//! daily-brief delta computation.
//!
//! This module ships the model + trait + in-memory implementation +
//! tests. The SQLite-backed implementation + migration land in a
//! follow-on PR once the daily-snapshot scheduler picks the right
//! emit time (UTC midnight vs. user-local midnight is a real design
//! decision and shouldn't smuggle through with a "good enough" default).

mod model;
mod service;

pub use model::{NetWorthBreakdownEntry, NetWorthSnapshot, NetWorthSnapshotInput, SnapshotSource};
pub use service::{InMemoryNetWorthSnapshotService, NetWorthSnapshotService};
