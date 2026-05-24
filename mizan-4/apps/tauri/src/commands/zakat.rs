//! Zakat assessment command.
//!
//! Wraps the pure-math + portfolio-aggregator already shipped in
//! `crates/core/src/portfolio/zakat/`. Gated on `advanced_reports`
//! (Pro+) — the desktop's UpgradeGate raises automatically on
//! `GatedError("advanced_reports", …)` for Free/Basic users.

use std::sync::Arc;

use rust_decimal::Decimal;
use tauri::State;

use mizan_core::portfolio::zakat::ZakatReport;

use crate::context::ServiceContext;

/// `compute_zakat(nisab, baseCurrencyOverride?) -> ZakatReport`
///
/// `nisab` is the Zakat threshold in the user's base currency (the most
/// common modern approach uses the silver-Nisab value in their fiat). When
/// `base_currency_override` is `None`, we use the user's configured base
/// currency from settings (the normal path).
#[tauri::command(rename_all = "camelCase")]
pub async fn compute_zakat(
    nisab: String,
    base_currency_override: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ZakatReport, String> {
    // Pro-gated. Free users get a clean modal pointing them at the Pro tier.
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.advanced_reports,
        "advanced_reports",
        "pro",
        &entitlements.plan,
        "Zakat assessment is a Pro feature. Upgrade to compute it across your full \
         portfolio, with deductions for short-term debts.",
    )?;

    let nisab = nisab
        .trim()
        .parse::<Decimal>()
        .map_err(|e| format!("Invalid nisab amount: {e}"))?;
    if nisab.is_sign_negative() {
        return Err("Nisab must be non-negative".to_string());
    }

    let base_currency = base_currency_override.unwrap_or_else(|| state.get_base_currency());

    state
        .zakat_service()
        .assess_portfolio(&base_currency, nisab)
        .await
        .map_err(|e| format!("Failed to compute Zakat: {e}"))
}
