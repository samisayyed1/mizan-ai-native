//! Holdings-metadata SQLite reader — Track C PR-C5.d.5 / Goal v3 §V step C5.d.
//!
//! Thin read layer over the `holdings_metadata` table (Track E PR-E1
//! migration `2026-06-02-000001_holdings_metadata`). Used by the
//! insights scheduler to detect AAOIFI screening flips for the
//! `ShariaStatusChanged` rule.
//!
//! Reads only — writes flow through the AAOIFI screening worker
//! (Track E PR-E4) at screening time.

pub mod repository;
pub use repository::{HoldingsMetadataRepository, ShariaStatusFlip};
