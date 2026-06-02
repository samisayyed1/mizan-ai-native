//! Update Liability tool — edit-first companion to create_liability.
//!
//! Primary use: "replace the Example mortgage with my real one" — the user
//! tweaks the seeded liability instead of deleting + recreating. Phase C
//! seeds three Example liabilities on first onboarding; this tool is the
//! AI's path for converting them into the user's real numbers.
//!
//! The LLM passes an `assetId` (it can read existing liabilities via the
//! standard holdings tools) plus whichever fields it wants to change. The
//! tool produces a DRAFT preview with a per-field diff; the frontend tool
//! UI applies the change by calling the existing
//! `updateAlternativeAssetMetadata` and/or `updateAlternativeAssetValuation`
//! adapters on confirm.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLiabilityArgs {
    /// Asset id of the existing liability (e.g. "LIAB-…" or whatever prefix
    /// the local store uses). Required.
    pub asset_id: String,
    /// New user-visible name. Empty string clears any "Example —" prefix.
    pub name: Option<String>,
    /// New outstanding balance / principal in the same currency.
    pub principal: Option<f64>,
    /// New interest rate as a percentage.
    pub rate_pct: Option<f64>,
    /// New monthly payment.
    pub monthly_payment: Option<f64>,
    /// New originated_at ISO 8601 date.
    pub originated_at: Option<String>,
    /// Updated liability subtype (mortgage | student_loan | credit_card | …).
    pub liability_type: Option<String>,
    /// New notes (empty string clears).
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLiabilityOutput {
    pub draft: LiabilityUpdateDraft,
    pub validation: ValidationResult,
    pub available_subtypes: Vec<LiabilitySubtypeOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityUpdateDraft {
    pub asset_id: String,
    /// `None` for any field the user did NOT mention — UI keeps current.
    pub name: Option<String>,
    pub principal: Option<f64>,
    pub rate_pct: Option<f64>,
    pub monthly_payment: Option<f64>,
    pub originated_at: Option<String>,
    pub liability_type: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub is_valid: bool,
    pub missing_fields: Vec<String>,
    pub warnings: Vec<String>,
    /// True when at least one field actually changes.
    pub has_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilitySubtypeOption {
    pub value: String,
    pub label: String,
}

const SUBTYPES: &[(&str, &str)] = &[
    ("mortgage", "Mortgage"),
    ("student_loan", "Student loan"),
    ("credit_card", "Credit card"),
    ("personal_loan", "Personal loan"),
    ("auto_loan", "Auto loan"),
    ("heloc", "HELOC / equity line"),
    ("other", "Other"),
];

fn subtype_options() -> Vec<LiabilitySubtypeOption> {
    SUBTYPES
        .iter()
        .map(|(v, l)| LiabilitySubtypeOption {
            value: (*v).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

fn normalize_subtype(raw: &str) -> Option<String> {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "mortgage" | "home_loan" | "home-loan" => Some("mortgage".to_string()),
        "student_loan" | "student-loan" | "studentloan" | "tuition_loan" => {
            Some("student_loan".to_string())
        }
        "credit_card" | "credit-card" | "creditcard" | "cc" => Some("credit_card".to_string()),
        "personal_loan" | "personal-loan" | "personalloan" | "loan" => {
            Some("personal_loan".to_string())
        }
        "auto_loan" | "auto-loan" | "car_loan" | "car-loan" | "autoloan" => {
            Some("auto_loan".to_string())
        }
        "heloc" | "home_equity" | "home-equity" | "equity_line" => Some("heloc".to_string()),
        "other" => Some("other".to_string()),
        _ => None,
    }
}

fn is_iso_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").is_ok()
}

pub struct UpdateLiabilityTool<E: AiEnvironment> {
    // Kept for type signature parity with the other AI tools (they all
    // hold `env: Arc<E>` so the tool registry can store them uniformly).
    // This tool only emits drafts so the env is unused by `build_output`;
    // the field remains for future read paths (e.g. validating asset_id
    // exists before drafting).
    #[allow(dead_code)]
    env: Arc<E>,
}

impl<E: AiEnvironment> UpdateLiabilityTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: UpdateLiabilityArgs,
    ) -> Result<UpdateLiabilityOutput, AiError> {
        debug!(
            "update_liability called: asset_id={}, name={:?}, principal={:?}",
            args.asset_id, args.name, args.principal
        );

        let mut missing_fields: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        if args.asset_id.trim().is_empty() {
            missing_fields.push("assetId".to_string());
        }

        // Drop fields that don't actually change anything (empty strings → None).
        let name = args.name.as_ref().map(|s| s.trim().to_string());
        let liability_type = match args.liability_type.as_deref() {
            Some(s) if s.trim().is_empty() => None,
            Some(s) => match normalize_subtype(s) {
                Some(t) => Some(t),
                None => {
                    warnings.push(format!("Unknown liability type \"{}\" — ignoring.", s));
                    None
                }
            },
            None => None,
        };

        if let Some(rate) = args.rate_pct {
            if !(0.0..=100.0).contains(&rate) {
                warnings.push(format!(
                    "Rate {}% looks unusual — confirm before saving.",
                    rate
                ));
            }
        }
        if let Some(monthly) = args.monthly_payment {
            if monthly < 0.0 {
                warnings.push("Monthly payment should be non-negative.".to_string());
            }
        }
        if let Some(principal) = args.principal {
            if principal < 0.0 {
                warnings.push("Principal should be non-negative.".to_string());
            }
        }

        let originated_at = match args.originated_at.as_deref() {
            Some(s) if s.trim().is_empty() => None,
            Some(s) if is_iso_date(s) => Some(s.trim().to_string()),
            Some(s) => {
                warnings.push(format!(
                    "Could not parse originatedAt \"{}\" — expecting YYYY-MM-DD.",
                    s
                ));
                None
            }
            None => None,
        };

        let notes = args.notes.as_ref().map(|s| s.trim().to_string());

        let has_changes = name.is_some()
            || args.principal.is_some()
            || args.rate_pct.is_some()
            || args.monthly_payment.is_some()
            || originated_at.is_some()
            || liability_type.is_some()
            || notes.is_some();

        if !has_changes && missing_fields.is_empty() {
            warnings.push("No fields changed — pass at least one field to update.".to_string());
        }

        Ok(UpdateLiabilityOutput {
            draft: LiabilityUpdateDraft {
                asset_id: args.asset_id.trim().to_string(),
                name,
                principal: args.principal,
                rate_pct: args.rate_pct,
                monthly_payment: args.monthly_payment,
                originated_at,
                liability_type,
                notes,
            },
            validation: ValidationResult {
                is_valid: missing_fields.is_empty() && has_changes,
                missing_fields,
                warnings,
                has_changes,
            },
            available_subtypes: subtype_options(),
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for UpdateLiabilityTool<E> {
    const NAME: &'static str = "update_liability";

    type Error = AiError;
    type Args = UpdateLiabilityArgs;
    type Output = UpdateLiabilityOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Edit an existing liability (mortgage, loan, credit card). Use this — \
                NOT create_liability — when the user wants to modify a liability they already \
                have, especially to replace one of the pre-seeded Example liabilities with their \
                real numbers (edit-first UX per Feroz 25 May 2026). Returns a DRAFT preview the \
                user confirms — never writes directly. Currency is immutable (carried for display)."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetId": {
                        "type": "string",
                        "description": "Asset id of the liability to update (call get_holdings or list_alt_holdings first if you don't have it)."
                    },
                    "name": { "type": "string", "description": "New name. Pass empty string to clear the 'Example — ' prefix." },
                    "principal": { "type": "number", "description": "New outstanding balance / principal." },
                    "ratePct": { "type": "number", "description": "New interest rate as a percentage." },
                    "monthlyPayment": { "type": "number", "description": "New monthly payment." },
                    "originatedAt": { "type": "string", "description": "ISO 8601 date YYYY-MM-DD." },
                    "liabilityType": {
                        "type": "string",
                        "description": "Updated subtype.",
                        "enum": ["mortgage", "student_loan", "credit_card", "personal_loan", "auto_loan", "heloc", "other"]
                    },
                    "notes": { "type": "string" }
                },
                "required": ["assetId"]
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

    fn tool() -> UpdateLiabilityTool<MockEnvironment> {
        UpdateLiabilityTool::new(Arc::new(MockEnvironment::new()))
    }

    #[tokio::test]
    async fn renames_and_increases_principal() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "LIAB-abc".into(),
                name: Some("Real mortgage".into()),
                principal: Some(620_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.validation.has_changes);
        assert_eq!(out.draft.name.as_deref(), Some("Real mortgage"));
        assert_eq!(out.draft.principal, Some(620_000.0));
    }

    #[tokio::test]
    async fn empty_name_clears_example_prefix() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "LIAB-abc".into(),
                name: Some("".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        // Empty string survives as Some("") so the frontend can detect "clear name" intent.
        assert_eq!(out.draft.name.as_deref(), Some(""));
        assert!(out.validation.has_changes);
    }

    #[tokio::test]
    async fn no_op_warns() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "LIAB-abc".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(!out.validation.has_changes);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("No fields changed")));
    }

    #[tokio::test]
    async fn invalid_date_warns_and_drops() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "LIAB-abc".into(),
                originated_at: Some("yesterday".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.draft.originated_at.is_none());
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("originatedAt")));
    }

    #[tokio::test]
    async fn unknown_subtype_is_dropped_with_warning() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "LIAB-abc".into(),
                liability_type: Some("bitcoin-loan".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.draft.liability_type.is_none());
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("Unknown liability type")));
    }

    #[tokio::test]
    async fn missing_asset_id_fails() {
        let out = tool()
            .build_output(UpdateLiabilityArgs {
                asset_id: "  ".into(),
                principal: Some(100_000.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.missing_fields.iter().any(|f| f == "assetId"));
    }
}
