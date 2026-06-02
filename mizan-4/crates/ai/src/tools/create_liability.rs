//! Create Liability tool — propose a new liability (mortgage, student loan,
//! credit card, …) from natural language.
//!
//! Wraps the AlternativeAsset model with `kind: Liability` and extra
//! liability-specific metadata (sub_type, rate_pct, monthly_payment,
//! originated_at, linked_asset_id). Produces a DRAFT preview the user
//! confirms — the tool never writes directly.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLiabilityArgs {
    /// One of: mortgage | student_loan | credit_card | personal_loan | auto_loan | heloc | other.
    pub liability_type: String,
    /// User-visible name. Defaults to a derived label (e.g. "Mortgage") when omitted.
    pub name: Option<String>,
    /// Outstanding balance / principal in the liability's currency.
    pub principal: f64,
    /// ISO-4217 currency code. Defaults to base.
    pub currency: Option<String>,
    /// Interest rate as a percentage (e.g. 5.2 for 5.2%).
    pub rate_pct: Option<f64>,
    /// Monthly payment in the same currency.
    pub monthly_payment: Option<f64>,
    /// ISO 8601 date when the liability was originated.
    pub originated_at: Option<String>,
    /// Optional alt-asset id to link this liability against (e.g. the property a mortgage finances).
    pub linked_asset_id: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLiabilityOutput {
    pub draft: LiabilityDraft,
    pub validation: ValidationResult,
    pub available_subtypes: Vec<LiabilitySubtypeOption>,
    pub available_currencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityDraft {
    /// Always "Liability" — carried for the UI's badge / icon.
    pub kind: String,
    pub name: String,
    pub liability_type: String,
    pub principal: f64,
    pub currency: String,
    pub rate_pct: Option<f64>,
    pub monthly_payment: Option<f64>,
    pub originated_at: Option<String>,
    pub linked_asset_id: Option<String>,
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

fn normalize_subtype(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "mortgage" | "home_loan" | "home-loan" => "mortgage".to_string(),
        "student_loan" | "student-loan" | "studentloan" | "tuition_loan" => {
            "student_loan".to_string()
        }
        "credit_card" | "credit-card" | "creditcard" | "cc" => "credit_card".to_string(),
        "personal_loan" | "personal-loan" | "personalloan" | "loan" => "personal_loan".to_string(),
        "auto_loan" | "auto-loan" | "car_loan" | "car-loan" | "autoloan" => "auto_loan".to_string(),
        "heloc" | "home_equity" | "home-equity" | "equity_line" => "heloc".to_string(),
        _ => "other".to_string(),
    }
}

fn default_name_for(subtype: &str) -> String {
    SUBTYPES
        .iter()
        .find(|(v, _)| *v == subtype)
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| "Liability".to_string())
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
    for code in [
        "USD", "EUR", "GBP", "CAD", "INR", "AED", "SGD", "AUD", "JPY",
    ] {
        push(&mut out, code);
    }
    out
}

fn is_iso_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").is_ok()
}

pub struct CreateLiabilityTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> CreateLiabilityTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: CreateLiabilityArgs,
    ) -> Result<CreateLiabilityOutput, AiError> {
        debug!(
            "create_liability called: type={:?}, principal={}, rate={:?}",
            args.liability_type, args.principal, args.rate_pct
        );

        let base_currency = self.env.base_currency();
        let liability_type = normalize_subtype(&args.liability_type);

        let name = args
            .name
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_name_for(&liability_type));

        let currency = args
            .currency
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_uppercase())
            .unwrap_or_else(|| base_currency.clone());

        let mut missing_fields: Vec<String> = Vec::new();
        if args.principal <= 0.0 {
            missing_fields.push("principal".to_string());
        }
        if args.liability_type.trim().is_empty() {
            missing_fields.push("liabilityType".to_string());
        }

        let mut warnings: Vec<String> = Vec::new();
        if let Some(rate) = args.rate_pct {
            if !(0.0..=100.0).contains(&rate) {
                warnings.push(format!(
                    "Rate {}% looks unusual — confirm before saving.",
                    rate
                ));
            }
        }
        if let Some(monthly) = args.monthly_payment {
            if monthly <= 0.0 {
                warnings.push("Monthly payment should be positive.".to_string());
            }
        }

        let originated_at = match args.originated_at.as_deref() {
            Some(s) if !s.trim().is_empty() && is_iso_date(s) => Some(s.trim().to_string()),
            Some(s) if !s.trim().is_empty() => {
                warnings.push(format!(
                    "Could not parse originated_at \"{}\" — expecting YYYY-MM-DD.",
                    s
                ));
                None
            }
            _ => None,
        };

        Ok(CreateLiabilityOutput {
            draft: LiabilityDraft {
                kind: "Liability".to_string(),
                name,
                liability_type,
                principal: args.principal.max(0.0),
                currency: currency.clone(),
                rate_pct: args.rate_pct,
                monthly_payment: args.monthly_payment,
                originated_at,
                linked_asset_id: args.linked_asset_id.filter(|s| !s.trim().is_empty()),
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
            available_subtypes: subtype_options(),
            available_currencies: available_currencies(&base_currency, Some(&currency)),
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for CreateLiabilityTool<E> {
    const NAME: &'static str = "create_liability";

    type Error = AiError;
    type Args = CreateLiabilityArgs;
    type Output = CreateLiabilityOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let base = self.env.base_currency();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Propose a new liability (mortgage, student loan, credit card, personal loan, \
                auto loan, HELOC). Returns a draft preview the user confirms — never writes. \
                Base currency is {base}; default to that unless the user names another. If the \
                user mentions a property the mortgage finances, pass linkedAssetId."
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "liabilityType": {
                        "type": "string",
                        "description": "One of mortgage | student_loan | credit_card | personal_loan | auto_loan | heloc | other.",
                        "enum": ["mortgage", "student_loan", "credit_card", "personal_loan", "auto_loan", "heloc", "other"]
                    },
                    "name": { "type": "string", "description": "User-visible name. Defaults to the type label." },
                    "principal": {
                        "type": "number",
                        "description": "Outstanding balance / principal. Required and must be positive."
                    },
                    "currency": {
                        "type": "string",
                        "description": format!("ISO-4217 code. Defaults to base {base} if omitted.")
                    },
                    "ratePct": {
                        "type": "number",
                        "description": "Interest rate as a percentage (5.2, not 0.052)."
                    },
                    "monthlyPayment": {
                        "type": "number",
                        "description": "Monthly payment amount."
                    },
                    "originatedAt": {
                        "type": "string",
                        "description": "ISO 8601 date YYYY-MM-DD when the liability was originated. Resolve relative phrases before calling."
                    },
                    "linkedAssetId": {
                        "type": "string",
                        "description": "Optional alt-asset id to link the liability against (e.g. PROP-…)."
                    },
                    "notes": { "type": "string" }
                },
                "required": ["liabilityType", "principal"]
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

    fn tool() -> CreateLiabilityTool<MockEnvironment> {
        CreateLiabilityTool::new(Arc::new(MockEnvironment::new()))
    }

    #[tokio::test]
    async fn produces_valid_mortgage_draft() {
        let out = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "mortgage".into(),
                principal: 480_000.0,
                rate_pct: Some(5.2),
                monthly_payment: Some(2650.0),
                originated_at: Some("2023-01-15".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.liability_type, "mortgage");
        assert_eq!(out.draft.principal, 480_000.0);
        assert_eq!(out.draft.currency, "USD");
        assert_eq!(out.draft.name, "Mortgage");
    }

    #[tokio::test]
    async fn normalises_aliases() {
        let auto = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "car-loan".into(),
                principal: 22_000.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(auto.draft.liability_type, "auto_loan");

        let cc = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "CC".into(),
                principal: 5_400.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(cc.draft.liability_type, "credit_card");
    }

    #[tokio::test]
    async fn flags_missing_principal() {
        let out = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "mortgage".into(),
                principal: 0.0,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .missing_fields
            .iter()
            .any(|f| f == "principal"));
    }

    #[tokio::test]
    async fn unusual_rate_warns() {
        let out = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "credit_card".into(),
                principal: 1_000.0,
                rate_pct: Some(250.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("unusual")));
    }

    #[tokio::test]
    async fn invalid_date_emits_warning() {
        let out = tool()
            .build_output(CreateLiabilityArgs {
                liability_type: "mortgage".into(),
                principal: 100_000.0,
                originated_at: Some("2 years ago".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.draft.originated_at.is_none());
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("originated_at")));
    }
}
