//! Update Account tool — edit an existing account from natural language.
//!
//! Companion to `create_account` for the edit-first UX (Feroz 25 May 2026:
//! "Edit-first UX over blank-form UX"). When the user says "rename my Vanguard
//! taxable to Vanguard brokerage" or "make Schwab my default", the AI calls
//! this with a target reference (id or name) + the fields it wants to change.
//! Returns a DRAFT preview the user confirms; the tool itself never writes.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

/// Args the LLM produces.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountArgs {
    /// Either the account's canonical id or a name match. Required.
    pub account_ref: String,
    /// New name (omit to keep current).
    pub name: Option<String>,
    /// New canonical type — SECURITIES, CASH, CRYPTOCURRENCY.
    pub account_type: Option<String>,
    /// Move this account in/out of "default" status.
    pub is_default: Option<bool>,
    /// Change active state.
    pub is_active: Option<bool>,
    /// Update the group label.
    pub group: Option<String>,
    /// Free-form notes that overwrite the meta field.
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountOutput {
    pub draft: AccountUpdateDraft,
    pub current: AccountSnapshot,
    pub diff: Vec<FieldDiff>,
    pub validation: ValidationResult,
    pub available_types: Vec<AccountTypeOption>,
}

/// What the draft will look like AFTER the change is applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdateDraft {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub group: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
    pub currency: String, // immutable; carried for display
    pub notes: Option<String>,
}

/// Current state for the UI to diff against.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshot {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub group: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
    pub currency: String,
    pub notes: Option<String>,
}

/// A single (field, old, new) row the UI renders as a struck-through change.
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
    pub is_valid: bool,
    pub missing_fields: Vec<String>,
    pub warnings: Vec<String>,
    /// True when the LLM's reference resolved to a real account.
    pub resolved: bool,
    /// Candidate IDs when the reference was ambiguous (multiple matches).
    pub candidates: Vec<AccountSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTypeOption {
    pub value: String,
    pub label: String,
}

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

fn normalize_account_type(raw: &str) -> Option<String> {
    let up = raw.trim().to_uppercase();
    match up.as_str() {
        "SECURITIES" | "BROKERAGE" | "TAXABLE" | "INVESTMENT" | "INVESTMENTS" | "RETIREMENT"
        | "401K" | "IRA" | "ROTH" | "ROTH_IRA" | "RRSP" | "TFSA" | "SIPP" | "PENSION" | "STOCK"
        | "STOCKS" | "EQUITIES" => Some("SECURITIES".to_string()),
        "CRYPTOCURRENCY" | "CRYPTO" | "WALLET" | "BTC" | "ETH" => {
            Some("CRYPTOCURRENCY".to_string())
        }
        "CASH" | "BANK" | "CHECKING" | "SAVINGS" | "DEPOSIT" | "CURRENT" => {
            Some("CASH".to_string())
        }
        _ => None,
    }
}

fn snapshot(a: &mizan_core::accounts::Account) -> AccountSnapshot {
    AccountSnapshot {
        id: a.id.clone(),
        name: a.name.clone(),
        account_type: a.account_type.clone(),
        group: a.group.clone(),
        is_default: a.is_default,
        is_active: a.is_active,
        currency: a.currency.clone(),
        notes: a.meta.clone(),
    }
}

fn resolve_account(
    accounts: &[mizan_core::accounts::Account],
    reference: &str,
) -> ResolutionResult {
    let r = reference.trim();
    if r.is_empty() {
        return ResolutionResult::Missing;
    }

    // 1. Exact id.
    if let Some(a) = accounts.iter().find(|a| a.id == r) {
        return ResolutionResult::Single(Box::new(a.clone()));
    }

    // 2. Exact name (case-insensitive).
    let lower = r.to_lowercase();
    let name_matches: Vec<_> = accounts
        .iter()
        .filter(|a| a.name.to_lowercase() == lower)
        .cloned()
        .collect();
    if name_matches.len() == 1 {
        return ResolutionResult::Single(Box::new(name_matches.into_iter().next().unwrap()));
    }
    if name_matches.len() > 1 {
        return ResolutionResult::Ambiguous(name_matches);
    }

    // 3. Substring (case-insensitive) — fallback for "vanguard" → "Vanguard taxable".
    let substring_matches: Vec<_> = accounts
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&lower))
        .cloned()
        .collect();
    match substring_matches.len() {
        0 => ResolutionResult::NotFound,
        1 => ResolutionResult::Single(Box::new(substring_matches.into_iter().next().unwrap())),
        _ => ResolutionResult::Ambiguous(substring_matches),
    }
}

// Single + Ambiguous carry a heavyweight Account (~500 bytes) while
// NotFound + Missing are zero-payload. Box'ing the Single variant
// equalises the enum size so it fits in the standard 64-byte enum
// budget without sacrificing pattern-match clarity at the call site.
enum ResolutionResult {
    Single(Box<mizan_core::accounts::Account>),
    Ambiguous(Vec<mizan_core::accounts::Account>),
    NotFound,
    Missing,
}

fn opt_str(s: &Option<String>) -> Option<String> {
    s.clone()
}

fn opt_bool_str(b: Option<bool>) -> Option<String> {
    b.map(|v| {
        if v {
            "yes".to_string()
        } else {
            "no".to_string()
        }
    })
}

fn diff_field<T: PartialEq + ToString>(field: &str, old: &T, new: &T, diffs: &mut Vec<FieldDiff>) {
    if old != new {
        diffs.push(FieldDiff {
            field: field.to_string(),
            old: Some(old.to_string()),
            new: Some(new.to_string()),
        });
    }
}

fn diff_opt(field: &str, old: &Option<String>, new: &Option<String>, diffs: &mut Vec<FieldDiff>) {
    if old != new {
        diffs.push(FieldDiff {
            field: field.to_string(),
            old: opt_str(old),
            new: opt_str(new),
        });
    }
}

pub struct UpdateAccountTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> UpdateAccountTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: UpdateAccountArgs,
    ) -> Result<UpdateAccountOutput, AiError> {
        debug!(
            "update_account called: ref={:?}, name={:?}, type={:?}",
            args.account_ref, args.name, args.account_type
        );

        let accounts = self
            .env
            .account_service()
            .get_non_archived_accounts()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolution = resolve_account(&accounts, &args.account_ref);
        let current = match &resolution {
            ResolutionResult::Single(a) => snapshot(a),
            ResolutionResult::Ambiguous(_)
            | ResolutionResult::NotFound
            | ResolutionResult::Missing => {
                // Return a stub draft + the candidates so the UI can prompt.
                let candidates = match resolution {
                    ResolutionResult::Ambiguous(list) => list.iter().map(snapshot).collect(),
                    _ => Vec::new(),
                };
                let mut missing = Vec::new();
                if args.account_ref.trim().is_empty() {
                    missing.push("accountRef".to_string());
                }
                return Ok(UpdateAccountOutput {
                    draft: AccountUpdateDraft {
                        id: String::new(),
                        name: args.name.unwrap_or_default(),
                        account_type: args
                            .account_type
                            .and_then(|t| normalize_account_type(&t))
                            .unwrap_or_default(),
                        group: args.group,
                        is_default: args.is_default.unwrap_or(false),
                        is_active: args.is_active.unwrap_or(true),
                        currency: String::new(),
                        notes: args.notes,
                    },
                    current: AccountSnapshot {
                        id: String::new(),
                        name: String::new(),
                        account_type: String::new(),
                        group: None,
                        is_default: false,
                        is_active: false,
                        currency: String::new(),
                        notes: None,
                    },
                    diff: Vec::new(),
                    validation: ValidationResult {
                        is_valid: false,
                        missing_fields: missing,
                        warnings: vec![if candidates.is_empty() {
                            format!(
                                "No active account matches \"{}\". Use create_account to add it, or list accounts first.",
                                args.account_ref
                            )
                        } else {
                            format!(
                                "Multiple accounts match \"{}\". Ask the user which one to update.",
                                args.account_ref
                            )
                        }],
                        resolved: false,
                        candidates,
                    },
                    available_types: account_type_options(),
                });
            }
        };

        // Build the proposed draft = current overlaid by any args the LLM set.
        let new_name = args
            .name
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| current.name.clone());

        let new_account_type = args
            .account_type
            .as_ref()
            .and_then(|t| normalize_account_type(t))
            .unwrap_or_else(|| current.account_type.clone());

        let new_group = match args.group.as_ref() {
            Some(g) if g.trim().is_empty() => None,
            Some(g) => Some(g.trim().to_string()),
            None => current.group.clone(),
        };

        let new_is_default = args.is_default.unwrap_or(current.is_default);
        let new_is_active = args.is_active.unwrap_or(current.is_active);

        let new_notes = match args.notes.as_ref() {
            Some(n) if n.trim().is_empty() => None,
            Some(n) => Some(n.trim().to_string()),
            None => current.notes.clone(),
        };

        // Compute per-field diffs.
        let mut diffs: Vec<FieldDiff> = Vec::new();
        diff_field("name", &current.name, &new_name, &mut diffs);
        diff_field(
            "accountType",
            &current.account_type,
            &new_account_type,
            &mut diffs,
        );
        diff_opt("group", &current.group, &new_group, &mut diffs);
        if current.is_default != new_is_default {
            diffs.push(FieldDiff {
                field: "isDefault".to_string(),
                old: opt_bool_str(Some(current.is_default)),
                new: opt_bool_str(Some(new_is_default)),
            });
        }
        if current.is_active != new_is_active {
            diffs.push(FieldDiff {
                field: "isActive".to_string(),
                old: opt_bool_str(Some(current.is_active)),
                new: opt_bool_str(Some(new_is_active)),
            });
        }
        diff_opt("notes", &current.notes, &new_notes, &mut diffs);

        let mut warnings = Vec::new();
        if diffs.is_empty() {
            warnings.push(
                "No fields changed — the draft matches the current account exactly.".to_string(),
            );
        }

        Ok(UpdateAccountOutput {
            draft: AccountUpdateDraft {
                id: current.id.clone(),
                name: new_name,
                account_type: new_account_type,
                group: new_group,
                is_default: new_is_default,
                is_active: new_is_active,
                currency: current.currency.clone(),
                notes: new_notes,
            },
            current,
            diff: diffs.clone(),
            validation: ValidationResult {
                is_valid: !diffs.is_empty(),
                missing_fields: Vec::new(),
                warnings,
                resolved: true,
                candidates: Vec::new(),
            },
            available_types: account_type_options(),
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for UpdateAccountTool<E> {
    const NAME: &'static str = "update_account";

    type Error = AiError;
    type Args = UpdateAccountArgs;
    type Output = UpdateAccountOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Edit an existing account: rename, change type, toggle default, set group, \
                or update notes. Returns a DRAFT preview the user confirms — never writes directly. \
                Pass any subset of optional fields the user wants to change; omitted fields keep \
                their current value. Use this (not create_account) when the user wants to modify \
                an account that already exists OR to replace an \"Example —\" pre-seeded liability \
                / account with their real one (edit-first UX per Feroz 25 May 2026)."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountRef": {
                        "type": "string",
                        "description": "The account's id, exact name, or a substring of its name. Required."
                    },
                    "name": { "type": "string", "description": "New name." },
                    "accountType": {
                        "type": "string",
                        "description": "New canonical type.",
                        "enum": ["SECURITIES", "CASH", "CRYPTOCURRENCY"]
                    },
                    "isDefault": { "type": "boolean", "description": "Set/unset default status." },
                    "isActive": { "type": "boolean", "description": "Activate/deactivate the account." },
                    "group": { "type": "string", "description": "New group label (empty string clears it)." },
                    "notes": { "type": "string", "description": "Free-form notes." }
                },
                "required": ["accountRef"]
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
    use chrono::NaiveDateTime;
    use mizan_core::accounts::Account;

    fn sample_account(id: &str, name: &str, account_type: &str) -> Account {
        Account {
            id: id.to_string(),
            name: name.to_string(),
            account_type: account_type.to_string(),
            group: None,
            currency: "USD".to_string(),
            is_default: false,
            is_active: true,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            platform_id: None,
            account_number: None,
            meta: None,
            provider: Some("MANUAL".to_string()),
            provider_account_id: None,
            is_archived: false,
            tracking_mode: Default::default(),
        }
    }

    fn env_with(accounts: Vec<Account>) -> Arc<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.set_accounts(accounts);
        Arc::new(env)
    }

    #[tokio::test]
    async fn renames_an_account() {
        let env = env_with(vec![sample_account(
            "acc-1",
            "Vanguard taxable",
            "SECURITIES",
        )]);
        let out = UpdateAccountTool::new(env)
            .build_output(UpdateAccountArgs {
                account_ref: "Vanguard taxable".into(),
                name: Some("Vanguard brokerage".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.draft.name, "Vanguard brokerage");
        assert_eq!(out.current.name, "Vanguard taxable");
        assert_eq!(out.diff.len(), 1);
        assert_eq!(out.diff[0].field, "name");
    }

    #[tokio::test]
    async fn resolves_substring_match() {
        let env = env_with(vec![sample_account(
            "acc-1",
            "Vanguard taxable",
            "SECURITIES",
        )]);
        let out = UpdateAccountTool::new(env)
            .build_output(UpdateAccountArgs {
                account_ref: "vanguard".into(),
                is_default: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.resolved);
        assert!(out.draft.is_default);
        assert!(!out.current.is_default);
    }

    #[tokio::test]
    async fn flags_ambiguous_reference() {
        let env = env_with(vec![
            sample_account("acc-1", "Schwab Roth", "SECURITIES"),
            sample_account("acc-2", "Schwab Traditional", "SECURITIES"),
        ]);
        let out = UpdateAccountTool::new(env)
            .build_output(UpdateAccountArgs {
                account_ref: "schwab".into(),
                name: Some("Schwab".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(!out.validation.resolved);
        assert_eq!(out.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn no_op_change_warns() {
        let env = env_with(vec![sample_account("acc-1", "Schwab", "SECURITIES")]);
        let out = UpdateAccountTool::new(env)
            .build_output(UpdateAccountArgs {
                account_ref: "Schwab".into(),
                name: Some("Schwab".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("No fields changed")));
    }
}
