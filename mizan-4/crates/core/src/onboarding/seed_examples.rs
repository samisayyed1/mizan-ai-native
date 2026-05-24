//! Seed the user's local store with three example liabilities on first
//! onboarding completion.
//!
//! Feroz 25 May 2026: *"Seed 3 dummy liabilities on first launch — mortgage,
//! student loan, credit card. Editing beats blank-staring."*
//!
//! All three rows carry `metadata.example = true` and a name prefixed
//! with `"Example — "` so the frontend can render them with a soft amber
//! border and a "Tap to make real" hint. On first edit, the prefix +
//! flag are stripped automatically.
//!
//! The function is **idempotent**: it bails out cleanly if any liability
//! already exists, so re-running on an already-onboarded device is a no-op.

use chrono::Utc;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;

use crate::assets::AssetKind;
use crate::assets::{AlternativeAssetServiceTrait, CreateAlternativeAssetRequest};
use crate::Result;

/// Name prefix used by every seeded example. The frontend strips this on
/// first edit. Kept in code (not just config) so the frontend renderer
/// and the seeder can't drift.
pub const EXAMPLE_NAME_PREFIX: &str = "Example — ";

/// Seed three example liabilities — mortgage, student loan, credit card —
/// IF the user has no alternative-asset liabilities yet.
///
/// Safe to call on every settings update; the existence check short-circuits.
/// Failures on individual seed rows do NOT roll back earlier ones (each
/// liability is a standalone asset record), but every error bubbles up.
pub async fn seed_example_liabilities(
    service: &Arc<dyn AlternativeAssetServiceTrait>,
    base_currency: &str,
) -> Result<usize> {
    if has_any_liability(service)? {
        return Ok(0);
    }

    let today = Utc::now().date_naive();
    let originated_mortgage = chrono::NaiveDate::from_ymd_opt(2023, 1, 15).unwrap_or(today);
    // value_date must be > purchase_date — using today is always safe.
    let value_date = today;

    let mortgage = CreateAlternativeAssetRequest {
        kind: AssetKind::Liability,
        name: format!("{EXAMPLE_NAME_PREFIX}Home mortgage"),
        currency: base_currency.to_string(),
        current_value: dec!(480_000),
        value_date,
        purchase_price: None,
        purchase_date: Some(originated_mortgage),
        metadata: Some(json!({
            "example": true,
            "sub_type": "mortgage",
            "rate_pct": "5.2",
            "monthly_payment": "2650",
            "originated_at": "2023-01-15"
        })),
        linked_asset_id: None,
    };

    let student_loan = CreateAlternativeAssetRequest {
        kind: AssetKind::Liability,
        name: format!("{EXAMPLE_NAME_PREFIX}Student loan"),
        currency: base_currency.to_string(),
        current_value: dec!(32_000),
        value_date,
        purchase_price: None,
        purchase_date: None,
        metadata: Some(json!({
            "example": true,
            "sub_type": "student_loan",
            "rate_pct": "6.8",
            "monthly_payment": "410"
        })),
        linked_asset_id: None,
    };

    let credit_card = CreateAlternativeAssetRequest {
        kind: AssetKind::Liability,
        name: format!("{EXAMPLE_NAME_PREFIX}Credit card"),
        currency: base_currency.to_string(),
        current_value: dec!(4_800),
        value_date,
        purchase_price: None,
        purchase_date: None,
        metadata: Some(json!({
            "example": true,
            "sub_type": "credit_card",
            "rate_pct": "22.99",
            "monthly_payment": "145"
        })),
        linked_asset_id: None,
    };

    let mut created = 0usize;
    for req in [mortgage, student_loan, credit_card] {
        service.create_alternative_asset(req).await?;
        created += 1;
    }
    Ok(created)
}

fn has_any_liability(service: &Arc<dyn AlternativeAssetServiceTrait>) -> Result<bool> {
    let holdings = service.get_alternative_holdings()?;
    Ok(holdings.iter().any(|h| h.kind == AssetKind::Liability))
}
