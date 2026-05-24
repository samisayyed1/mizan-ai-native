//! Portfolio health command — deterministic Gold monitoring metric.
//!
//! Thin wrapper over `mizan_core::portfolio::health::score_health`. The
//! frontend assembles `HealthInputs` from `useHoldings` + the configured
//! target allocation; this command is the math entry only.
//!
//! Gated on `advanced_reports` for the full breakdown. The simpler
//! "score-only" preview on the Home card is computed client-side from
//! cached holdings — this command is what powers the full report and the
//! detailed driver tooltips.

use std::sync::Arc;

use tauri::State;

use mizan_core::portfolio::health::{score_health, HealthInputs, HealthReport};

use crate::context::ServiceContext;

/// `compute_portfolio_health(inputs) -> HealthReport`
#[tauri::command(rename_all = "camelCase")]
pub async fn compute_portfolio_health(
    inputs: HealthInputs,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HealthReport, String> {
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.advanced_reports,
        "advanced_reports",
        "pro",
        &entitlements.plan,
        "The portfolio health score with full driver breakdown is a Pro feature.",
    )?;

    Ok(score_health(inputs))
}
