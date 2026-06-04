//! Hawl-anchor SQLite reader — Track C PR-C5.d.1 / Goal v3 §V step C5.d.
//!
//! Thin read layer over the `hawl_anchors` table (Track F PR-F1
//! migration `2026-06-02-000003_hawl_anchors`). Used by the insights
//! scheduler to hydrate `InsightsInput.hawl_anchors_approaching`
//! before calling `mizan_insights::evaluate()`.
//!
//! Reads only — writes to `hawl_anchors` go through the Zakat engine
//! at evaluation time (per the migration header note: "durable
//! financial state — never evicted by cache_policy worker").

pub mod repository;
pub use repository::{HawlAnchorRepository, HawlAnchorRow};
