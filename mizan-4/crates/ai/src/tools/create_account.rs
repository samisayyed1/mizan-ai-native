//! Create Account tool — propose a new account from natural language.
//!
//! Produces a draft preview the user confirms via the assistant tool-UI. The
//! tool itself never writes — the actual persistence runs through the existing
//! `accounts_service.create_account` mutation when the user clicks Confirm,
//! mirroring the `record_activity` pattern.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

/// Args the LLM produces.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountArgs {
    /// User-visible name for the account (e.g. "Vanguard taxable", "HDFC India").
    pub name: String,
    /// One of: BROKERAGE, CASH, RETIREMENT, CRYPTO, SAVINGS, CHECKING, OTHER.
    pub account_type: String,
    /// ISO-4217 currency code (e.g. "USD", "INR"). Falls back to the user's base currency.
    pub currency: Option<String>,
    /// Mark this account as the default for new activities.
    #[serde(default)]
    pub is_default: bool,
    /// Optional grouping label ("Retirement", "Spouse", etc.).
    pub group: Option<String>,
    /// Optional free-form notes the user mentioned.
    pub notes: Option<String>,
}

/// Tool output: a draft preview plus context for the confirm card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountOutput {
    pub draft: AccountDraft,
    pub validation: ValidationResult,
    /// Existing accounts so the UI can warn on near-duplicates.
    pub existing_accounts: Vec<ExistingAccount>,
    /// Suggested currency choices, base currency first.
    pub available_currencies: Vec<String>,
    /// Canonical account-type options for the dropdown.
    pub available_types: Vec<AccountTypeOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDraft {
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub is_default: bool,
    pub group: Option<String>,
    pub notes: Option<String>,
    /// Always "MANUAL" for AI-created accounts.
    pub provider: String,
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
pub struct ExistingAccount {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTypeOption {
    pub value: String,
    pub label: String,
}

/// Canonical account types (mirrors `apps/frontend/src/lib/constants.ts::AccountType`).
/// Sub-buckets like "Retirement", "Checking", "Savings" live in the `group` field.
const ACCOUNT_TYPES: &[(&str, &str)] = &[
    ("SECURITIES", "Brokerage / investments"),
    ("CASH", "Cash / bank account"),
    ("CRYPTOCURRENCY", "Crypto wallet / exchange"),
];

fn account_type_options() -> Vec<AccountTypeOption> {
    ACCOUNT_TYPES
        .iter()
        .map(|(v, l)| AccountTypeOption {
            value: (*v).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

/// Maps the LLM's free-form account type to one of the canonical values.
/// Common synonyms (brokerage/ira/401k/bank/checking/savings/crypto/btc/eth) all
/// resolve correctly so the user's natural phrasing works on the first try.
fn normalize_account_type(raw: &str) -> String {
    let up = raw.trim().to_uppercase();
    match up.as_str() {
        "SECURITIES" | "BROKERAGE" | "TAXABLE" | "INVESTMENT" | "INVESTMENTS" | "RETIREMENT"
        | "401K" | "IRA" | "ROTH" | "ROTH_IRA" | "RRSP" | "TFSA" | "SIPP" | "PENSION" | "STOCK"
        | "STOCKS" | "EQUITIES" => "SECURITIES".to_string(),

        "CRYPTOCURRENCY" | "CRYPTO" | "WALLET" | "BTC" | "ETH" => "CRYPTOCURRENCY".to_string(),

        "CASH" | "BANK" | "CHECKING" | "SAVINGS" | "DEPOSIT" | "CURRENT" => "CASH".to_string(),

        _ => "CASH".to_string(),
    }
}

/// When the LLM mentioned a sub-bucket (retirement / checking / savings), we
/// capture that in `group` so the user can see "SECURITIES → Retirement" in
/// the UI without needing a separate accountType field.
fn infer_group_hint(raw: &str) -> Option<String> {
    let up = raw.trim().to_uppercase();
    match up.as_str() {
        "RETIREMENT" | "401K" | "IRA" | "ROTH" | "ROTH_IRA" | "RRSP" | "TFSA" | "SIPP"
        | "PENSION" => Some("Retirement".to_string()),
        "CHECKING" | "CURRENT" => Some("Checking".to_string()),
        "SAVINGS" | "DEPOSIT" => Some("Savings".to_string()),
        _ => None,
    }
}

/// Compute the currency list: base currency first, then common others.
fn available_currencies(base: &str, hint: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push_unique = |out: &mut Vec<String>, code: &str| {
        let code = code.trim().to_uppercase();
        if !code.is_empty() && !out.contains(&code) {
            out.push(code);
        }
    };
    push_unique(&mut out, base);
    if let Some(h) = hint {
        push_unique(&mut out, h);
    }
    for code in [
        "USD", "EUR", "GBP", "CAD", "INR", "AED", "SGD", "AUD", "JPY",
    ] {
        push_unique(&mut out, code);
    }
    out
}

pub struct CreateAccountTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> CreateAccountTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: CreateAccountArgs,
    ) -> Result<CreateAccountOutput, AiError> {
        debug!(
            "create_account called: name={:?}, type={:?}, currency={:?}",
            args.name, args.account_type, args.currency
        );

        let base_currency = self.env.base_currency();
        let currency = args
            .currency
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_uppercase())
            .unwrap_or_else(|| base_currency.clone());

        let account_type = normalize_account_type(&args.account_type);
        let name = args.name.trim().to_string();
        // If user said "retirement" but didn't separately set a group, capture it.
        let group_hint = args
            .group
            .clone()
            .or_else(|| infer_group_hint(&args.account_type));

        let mut missing_fields: Vec<String> = Vec::new();
        if name.is_empty() {
            missing_fields.push("name".to_string());
        }
        if args.account_type.trim().is_empty() {
            missing_fields.push("accountType".to_string());
        }

        // Surface existing accounts so the UI can warn on near-duplicates and
        // the AI's confirm card never blindly creates a "Schwab" when one
        // already exists.
        let existing = self
            .env
            .account_service()
            .get_active_non_archived_accounts()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut warnings: Vec<String> = Vec::new();
        let name_lower = name.to_lowercase();
        if !name_lower.is_empty() && existing.iter().any(|a| a.name.to_lowercase() == name_lower) {
            warnings.push(format!(
                "An account named \"{}\" already exists — consider update_account instead.",
                name
            ));
        }

        let existing_accounts = existing
            .iter()
            .map(|a| ExistingAccount {
                id: a.id.clone(),
                name: a.name.clone(),
                account_type: a.account_type.clone(),
                currency: a.currency.clone(),
            })
            .collect();

        let validation = ValidationResult {
            is_valid: missing_fields.is_empty(),
            missing_fields,
            warnings,
        };

        Ok(CreateAccountOutput {
            draft: AccountDraft {
                name,
                account_type,
                currency: currency.clone(),
                is_default: args.is_default,
                group: group_hint
                    .map(|g| g.trim().to_string())
                    .filter(|g| !g.is_empty()),
                notes: args
                    .notes
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty()),
                provider: "MANUAL".to_string(),
            },
            validation,
            existing_accounts,
            available_currencies: available_currencies(&base_currency, Some(&currency)),
            available_types: account_type_options(),
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for CreateAccountTool<E> {
    const NAME: &'static str = "create_account";

    type Error = AiError;
    type Args = CreateAccountArgs;
    type Output = CreateAccountOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let base = self.env.base_currency();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Propose a new account (brokerage, cash, retirement, crypto, bank). Use this \
                whenever the user mentions an account they want tracked — including non-US \
                institutions Plaid can't reach (Indian, UK, UAE, Saudi banks etc.) where \
                manual tracking is the right path. Returns a DRAFT preview for the user to \
                confirm — does not write directly. The user's base currency is {base}; default \
                the account currency to that unless the user names another."
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "User-visible name (e.g. 'Vanguard taxable', 'HDFC India savings')."
                    },
                    "accountType": {
                        "type": "string",
                        "description": "Canonical type. SECURITIES for any brokerage / IRA / 401k / RRSP / pension / investment account. CASH for any bank / checking / savings / current. CRYPTOCURRENCY for any wallet / exchange. If the user says 'retirement', use SECURITIES (and the tool will set group='Retirement' automatically).",
                        "enum": ["SECURITIES", "CASH", "CRYPTOCURRENCY"]
                    },
                    "currency": {
                        "type": "string",
                        "description": format!("ISO-4217 currency code. Defaults to base currency {base} if omitted.")
                    },
                    "isDefault": {
                        "type": "boolean",
                        "description": "Mark this as the default account for new activities."
                    },
                    "group": {
                        "type": "string",
                        "description": "Optional grouping label (e.g. 'Retirement', 'Spouse')."
                    },
                    "notes": {
                        "type": "string",
                        "description": "Optional free-form notes from the user."
                    }
                },
                "required": ["name", "accountType"]
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

    fn tool() -> CreateAccountTool<MockEnvironment> {
        CreateAccountTool::new(Arc::new(MockEnvironment::new()))
    }

    #[tokio::test]
    async fn defaults_currency_to_base_and_normalises_brokerage() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "Vanguard".into(),
                account_type: "brokerage".into(),
                currency: None,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.currency, "USD");
        assert_eq!(out.draft.account_type, "SECURITIES");
        assert_eq!(out.draft.provider, "MANUAL");
        assert!(out.validation.is_valid);
    }

    #[tokio::test]
    async fn retirement_resolves_to_securities_with_group_hint() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "Vanguard IRA".into(),
                account_type: "retirement".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.account_type, "SECURITIES");
        assert_eq!(out.draft.group.as_deref(), Some("Retirement"));
    }

    #[tokio::test]
    async fn checking_resolves_to_cash_with_group_hint() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "Chase Checking".into(),
                account_type: "checking".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.account_type, "CASH");
        assert_eq!(out.draft.group.as_deref(), Some("Checking"));
    }

    #[tokio::test]
    async fn unknown_type_falls_back_to_cash() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "Acme".into(),
                account_type: "weird-thing".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.account_type, "CASH");
    }

    #[tokio::test]
    async fn flags_missing_name() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "  ".into(),
                account_type: "CASH".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.missing_fields.iter().any(|f| f == "name"));
    }

    #[tokio::test]
    async fn surfaces_currency_choices_base_first() {
        let out = tool()
            .build_output(CreateAccountArgs {
                name: "HDFC India".into(),
                account_type: "SAVINGS".into(),
                currency: Some("inr".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out.draft.currency, "INR");
        assert_eq!(out.available_currencies.first(), Some(&"USD".to_string()));
        assert!(out.available_currencies.contains(&"INR".to_string()));
    }
}
