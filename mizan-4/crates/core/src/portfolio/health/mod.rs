//! Portfolio health score — Pro headline metric (M4.2).
//!
//! Produces a 0–100 composite health rating with per-driver breakdown.
//! Higher = healthier. The math is deterministic and uses only the
//! caller-supplied summary (no holdings service access here); the Tauri
//! command assembles inputs from the existing holdings + allocation
//! services and passes them in.
//!
//! Drivers (each scored 0–100, higher = better):
//! - **Concentration**: `100 * (1 - top_holding_share)`. A single
//!   position at 50% of the portfolio scores 50; a perfectly diversified
//!   portfolio scores ~100.
//! - **FX exposure**: `100 * (1 - non_base_share)`. The closer the
//!   portfolio is to the user's base currency, the higher.
//! - **Cash drag**: penalises cash above a 20% target — at 0–20% you
//!   score 100; at 100% cash you score 0.
//! - **Allocation drift**: `100 - sum(|actual - target| / 2 * 100)`
//!   where the divisor `2` accounts for double-counting (an overweight in
//!   X always equals an underweight elsewhere). Drift ≥ 100% (impossible
//!   given the math) clamps to 0.
//!
//! Composite = equal-weighted average of the four drivers.
//!
//! **Not financial advice.** This is a heuristic score; users should
//! consult an advisor for genuine portfolio review.

mod health_model;
mod health_service;

pub use health_model::*;
pub use health_service::*;
