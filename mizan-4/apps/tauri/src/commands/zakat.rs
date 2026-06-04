//! Zakat assessment command.
//!
//! Wraps the pure-math + portfolio-aggregator already shipped in
//! `crates/core/src/portfolio/zakat/`. Gated on `zakat_engine` (Gold-only,
//! per Feroz 25 May 2026) — the desktop's UpgradeGate raises automatically
//! on `GatedError("zakat_engine", …)` for Silver users.
//!
//! # Truth Ledger entry — PR-F4.b
//!
//! Every successful `compute_zakat` invocation appends a
//! [`LedgerEntryKind::ZakatComputed`] row to the desktop's hash-chained
//! Truth Ledger per CLAUDE.md §0 rule 1. The audit trail captures
//! school + 5 Decimal components + currency + zakat_due so the user's
//! imam (or an external auditor) can re-derive the number months
//! later. Ledger append failures are logged but do NOT block the
//! return — the user still gets their Zakat number; the audit trail
//! is best-effort with retry via the existing retry queue.

use std::sync::Arc;

use chrono::Utc;
use log::warn;
use rust_decimal::Decimal;
use tauri::State;

use mizan_financial_truth::{build_zakat_append_input, ZakatLedgerInputs};
use mizan_zakat::{School, ZakatReport};

use crate::context::ServiceContext;

/// `compute_zakat(nisab, baseCurrencyOverride?, school?) -> ZakatReport`
///
/// `nisab` is the Zakat threshold in the user's base currency (the most
/// common modern approach uses the silver-Nisab value in their fiat). When
/// `base_currency_override` is `None`, we use the user's configured base
/// currency from settings (the normal path).
///
/// `school` selects the school of jurisprudence — `"hanafi"` (default),
/// `"shafii"`, `"maliki"`, or `"hanbali"`. PR-F2.b.1/c.1 honoured this
/// at the routing level. Unrecognised strings collapse to Hanafi with
/// a clean error message so the desktop UI can surface "unknown school"
/// without crashing.
#[tauri::command(rename_all = "camelCase")]
pub async fn compute_zakat(
    nisab: String,
    base_currency_override: Option<String>,
    school: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ZakatReport, String> {
    // Gold-gated. Silver users get a clean modal pointing them at the Gold tier.
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.zakat_engine,
        "zakat_engine",
        "gold",
        &entitlements.plan,
        "Zakat assessment is a Gold feature. Upgrade to compute it across your full \
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

    // Parse the school selector. Unrecognised strings → Hanafi default,
    // surfaced via the report's notes (school_note() pin).
    let school_selector = school
        .as_deref()
        .map(|s| School::parse(s).unwrap_or_default())
        .unwrap_or_default();

    let report = state
        .zakat_service()
        .assess_portfolio_for_school(school_selector, &base_currency, nisab)
        .await
        .map_err(|e| format!("Failed to compute Zakat: {e}"))?;

    // PR-F4.b: emit a Truth Ledger entry capturing the inputs + result.
    // Best-effort — log failures but never block the return on a
    // ledger transient.
    if let Err(e) = append_zakat_ledger_entry(&state, &report, &base_currency).await {
        warn!(
            "compute_zakat: Truth Ledger append failed (school={:?}): {}; report still returned",
            report.school, e
        );
    }

    Ok(report)
}

/// Build + append a `LedgerEntryKind::ZakatComputed` row to the
/// desktop's Truth Ledger. Returns the append error verbatim so the
/// caller can decide whether to surface or swallow it.
async fn append_zakat_ledger_entry(
    state: &Arc<ServiceContext>,
    report: &ZakatReport,
    base_currency: &str,
) -> Result<(), String> {
    let now = Utc::now();
    // Deterministic id keyed on (school, recorded_at iso second-precision)
    // so re-running the same calculation within the same second is
    // idempotent. Real callers will typically re-run hours apart.
    let entry_id = format!(
        "zakat-{}-{}",
        report
            .school
            .label()
            .to_lowercase()
            .replace(['(', ')', ' '], "-"),
        now.format("%Y%m%dT%H%M%SZ"),
    );

    let inputs = ZakatLedgerInputs {
        id: entry_id,
        school: report.school.label().to_string(),
        // assess_portfolio is portfolio-wide; the desktop's account_id
        // for the TOTAL view is the constant from mizan-core. We surface
        // it on the ledger row so by_account queries find Zakat entries.
        account_id: Some(mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID.to_string()),
        total_assessable: report.total_assessable_assets,
        deductible_debts: report.deductible_debts,
        net_zakat_base: report.net_zakat_base,
        nisab_threshold: report.nisab_threshold,
        is_above_nisab: report.is_above_nisab,
        zakat_due: report.zakat_due,
        currency: base_currency.to_string(),
        recorded_at: Some(now),
    };

    let append_input = build_zakat_append_input(inputs);
    state
        .truth_ledger()
        .append(append_input)
        .await
        .map_err(|e| format!("Truth Ledger append failed: {e}"))?;

    Ok(())
}
