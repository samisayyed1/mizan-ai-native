//! Create Goal tool — propose a new savings / planning goal from natural language.
//!
//! Builds an editable DRAFT preview the user confirms via the assistant tool-UI.
//! The tool never writes — `goals_service.create_goal` runs on user confirmation
//! through the existing TanStack mutation infrastructure.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoalArgs {
    /// User-visible title (e.g. "House down payment", "Retirement", "Hajj fund").
    pub title: String,
    /// One of the canonical goal types — the tool normalises common synonyms.
    /// "retirement", "education", "wedding", "home", "emergency_fund", "custom_save_up".
    pub goal_type: String,
    /// Optional target amount in the user's base currency (or the override below).
    pub target_amount: Option<f64>,
    /// ISO-4217 currency code. Defaults to the user's base currency.
    pub currency: Option<String>,
    /// ISO 8601 date (YYYY-MM-DD) — leave empty for open-ended goals.
    pub target_date: Option<String>,
    /// Optional ISO 8601 start date. Defaults to today on confirm.
    pub start_date: Option<String>,
    /// Optional free-form description.
    pub description: Option<String>,
    /// Priority 1-3 (1 = highest). Defaults to 2.
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoalOutput {
    pub draft: GoalDraft,
    pub validation: ValidationResult,
    pub available_types: Vec<GoalTypeOption>,
    pub available_currencies: Vec<String>,
    /// Pre-existing goals so the UI can surface duplicates by title.
    pub existing_goals: Vec<ExistingGoal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalDraft {
    pub title: String,
    pub goal_type: String,
    pub target_amount: Option<f64>,
    pub currency: String,
    pub target_date: Option<String>,
    pub start_date: Option<String>,
    pub description: Option<String>,
    pub priority: i32,
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
pub struct ExistingGoal {
    pub id: String,
    pub title: String,
    pub goal_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalTypeOption {
    pub value: String,
    pub label: String,
}

/// Canonical goal types (mirrors apps/frontend/src/lib/schemas.ts::newGoalSchema).
const GOAL_TYPES: &[(&str, &str)] = &[
    ("retirement", "Retirement"),
    ("home", "Home / down-payment"),
    ("education", "Education"),
    ("wedding", "Wedding"),
    ("emergency_fund", "Emergency fund"),
    ("custom_save_up", "Custom save-up"),
];

fn goal_type_options() -> Vec<GoalTypeOption> {
    GOAL_TYPES
        .iter()
        .map(|(v, l)| GoalTypeOption {
            value: (*v).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

/// Maps the LLM's free-form goal type to one of the canonical values.
fn normalize_goal_type(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "retirement" | "retire" | "pension" | "fire" => "retirement".to_string(),
        "home" | "house" | "down_payment" | "down-payment" | "downpayment" | "property" => {
            "home".to_string()
        }
        "education" | "school" | "college" | "university" | "tuition" => "education".to_string(),
        "wedding" | "marriage" | "nikah" => "wedding".to_string(),
        "emergency_fund" | "emergency" | "rainy_day" | "rainy-day" | "safety_net" => {
            "emergency_fund".to_string()
        }
        // Anything else (hajj, car, vacation, "save for X", …) flows through the
        // generic save-up bucket.
        _ => "custom_save_up".to_string(),
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

pub struct CreateGoalTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> CreateGoalTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: CreateGoalArgs,
    ) -> Result<CreateGoalOutput, AiError> {
        debug!(
            "create_goal called: title={:?}, type={:?}, target_amount={:?}, target_date={:?}",
            args.title, args.goal_type, args.target_amount, args.target_date
        );

        let base_currency = self.env.base_currency();
        let title = args.title.trim().to_string();

        let goal_type = normalize_goal_type(&args.goal_type);
        let currency = args
            .currency
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_uppercase())
            .unwrap_or_else(|| base_currency.clone());

        let mut missing_fields: Vec<String> = Vec::new();
        if title.is_empty() {
            missing_fields.push("title".to_string());
        }
        if args.goal_type.trim().is_empty() {
            missing_fields.push("goalType".to_string());
        }

        let target_date = match args.target_date.as_deref() {
            Some(s) if !s.trim().is_empty() && is_iso_date(s) => Some(s.trim().to_string()),
            Some(s) if !s.trim().is_empty() => {
                // Bad date format — surface as a warning but don't drop it from missing_fields,
                // since the UI form will block submit until corrected.
                missing_fields.push("targetDate".to_string());
                let _ = s;
                None
            }
            _ => None,
        };

        let start_date = match args.start_date.as_deref() {
            Some(s) if !s.trim().is_empty() && is_iso_date(s) => Some(s.trim().to_string()),
            _ => None,
        };

        let priority = args.priority.unwrap_or(2).clamp(1, 3);

        // Fetch existing goals so the UI can warn on duplicate titles.
        let existing = self
            .env
            .goal_service()
            .get_goals()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut warnings: Vec<String> = Vec::new();
        let lower = title.to_lowercase();
        if !lower.is_empty() && existing.iter().any(|g| g.title.to_lowercase() == lower) {
            warnings.push(format!(
                "You already have a goal titled \"{}\". Consider editing it instead.",
                title
            ));
        }

        if goal_type == "retirement" && args.target_amount.is_none() {
            warnings.push(
                "Retirement goals work best with a target amount. Most users plan to ~25× annual expenses."
                    .to_string(),
            );
        }

        let existing_goals: Vec<ExistingGoal> = existing
            .iter()
            .map(|g| ExistingGoal {
                id: g.id.clone(),
                title: g.title.clone(),
                goal_type: g.goal_type.clone(),
            })
            .collect();

        Ok(CreateGoalOutput {
            draft: GoalDraft {
                title,
                goal_type,
                target_amount: args.target_amount,
                currency: currency.clone(),
                target_date,
                start_date,
                description: args
                    .description
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty()),
                priority,
            },
            validation: ValidationResult {
                is_valid: missing_fields.is_empty(),
                missing_fields,
                warnings,
            },
            available_types: goal_type_options(),
            available_currencies: available_currencies(&base_currency, Some(&currency)),
            existing_goals,
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for CreateGoalTool<E> {
    const NAME: &'static str = "create_goal";

    type Error = AiError;
    type Args = CreateGoalArgs;
    type Output = CreateGoalOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let base = self.env.base_currency();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Propose a savings / planning goal (retirement, house down-payment, education, \
                emergency fund, wedding, hajj fund, etc.). Returns a draft preview the user \
                confirms — never writes directly. Base currency is {base}; default to that \
                unless the user names another. Convert relative date phrases (\"in 5 years\", \
                \"by 2030\") to concrete YYYY-MM-DD using the current date from context."
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "User-visible title (e.g. 'House down payment', 'Hajj fund')."
                    },
                    "goalType": {
                        "type": "string",
                        "description": "One of: retirement, home, education, wedding, emergency_fund, custom_save_up. Anything not covered (hajj, car, vacation) goes under custom_save_up.",
                        "enum": ["retirement", "home", "education", "wedding", "emergency_fund", "custom_save_up"]
                    },
                    "targetAmount": {
                        "type": "number",
                        "description": "Target amount in the goal's currency. Omit for open-ended goals."
                    },
                    "currency": {
                        "type": "string",
                        "description": format!("ISO-4217 code. Defaults to base {base} if omitted.")
                    },
                    "targetDate": {
                        "type": "string",
                        "description": "Concrete ISO 8601 date YYYY-MM-DD. Resolve relative phrases before calling."
                    },
                    "startDate": {
                        "type": "string",
                        "description": "Optional ISO 8601 start date. Defaults to today on confirm."
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional free-form description."
                    },
                    "priority": {
                        "type": "integer",
                        "description": "1 (highest) to 3 (lowest). Defaults to 2.",
                        "minimum": 1,
                        "maximum": 3
                    }
                },
                "required": ["title", "goalType"]
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

    fn tool() -> CreateGoalTool<MockEnvironment> {
        CreateGoalTool::new(Arc::new(MockEnvironment::new()))
    }

    #[tokio::test]
    async fn produces_valid_basic_draft() {
        let out = tool()
            .build_output(CreateGoalArgs {
                title: "House down payment".into(),
                goal_type: "home".into(),
                target_amount: Some(80_000.0),
                target_date: Some("2030-01-01".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.goal_type, "home");
        assert_eq!(out.draft.priority, 2);
        assert_eq!(out.draft.currency, "USD");
    }

    #[tokio::test]
    async fn normalises_hajj_to_custom_save_up() {
        let out = tool()
            .build_output(CreateGoalArgs {
                title: "Hajj fund".into(),
                goal_type: "hajj".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.goal_type, "custom_save_up");
    }

    #[tokio::test]
    async fn retirement_without_target_amount_warns() {
        let out = tool()
            .build_output(CreateGoalArgs {
                title: "Retirement".into(),
                goal_type: "retirement".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("target amount")));
    }

    #[tokio::test]
    async fn rejects_bad_target_date() {
        let out = tool()
            .build_output(CreateGoalArgs {
                title: "Wedding".into(),
                goal_type: "wedding".into(),
                target_date: Some("next-year".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .missing_fields
            .iter()
            .any(|f| f == "targetDate"));
    }

    #[tokio::test]
    async fn priority_clamps_to_valid_range() {
        let out = tool()
            .build_output(CreateGoalArgs {
                title: "Emergency".into(),
                goal_type: "emergency_fund".into(),
                priority: Some(99),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.priority, 3);
    }
}
