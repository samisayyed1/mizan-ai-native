//! Update Alternative Asset tool — edit a property / vehicle / collectible /
//! precious metal / private equity entry from natural language.
//!
//! Companion to `add_alternative_asset` / `delete_alternative_asset`. When the
//! user says "the house is worth 1.2M now" or "the cabin is renting at 4500
//! a month" or "rename the Tesla to Model Y", the AI calls this with the
//! target reference + the fields it wants to change. Returns a DRAFT
//! preview the user confirms; the tool itself never writes. On confirm,
//! the frontend hits two adapter endpoints: `updateAlternativeAssetMetadata`
//! (name + metadata patch) and `updateAlternativeAssetValuation` (new value).

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAlternativeAssetArgs {
    /// Asset id (preferred, returned by get_holdings) or a name match.
    pub asset_ref: String,
    /// New name. Omit to keep current; "" clears the user-facing label
    /// back to the canonical kind label ("Property", "Vehicle", …).
    pub name: Option<String>,
    /// New free-form notes. Omit to keep current; "" clears.
    pub notes: Option<String>,
    /// New current value in the asset's local currency. Triggers a fresh
    /// valuation row dated today. Omit to leave valuation untouched.
    pub current_value: Option<f64>,
    /// Optional metadata patch — common kind-specific fields the AI
    /// can set without knowing the full schema. Unknown keys are dropped
    /// with a warning; null values clear the key. Recognised:
    ///   address (string), year (number), mileage (number),
    ///   monthly_rent (number), tenant_name (string), lease_start (date),
    ///   lease_end (date), purity (number), serial (string).
    pub metadata: Option<std::collections::BTreeMap<String, Option<serde_json::Value>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAlternativeAssetOutput {
    pub draft: AssetUpdateDraft,
    pub current: AssetSnapshot,
    pub diff: Vec<FieldDiff>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSnapshot {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub currency: String,
    pub current_value: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUpdateDraft {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub currency: String,
    pub current_value: Option<f64>,
    pub notes: Option<String>,
    /// Metadata keys that will be SET (with their new value) or CLEARED
    /// (null) once the user confirms. Empty when only name/notes/value
    /// change.
    pub metadata_patch: std::collections::BTreeMap<String, Option<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub field: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<AssetSnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
    pub has_changes: bool,
}

const RECOGNISED_METADATA_KEYS: &[&str] = &[
    "address",
    "year",
    "mileage",
    "monthly_rent",
    "tenant_name",
    "lease_start",
    "lease_end",
    "purity",
    "serial",
];

fn kind_label(kind: &mizan_core::assets::AssetKind) -> &'static str {
    use mizan_core::assets::AssetKind::*;
    match kind {
        Property => "PROPERTY",
        Vehicle => "VEHICLE",
        Collectible => "COLLECTIBLE",
        PreciousMetal => "PRECIOUS_METAL",
        PrivateEquity => "PRIVATE_EQUITY",
        _ => "OTHER",
    }
}

fn is_alternative(kind: &mizan_core::assets::AssetKind) -> bool {
    use mizan_core::assets::AssetKind::*;
    matches!(
        kind,
        Property | Vehicle | Collectible | PreciousMetal | PrivateEquity
    )
}

fn snapshot(h: &mizan_core::holdings::Holding) -> AssetSnapshot {
    let kind = h
        .asset_kind
        .as_ref()
        .map(|k| kind_label(k).to_string())
        .unwrap_or_else(|| "OTHER".to_string());
    let name = h
        .instrument
        .as_ref()
        .and_then(|i| i.name.clone())
        .unwrap_or_else(|| "Asset".to_string());
    let id = h
        .instrument
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| h.id.clone());
    // For alternative assets, "current value" is the local market value
    // — quantity is 1 and market_value.local IS the asset's value.
    use rust_decimal::prelude::ToPrimitive;
    let current_value = h.market_value.local.to_f64().filter(|v| *v > 0.0);
    let notes = h
        .instrument
        .as_ref()
        .and_then(|i| i.notes.clone())
        .filter(|s| !s.is_empty());
    AssetSnapshot {
        id,
        name,
        kind,
        currency: h.local_currency.clone(),
        current_value,
        notes,
    }
}

enum Resolution {
    Single(Box<mizan_core::holdings::Holding>),
    Ambiguous(Vec<mizan_core::holdings::Holding>),
    NotFound,
    Missing,
}

fn name_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .and_then(|i| i.name.clone())
        .unwrap_or_default()
}

fn asset_id_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| h.id.clone())
}

fn resolve(holdings: &[mizan_core::holdings::Holding], reference: &str) -> Resolution {
    let r = reference.trim();
    if r.is_empty() {
        return Resolution::Missing;
    }
    let alternatives: Vec<&mizan_core::holdings::Holding> = holdings
        .iter()
        .filter(|h| h.asset_kind.as_ref().is_some_and(is_alternative))
        .collect();

    if let Some(h) = alternatives.iter().find(|h| asset_id_of(h) == r) {
        return Resolution::Single(Box::new((*h).clone()));
    }
    let lower = r.to_lowercase();
    let exact: Vec<_> = alternatives
        .iter()
        .filter(|h| name_of(h).to_lowercase() == lower)
        .map(|h| (*h).clone())
        .collect();
    if exact.len() == 1 {
        return Resolution::Single(Box::new(exact.into_iter().next().unwrap()));
    }
    if exact.len() > 1 {
        return Resolution::Ambiguous(exact);
    }
    let substring: Vec<_> = alternatives
        .iter()
        .filter(|h| name_of(h).to_lowercase().contains(&lower))
        .map(|h| (*h).clone())
        .collect();
    match substring.len() {
        0 => Resolution::NotFound,
        1 => Resolution::Single(Box::new(substring.into_iter().next().unwrap())),
        _ => Resolution::Ambiguous(substring),
    }
}

pub struct UpdateAlternativeAssetTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> UpdateAlternativeAssetTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: UpdateAlternativeAssetArgs,
    ) -> Result<UpdateAlternativeAssetOutput, AiError> {
        debug!("update_alternative_asset called: ref={:?}", args.asset_ref);

        let base_currency = self.env.base_currency();
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID, &base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut warnings: Vec<String> = Vec::new();

        match resolve(&holdings, &args.asset_ref) {
            Resolution::Single(boxed) => {
                let current = snapshot(&boxed);
                let mut draft = AssetUpdateDraft {
                    id: current.id.clone(),
                    name: current.name.clone(),
                    kind: current.kind.clone(),
                    currency: current.currency.clone(),
                    current_value: current.current_value,
                    notes: current.notes.clone(),
                    metadata_patch: std::collections::BTreeMap::new(),
                };
                let mut diff: Vec<FieldDiff> = Vec::new();

                if let Some(n) = args.name.as_ref() {
                    let trimmed = n.trim();
                    if trimmed != current.name {
                        diff.push(FieldDiff {
                            field: "name".to_string(),
                            old: Some(current.name.clone()),
                            new: Some(trimmed.to_string()),
                        });
                        draft.name = trimmed.to_string();
                    }
                }

                if let Some(notes_in) = args.notes.as_ref() {
                    let trimmed_owned: String = notes_in.trim().to_string();
                    let new_notes = if trimmed_owned.is_empty() {
                        None
                    } else {
                        Some(trimmed_owned)
                    };
                    if new_notes != current.notes {
                        diff.push(FieldDiff {
                            field: "notes".to_string(),
                            old: current.notes.clone(),
                            new: new_notes.clone(),
                        });
                        draft.notes = new_notes;
                    }
                }

                if let Some(v) = args.current_value {
                    if !v.is_finite() || v < 0.0 {
                        warnings.push(format!(
                            "Current value {} is invalid — drop it or send a positive number.",
                            v
                        ));
                    } else if Some(v) != current.current_value {
                        diff.push(FieldDiff {
                            field: "currentValue".to_string(),
                            old: current.current_value.map(|x| format!("{:.2}", x)),
                            new: Some(format!("{:.2}", v)),
                        });
                        draft.current_value = Some(v);
                    }
                }

                if let Some(md) = args.metadata.as_ref() {
                    for (key, value) in md {
                        if !RECOGNISED_METADATA_KEYS.contains(&key.as_str()) {
                            warnings.push(format!(
                                "Unknown metadata key '{}' — dropped. Recognised: {}.",
                                key,
                                RECOGNISED_METADATA_KEYS.join(", ")
                            ));
                            continue;
                        }
                        let new_repr = value.as_ref().map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        });
                        diff.push(FieldDiff {
                            field: format!("metadata.{}", key),
                            old: None,
                            new: new_repr,
                        });
                        draft.metadata_patch.insert(key.clone(), value.clone());
                    }
                }

                let has_changes = !diff.is_empty();
                if !has_changes {
                    warnings.push("Nothing to change — every field matched the current asset.".to_string());
                }

                Ok(UpdateAlternativeAssetOutput {
                    draft,
                    current,
                    diff,
                    validation: ValidationResult {
                        resolved: true,
                        candidates: Vec::new(),
                        warnings,
                        is_valid: has_changes,
                        has_changes,
                    },
                })
            }
            Resolution::Ambiguous(candidates) => Ok(UpdateAlternativeAssetOutput {
                draft: AssetUpdateDraft::default(),
                current: AssetSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} alternative assets. Ask the user which one they mean.",
                        args.asset_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                    has_changes: false,
                },
            }),
            Resolution::NotFound => Ok(UpdateAlternativeAssetOutput {
                draft: AssetUpdateDraft::default(),
                current: AssetSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No alternative asset matching \"{}\" found. Use get_holdings to list them.",
                        args.asset_ref
                    )],
                    is_valid: false,
                    has_changes: false,
                },
            }),
            Resolution::Missing => Ok(UpdateAlternativeAssetOutput {
                draft: AssetUpdateDraft::default(),
                current: AssetSnapshot::default(),
                diff: Vec::new(),
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![
                        "assetRef is required — pass the asset name or id.".to_string(),
                    ],
                    is_valid: false,
                    has_changes: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for UpdateAlternativeAssetTool<E> {
    const NAME: &'static str = "update_alternative_asset";

    type Error = AiError;
    type Args = UpdateAlternativeAssetArgs;
    type Output = UpdateAlternativeAssetOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Edit an existing alternative asset (property, vehicle, collectible, precious \
                 metal, private equity). Use to rename, update notes, push a new current \
                 valuation, or patch kind-specific metadata (rental income for properties, \
                 mileage for vehicles, purity for metals, …). Returns a DRAFT preview with a \
                 per-field diff the user confirms before writing. Prefer this over \
                 delete+create when the user wants to adjust an existing entry — preserves \
                 valuation history."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetRef": {
                        "type": "string",
                        "description": "Asset id (from get_holdings) or name match."
                    },
                    "name": { "type": "string", "description": "New name; \"\" to clear." },
                    "notes": { "type": "string", "description": "Free-form notes; \"\" to clear." },
                    "currentValue": {
                        "type": "number",
                        "description": "New current market value in the asset's local currency. \
                         Creates a fresh valuation row dated today."
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Patch for kind-specific metadata. Recognised keys: \
                         address, year, mileage, monthly_rent, tenant_name, lease_start, \
                         lease_end, purity, serial. Send null to clear a key."
                    }
                },
                "required": ["assetRef"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.build_output(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::{MockEnvironment, MockHoldingsService};
    use chrono::NaiveDate;
    use mizan_core::{
        assets::AssetKind,
        holdings::{Holding, HoldingType, Instrument, MonetaryValue},
    };
    use rust_decimal::Decimal;
    use std::collections::BTreeMap;

    fn alt_holding(asset_id: &str, name: &str, kind: AssetKind, value: i64) -> Holding {
        Holding {
            id: format!("h-{}", asset_id),
            account_id: "TOTAL".to_string(),
            holding_type: HoldingType::Cash,
            instrument: Some(Instrument {
                id: asset_id.to_string(),
                symbol: asset_id.to_string(),
                name: Some(name.to_string()),
                currency: "USD".to_string(),
                notes: None,
                pricing_mode: "MANUAL".to_string(),
                preferred_provider: None,
                exchange_mic: None,
                classifications: None,
                metadata: None,
            }),
            asset_kind: Some(kind),
            quantity: Decimal::ONE,
            open_date: None,
            lots: None,
            contract_multiplier: Decimal::ONE,
            local_currency: "USD".to_string(),
            base_currency: "USD".to_string(),
            fx_rate: None,
            market_value: MonetaryValue {
                local: Decimal::from(value),
                base: Decimal::from(value),
            },
            cost_basis: None,
            price: None,
            purchase_price: None,
            unrealized_gain: None,
            unrealized_gain_pct: None,
            realized_gain: None,
            realized_gain_pct: None,
            dividend_income: None,
            total_gain: None,
            total_gain_pct: None,
            day_change: None,
            day_change_pct: None,
            prev_close_value: None,
            weight: Decimal::ZERO,
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
            metadata: None,
        }
    }

    fn tool_with_holdings(holdings: Vec<Holding>) -> UpdateAlternativeAssetTool<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.holdings_service = Arc::new(MockHoldingsService { holdings });
        UpdateAlternativeAssetTool::new(Arc::new(env))
    }

    #[tokio::test]
    async fn rename_emits_diff() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Lake cabin", AssetKind::Property, 800_000)]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "Lake cabin".into(),
                name: Some("Mountain cabin".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.name, "Mountain cabin");
        assert!(out.diff.iter().any(|d| d.field == "name"));
    }

    #[tokio::test]
    async fn raise_value_emits_diff() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "House", AssetKind::Property, 1_000_000)]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "p1".into(),
                current_value: Some(1_200_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.current_value, Some(1_200_000.0));
        let diff = out.diff.iter().find(|d| d.field == "currentValue").unwrap();
        assert_eq!(diff.new.as_deref(), Some("1200000.00"));
    }

    #[tokio::test]
    async fn invalid_value_warns_and_drops() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "House", AssetKind::Property, 1_000_000)]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "p1".into(),
                current_value: Some(-1.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.warnings.iter().any(|w| w.contains("invalid")));
        assert!(!out.validation.has_changes);
    }

    #[tokio::test]
    async fn recognised_metadata_keys_pass() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Rental", AssetKind::Property, 500_000)]);
        let mut md = BTreeMap::new();
        md.insert("monthly_rent".to_string(), Some(serde_json::json!(4500)));
        md.insert("tenant_name".to_string(), Some(serde_json::json!("Acme LLC")));
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "Rental".into(),
                metadata: Some(md),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.draft.metadata_patch.contains_key("monthly_rent"));
        assert!(out.draft.metadata_patch.contains_key("tenant_name"));
        assert!(out.diff.iter().any(|d| d.field == "metadata.monthly_rent"));
    }

    #[tokio::test]
    async fn unknown_metadata_key_dropped_with_warning() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "House", AssetKind::Property, 1_000_000)]);
        let mut md = BTreeMap::new();
        md.insert("zip_code".to_string(), Some(serde_json::json!("90210")));
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "House".into(),
                metadata: Some(md),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.warnings.iter().any(|w| w.contains("zip_code")));
        assert!(!out.draft.metadata_patch.contains_key("zip_code"));
    }

    #[tokio::test]
    async fn no_op_warns() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Cabin", AssetKind::Property, 800_000)]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "Cabin".into(),
                current_value: Some(800_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.warnings.iter().any(|w| w.contains("Nothing to change")));
    }

    #[tokio::test]
    async fn ambiguous_match_blocks_confirm() {
        let tool = tool_with_holdings(vec![
            alt_holding("p1", "Apartment A", AssetKind::Property, 500_000),
            alt_holding("p2", "Apartment B", AssetKind::Property, 600_000),
        ]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "Apartment".into(),
                current_value: Some(700_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert_eq!(out.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn tradable_assets_filtered_out() {
        let mut equity = alt_holding("eq-1", "Apple", AssetKind::Property, 0);
        equity.asset_kind = Some(AssetKind::Investment);
        let tool = tool_with_holdings(vec![equity]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "Apple".into(),
                name: Some("AAPL".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
    }

    #[tokio::test]
    async fn missing_ref_blocks() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "House", AssetKind::Property, 1_000_000)]);
        let out = tool
            .build_output(UpdateAlternativeAssetArgs {
                asset_ref: "  ".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.warnings.iter().any(|w| w.contains("required")));
    }
}
