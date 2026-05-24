//! Amortization schedule command — deterministic liability payoff engine.
//!
//! Thin wrapper over `mizan_core::portfolio::amortization::build_schedule`.
//! Gated on `advanced_reports` so the desktop's UpgradeGate raises
//! cleanly for Silver users.
//!
//! The frontend assembles `AmortizationInputs` from the user's existing
//! liability record (alternative-asset metadata + balance). Keeping the
//! command input-driven (rather than `liability_id` lookup) keeps the
//! command pure and lets the assistant tool reuse the same math without
//! threading the alt-asset service.

use std::sync::Arc;

use tauri::State;

use mizan_core::portfolio::amortization::{build_schedule, AmortizationInputs, AmortizationReport};

use crate::context::ServiceContext;

/// `compute_amortization(inputs) -> AmortizationReport`
#[tauri::command(rename_all = "camelCase")]
pub async fn compute_amortization(
    inputs: AmortizationInputs,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<AmortizationReport, String> {
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.advanced_reports,
        "advanced_reports",
        "pro",
        &entitlements.plan,
        "Loan payoff reports are a Pro feature. Upgrade to project amortization \
         schedules and total interest cost across the life of your liabilities.",
    )?;

    Ok(build_schedule(inputs))
}
