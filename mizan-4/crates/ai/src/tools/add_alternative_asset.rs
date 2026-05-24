//! Add Alternative Asset tool — propose a new alternative asset (property,
//! vehicle, collectible, precious metal, or other) from natural language.
//!
//! Mirrors `create_liability` but for non-liability AlternativeAsset kinds.
//! Produces a DRAFT preview the user confirms; the tool never writes — on
//! confirm the frontend calls the existing `createAlternativeAsset` Tauri
//! command (the same one the legacy add-asset wizard uses).

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAlternativeAssetArgs {
    /// One of: property | vehicle | collectible | precious | other.
    pub kind: String,
    /// User-visible name (e.g. "Brooklyn brownstone", "2020 Tesla Model 3", "Gold coins").
    pub name: String,
    /// Current market value of the asset.
    pub current_value: f64,
    /// ISO-4217 currency code. Defaults to base.
    pub currency: Option<String>,
    /// Optional purchase price for gain calculation.
    pub purchase_price: Option<f64>,
    /// Optional purchase date YYYY-MM-DD.
    pub purchase_date: Option<String>,
    /// Optional ISO 8601 valuation date. Defaults to today on save.
    pub value_date: Option<String>,
    /// Kind-specific metadata. Free-form key→string map.
    ///   Property:    { "sub_type": "residence|rental|land", "address": "..." }
    ///   Vehicle:     { "sub_type": "car|motorcycle|boat|rv", "year": "2020", "make": "...", "model": "..." }
    ///   Collectible: { "sub_type": "art|watch|wine|...", }
    ///   Precious:    { "metal_type": "gold|silver|platinum", "weight": "10", "unit": "oz|g|kg" }
    pub metadata: Option<std::collections::HashMap<String, String>>,
    /// Optional notes.
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAlternativeAssetOutput {
    pub draft: AlternativeAssetDraft,
    pub validation: ValidationResult,
    pub available_kinds: Vec<KindOption>,
    pub available_currencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeAssetDraft {
    pub kind: String,
    pub name: String,
    pub current_value: f64,
    pub currency: String,
    pub purchase_price: Option<f64>,
    pub purchase_date: Option<String>,
    pub value_date: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub is_valid: bool,
    pub missing_fields: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KindOption {
    pub value: String,
    pub label: String,
}

const KINDS: &[(&str, &str)] = &[
    ("property", "Property"),
    ("vehicle", "Vehicle"),
    ("collectible", "Collectible"),
    ("precious", "Precious metal"),
    ("other", "Other"),
];

fn kind_options() -> Vec<KindOption> {
    KINDS
        .iter()
        .map(|(v, l)| KindOption {
            value: (*v).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

fn normalize_kind(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "property" | "real_estate" | "real-estate" | "house" | "home" | "land" | "apartment"
        | "condo" | "rental" | "building" => "property".to_string(),
        "vehicle" | "car" | "auto" | "motorcycle" | "bike" | "boat" | "rv" | "truck" => {
            "vehicle".to_string()
        }
        "collectible" | "art" | "watch" | "watches" | "wine" | "jewelry" | "memorabilia" => {
            "collectible".to_string()
        }
        "precious" | "precious_metal" | "precious-metal" | "gold" | "silver" | "platinum"
        | "metals" => "precious".to_string(),
        _ => "other".to_string(),
    }
}

fn available_currencies(base: &str, hint: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |out: &mut Vec<String>, code: &str| {
        let c = code.trim().to_uppercase();
        if !c.is_empty() && !out.contains(&c) {
            out.push(c);
        }
    };
    push(&mut out, base);
    if let Some(h) = hint {
        push(&mut out, h);
    }
    for code in ["USD", "EUR", "GBP", "CAD", "INR", "AED", "SGD", "AUD", "JPY"] {
        push(&mut out, code);
    }
    out
}

fn is_iso_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").is_ok()
}

pub struct AddAlternativeAssetTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> AddAlternativeAssetTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: AddAlternativeAssetArgs,
    ) -> Result<AddAlternativeAssetOutput, AiError> {
        debug!(
            "add_alternative_asset called: kind={:?}, name={:?}, value={}",
            args.kind, args.name, args.current_value
        );

        let base_currency = self.env.base_currency();
        let kind = normalize_kind(&args.kind);
        let name = args.name.trim().to_string();

        let currency = args
            .currency
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_uppercase())
            .unwrap_or_else(|| base_currency.clone());

        let mut missing_fields: Vec<String> = Vec::new();
        if name.is_empty() {
            missing_fields.push("name".to_string());
        }
        if args.kind.trim().is_empty() {
            missing_fields.push("kind".to_string());
        }
        if args.current_value <= 0.0 {
            missing_fields.push("currentValue".to_string());
        }

        let mut warnings: Vec<String> = Vec::new();

        // Date validations.
        let value_date = match args.value_date.as_deref() {
            Some(s) if !s.trim().is_empty() && is_iso_date(s) => s.trim().to_string(),
            Some(s) if !s.trim().is_empty() => {
                warnings.push(format!(
                    "Could not parse valueDate \"{}\" — using today instead.",
                    s
                ));
                chrono::Utc::now().format("%Y-%m-%d").to_string()
            }
            _ => chrono::Utc::now().format("%Y-%m-%d").to_string(),
        };

        let purchase_date = match args.purchase_date.as_deref() {
            Some(s) if !s.trim().is_empty() && is_iso_date(s) => Some(s.trim().to_string()),
            Some(s) if !s.trim().is_empty() => {
                warnings.push(format!(
                    "Could not parse purchaseDate \"{}\" — expecting YYYY-MM-DD.",
                    s
                ));
                None
            }
            _ => None,
        };

        // Per-kind metadata sanity checks (warnings only — UI lets user edit).
        let mut metadata = args.metadata.unwrap_or_default();
        match kind.as_str() {
            "property"
                if !metadata.contains_key("sub_type") => {
                    metadata.insert("sub_type".to_string(), "residence".to_string());
                }
            "vehicle"
                if !metadata.contains_key("sub_type") => {
                    metadata.insert("sub_type".to_string(), "car".to_string());
                }
            "precious" => {
                if !metadata.contains_key("metal_type") {
                    warnings.push(
                        "Specify metal_type (gold | silver | platinum) in metadata for precious metals."
                            .to_string(),
                    );
                }
                if !metadata.contains_key("unit") {
                    metadata.insert("unit".to_string(), "oz".to_string());
                }
            }
            _ => {}
        }

        Ok(AddAlternativeAssetOutput {
            draft: AlternativeAssetDraft {
                kind,
                name,
                current_value: args.current_value.max(0.0),
                currency: currency.clone(),
                purchase_price: args.purchase_price.filter(|v| *v > 0.0),
                purchase_date,
                value_date,
                metadata,
                notes: args
                    .notes
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty()),
            },
            validation: ValidationResult {
                is_valid: missing_fields.is_empty(),
                missing_fields,
                warnings,
            },
            available_kinds: kind_options(),
            available_currencies: available_currencies(&base_currency, Some(&currency)),
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for AddAlternativeAssetTool<E> {
    const NAME: &'static str = "add_alternative_asset";

    type Error = AiError;
    type Args = AddAlternativeAssetArgs;
    type Output = AddAlternativeAssetOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let base = self.env.base_currency();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Propose a new alternative asset (property, vehicle, collectible, precious metal, \
                or other) the user wants to track on their net-worth picture. NOT for tradable \
                securities — use record_activity / create_account for those. NOT for liabilities — \
                use create_liability. Returns a draft preview the user confirms — never writes. \
                Base currency is {base}."
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "description": "property | vehicle | collectible | precious | other. Synonyms accepted (real_estate → property, car → vehicle, gold/silver → precious).",
                        "enum": ["property", "vehicle", "collectible", "precious", "other"]
                    },
                    "name": { "type": "string", "description": "User-visible name (e.g. 'Brooklyn brownstone')." },
                    "currentValue": {
                        "type": "number",
                        "description": "Current market value in the asset's currency. Must be positive."
                    },
                    "currency": {
                        "type": "string",
                        "description": format!("ISO-4217 code. Defaults to base {base}.")
                    },
                    "purchasePrice": { "type": "number", "description": "Optional purchase price for gain calc." },
                    "purchaseDate": { "type": "string", "description": "Optional ISO 8601 purchase date." },
                    "valueDate": { "type": "string", "description": "ISO 8601 valuation date. Defaults to today." },
                    "metadata": {
                        "type": "object",
                        "description": "Kind-specific key→string map. Property: sub_type/address. Vehicle: sub_type/year/make/model. Collectible: sub_type. Precious: metal_type/weight/unit.",
                        "additionalProperties": { "type": "string" }
                    },
                    "notes": { "type": "string" }
                },
                "required": ["kind", "name", "currentValue"]
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
    use crate::env::test_env::MockEnvironment;
    use std::collections::HashMap;

    fn tool() -> AddAlternativeAssetTool<MockEnvironment> {
        AddAlternativeAssetTool::new(Arc::new(MockEnvironment::new()))
    }

    #[tokio::test]
    async fn produces_valid_property_draft() {
        let mut meta = HashMap::new();
        meta.insert("address".to_string(), "123 Main St".to_string());
        let out = tool()
            .build_output(AddAlternativeAssetArgs {
                kind: "real_estate".into(),
                name: "Brooklyn brownstone".into(),
                current_value: 850_000.0,
                currency: Some("USD".into()),
                purchase_price: Some(620_000.0),
                purchase_date: Some("2018-06-01".into()),
                metadata: Some(meta),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.kind, "property");
        // default sub_type added
        assert_eq!(out.draft.metadata.get("sub_type").map(|s| s.as_str()), Some("residence"));
        assert_eq!(out.draft.metadata.get("address").map(|s| s.as_str()), Some("123 Main St"));
    }

    #[tokio::test]
    async fn vehicle_default_sub_type_added() {
        let out = tool()
            .build_output(AddAlternativeAssetArgs {
                kind: "car".into(),
                name: "2020 Tesla Model 3".into(),
                current_value: 28_000.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.kind, "vehicle");
        assert_eq!(out.draft.metadata.get("sub_type").map(|s| s.as_str()), Some("car"));
    }

    #[tokio::test]
    async fn precious_metal_warns_without_metal_type() {
        let out = tool()
            .build_output(AddAlternativeAssetArgs {
                kind: "gold".into(),
                name: "Gold coins".into(),
                current_value: 5_000.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.kind, "precious");
        // default unit added
        assert_eq!(out.draft.metadata.get("unit").map(|s| s.as_str()), Some("oz"));
        assert!(out.validation.warnings.iter().any(|w| w.contains("metal_type")));
    }

    #[tokio::test]
    async fn flags_missing_value() {
        let out = tool()
            .build_output(AddAlternativeAssetArgs {
                kind: "vehicle".into(),
                name: "Car".into(),
                current_value: 0.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .missing_fields
            .iter()
            .any(|f| f == "currentValue"));
    }

    #[tokio::test]
    async fn unknown_kind_falls_back_to_other() {
        let out = tool()
            .build_output(AddAlternativeAssetArgs {
                kind: "spaceship".into(),
                name: "SpaceX rocket".into(),
                current_value: 1_000_000.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.kind, "other");
    }
}
