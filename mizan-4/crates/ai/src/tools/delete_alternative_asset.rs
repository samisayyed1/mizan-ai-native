//! Delete Alternative Asset tool — propose removal of a non-tradable
//! asset (property, vehicle, collectible, precious metal, private equity).
//!
//! Companion to `add_alternative_asset`. Alternative assets are stored as
//! `Asset` rows with `kind in { PROPERTY, VEHICLE, COLLECTIBLE,
//! PRECIOUS_METAL, PRIVATE_EQUITY }`; this tool resolves a reference + returns
//! a draft preview the user confirms before the cascade-delete fires.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAlternativeAssetArgs {
    /// Asset id (preferred — get_holdings returns it) or a name match.
    pub asset_ref: String,
    /// Optional reason carried onto the confirm card.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAlternativeAssetOutput {
    pub target: AssetSnapshot,
    pub reason: Option<String>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSnapshot {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub display_code: Option<String>,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<AssetSnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
}

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
    let kind = h.asset_kind.as_ref().map(kind_label).unwrap_or("OTHER").to_string();
    let name = h
        .instrument
        .as_ref()
        .and_then(|i| i.name.clone())
        .unwrap_or_else(|| "Asset".to_string());
    let display_code = h.instrument.as_ref().map(|i| i.symbol.clone()).filter(|s| !s.is_empty());
    let asset_id = h
        .instrument
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| h.id.clone());
    AssetSnapshot {
        id: asset_id,
        name,
        kind,
        display_code,
        currency: h.local_currency.clone(),
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

fn symbol_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .map(|i| i.symbol.clone())
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
        .filter(|h| name_of(h).to_lowercase() == lower || symbol_of(h).to_lowercase() == lower)
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
        .filter(|h| {
            name_of(h).to_lowercase().contains(&lower)
                || symbol_of(h).to_lowercase().contains(&lower)
        })
        .map(|h| (*h).clone())
        .collect();
    match substring.len() {
        0 => Resolution::NotFound,
        1 => Resolution::Single(Box::new(substring.into_iter().next().unwrap())),
        _ => Resolution::Ambiguous(substring),
    }
}

pub struct DeleteAlternativeAssetTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteAlternativeAssetTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: DeleteAlternativeAssetArgs,
    ) -> Result<DeleteAlternativeAssetOutput, AiError> {
        debug!(
            "delete_alternative_asset called: ref={:?}, reason={:?}",
            args.asset_ref, args.reason
        );

        let base_currency = self.env.base_currency();
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID, &base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolution = resolve(&holdings, &args.asset_ref);

        match resolution {
            Resolution::Single(a) => Ok(DeleteAlternativeAssetOutput {
                target: snapshot(&a),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: true,
                    candidates: Vec::new(),
                    warnings: vec![
                        "Deleting this asset is irreversible. Activities tied to it (purchase, \
                         valuations, sale) will cascade-delete with it. If the user sold the asset \
                         and wants the history kept, record a TRANSFER_OUT or SELL activity \
                         instead — that closes the position without losing the audit trail."
                            .to_string(),
                    ],
                    is_valid: true,
                },
            }),
            Resolution::Ambiguous(candidates) => Ok(DeleteAlternativeAssetOutput {
                target: AssetSnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} alternative assets. Ask the user which one they \
                         mean before retrying.",
                        args.asset_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                },
            }),
            Resolution::NotFound => Ok(DeleteAlternativeAssetOutput {
                target: AssetSnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No alternative asset matching \"{}\" found among PROPERTY / VEHICLE / \
                         COLLECTIBLE / PRECIOUS_METAL / PRIVATE_EQUITY rows. Use get_holdings to \
                         list them.",
                        args.asset_ref
                    )],
                    is_valid: false,
                },
            }),
            Resolution::Missing => Ok(DeleteAlternativeAssetOutput {
                target: AssetSnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![
                        "assetRef is required — give the asset name, display code, or id."
                            .to_string(),
                    ],
                    is_valid: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteAlternativeAssetTool<E> {
    const NAME: &'static str = "delete_alternative_asset";

    type Error = AiError;
    type Args = DeleteAlternativeAssetArgs;
    type Output = DeleteAlternativeAssetOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Propose deletion of a non-tradable (alternative) asset — property, vehicle, \
                 collectible, precious-metal holding, or private-equity stake. Use when the \
                 user sold one and doesn't want the row anymore, or created an example that \
                 should be removed. Returns a draft preview the user confirms. Does NOT cover \
                 tradable securities (those go through closing activities) or liabilities (use \
                 delete_liability instead). If the user sold but wants history preserved, \
                 record a SELL / TRANSFER_OUT activity instead."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetRef": {
                        "type": "string",
                        "description": "Asset id, name, or display_code of the alternative \
                         asset to delete."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason from the user — surfaced on the \
                         confirm card. E.g. 'I sold the apartment', 'this was a duplicate row'."
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

    fn alt_holding(asset_id: &str, name: &str, kind: AssetKind) -> Holding {
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
            market_value: MonetaryValue::zero(),
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

    fn tool_with_holdings(holdings: Vec<Holding>) -> DeleteAlternativeAssetTool<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.holdings_service = Arc::new(MockHoldingsService { holdings });
        DeleteAlternativeAssetTool::new(Arc::new(env))
    }

    #[tokio::test]
    async fn resolves_property_by_name() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Lake cabin", AssetKind::Property)]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "Lake cabin".into(),
                reason: Some("sold".into()),
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.target.id, "p1");
        assert_eq!(out.target.kind, "PROPERTY");
        assert_eq!(out.reason.as_deref(), Some("sold"));
    }

    #[tokio::test]
    async fn resolves_vehicle_by_id() {
        let tool = tool_with_holdings(vec![alt_holding("v1", "2021 Tesla", AssetKind::Vehicle)]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "v1".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.target.kind, "VEHICLE");
    }

    #[tokio::test]
    async fn ambiguous_match_blocks_confirm() {
        let tool = tool_with_holdings(vec![
            alt_holding("p1", "Apartment unit A", AssetKind::Property),
            alt_holding("p2", "Apartment unit B", AssetKind::Property),
        ]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "apartment".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert_eq!(out.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn missing_ref_blocks_confirm() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Cabin", AssetKind::Property)]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
    }

    #[tokio::test]
    async fn unknown_ref_returns_actionable_warning() {
        let tool = tool_with_holdings(vec![alt_holding("p1", "Cabin", AssetKind::Property)]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "ferrari".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(!out.validation.warnings.is_empty());
    }

    #[tokio::test]
    async fn tradable_asset_kinds_are_ignored() {
        // Equity holding must NOT match the alternative-asset resolver — it
        // scopes to PROPERTY/VEHICLE/COLLECTIBLE/PRECIOUS_METAL/PRIVATE_EQUITY.
        let mut equity = alt_holding("eq-1", "Apple Inc", AssetKind::Property);
        equity.asset_kind = Some(AssetKind::Investment);
        let tool = tool_with_holdings(vec![equity]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "Apple".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
    }

    #[tokio::test]
    async fn private_equity_kind_resolves() {
        let tool = tool_with_holdings(vec![alt_holding("pe1", "Acme PE Fund VII", AssetKind::PrivateEquity)]);
        let out = tool
            .build_output(DeleteAlternativeAssetArgs {
                asset_ref: "Acme".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.target.kind, "PRIVATE_EQUITY");
    }
}
