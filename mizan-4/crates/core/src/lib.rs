//! Mizan Core - Domain entities, services, and traits.
//!
//! This crate contains the core business logic for Mizan.
//! It is database-agnostic and defines traits that are implemented
//! by the `storage-sqlite` crate.

pub mod accounts;
pub mod activities;
pub mod addons;
pub mod assets;
pub mod constants;
pub mod custom_provider;
pub mod daily_brief;
pub mod errors;
pub mod events;
pub mod fx;
pub mod goals;
pub mod health;
// Track H PR-H3.c — insights moved out of mizan-core into the
// `mizan-insights` crate. Consumers import directly:
//   use mizan_insights::{evaluate, InsightsInput, ...};
pub mod limits;
pub mod mizan_error;
pub mod net_worth_snapshot;
pub mod news;
pub mod notifications;
pub mod onboarding;
#[cfg(test)]
pub mod perf_budget;
pub mod planning;
pub mod portfolio;
pub mod quotes;
pub mod secrets;
pub mod settings;
pub mod sync;
pub mod sync_ledger;
pub mod taxonomies;
// Track H PR-H3.a — truth_engine moved out of mizan-core into the new
// `mizan-financial-truth` crate. Consumers import directly:
//   use mizan_financial_truth::{TruthLedger, AppendInput, LedgerEntryKind, ...};
pub mod utils;

// Re-export common types from asset and portfolio modules
pub use assets::*;
pub use portfolio::*;

// Re-export error types
pub use errors::Error;
pub use errors::Result;

// Re-export the structured Mizan error contract (§A24).
pub use mizan_error::{
    wrap as wrap_mizan_error, DataSafetyStatus, MizanError, MizanErrorSeverity, RetryPolicy,
};
