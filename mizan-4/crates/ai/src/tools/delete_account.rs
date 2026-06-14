//! Delete Account tool — propose deletion of an existing account.
//!
//! Companion to `create_account` / `update_account`. When the user says
//! "delete my Wise account" or "remove the test brokerage I created", the AI
//! calls this with a target reference (id or name fragment). Returns a
//! DRAFT preview with an impact summary (how many activities + holdings
//! cascade away) so the user knows what they're agreeing to before they
//! click Confirm in the tool-call card. The tool itself never writes — the
//! delete fires on user confirmation through the same path
//! `useAccountMutations` already uses for manual deletion.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

/// Args the LLM produces.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountArgs {
    /// Either the account's canonical id or a name match (exact or substring).
    pub account_ref: String,
    /// Optional free-form reason the user gave. Surfaced on the confirm
    /// card so the user can sanity-check the AI's intent before
    /// committing — "delete my old test account because it's empty".
    pub reason: Option<String>,
}

/// Tool output: a snapshot of what's about to be removed + the cascade
/// impact + validation. The UI renders this as a destructive confirm card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountOutput {
    /// Target account snapshot (empty when unresolved).
    pub target: AccountSnapshot,
    /// What will be removed if the user confirms.
    pub impact: DeletionImpact,
    /// Free-form reason carried through from the user.
    pub reason: Option<String>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshot {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub group: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionImpact {
    /// Activities that will cascade-delete with the account.
    pub activity_count: usize,
    /// Most-recent few activity descriptions so the user sees what's at stake.
    pub recent_activities: Vec<ActivityPreview>,
    /// Whether this is the user's default account (extra warning if true —
    /// they may not realise another account will become default by inheritance).
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPreview {
    pub activity_type: String,
    pub date: String,
    pub asset_symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    /// True when the reference resolved to a single account.
    pub resolved: bool,
    /// Candidate IDs when the reference matched multiple accounts.
    pub candidates: Vec<AccountSnapshot>,
    /// Why the deletion is unsafe / impossible. Surfaced on the card.
    pub warnings: Vec<String>,
    /// True when the user CAN confirm — false blocks the Confirm button.
    pub is_valid: bool,
}

/// Resolve "account_ref" to a concrete account using id, exact name, or
/// substring fallback. Identical logic to `update_account::resolve_account`
/// — kept inline so the two tools don't grow a shared helper module yet
/// (only two callers).
enum Resolution {
    Single(Box<mizan_core::accounts::Account>),
    Ambiguous(Vec<mizan_core::accounts::Account>),
    NotFound,
    Missing,
}

fn snapshot(a: &mizan_core::accounts::Account) -> AccountSnapshot {
    AccountSnapshot {
        id: a.id.clone(),
        name: a.name.clone(),
        account_type: a.account_type.clone(),
        currency: a.currency.clone(),
        group: a.group.clone(),
        is_default: a.is_default,
        is_active: a.is_active,
    }
}

fn resolve(accounts: &[mizan_core::accounts::Account], reference: &str) -> Resolution {
    let r = reference.trim();
    if r.is_empty() {
        return Resolution::Missing;
    }
    if let Some(a) = accounts.iter().find(|a| a.id == r) {
        return Resolution::Single(Box::new(a.clone()));
    }
    let lower = r.to_lowercase();
    let exact: Vec<_> = accounts
        .iter()
        .filter(|a| a.name.to_lowercase() == lower)
        .cloned()
        .collect();
    if exact.len() == 1 {
        return Resolution::Single(Box::new(exact.into_iter().next().unwrap()));
    }
    if exact.len() > 1 {
        return Resolution::Ambiguous(exact);
    }
    let substring: Vec<_> = accounts
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&lower))
        .cloned()
        .collect();
    match substring.len() {
        0 => Resolution::NotFound,
        1 => Resolution::Single(Box::new(substring.into_iter().next().unwrap())),
        _ => Resolution::Ambiguous(substring),
    }
}

pub struct DeleteAccountTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteAccountTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: DeleteAccountArgs,
    ) -> Result<DeleteAccountOutput, AiError> {
        debug!(
            "delete_account called: ref={:?}, reason={:?}",
            args.account_ref, args.reason
        );

        let accounts = self
            .env
            .account_service()
            .get_non_archived_accounts()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolution = resolve(&accounts, &args.account_ref);

        match resolution {
            Resolution::Single(boxed) => {
                let account = *boxed;
                // Count activities that will cascade away. We intentionally
                // swallow errors from the activity service here — the
                // delete tool's primary job is to identify the target and
                // let the user confirm; not knowing the activity count is
                // a UX degradation (the card just won't show the cascade
                // impact line) but not a reason to fail the whole call.
                let activities = self
                    .env
                    .activity_service()
                    .get_activities_by_account_id(&account.id)
                    .unwrap_or_default();

                let mut recent: Vec<ActivityPreview> = activities
                    .iter()
                    .rev()
                    .take(5)
                    .map(|a| ActivityPreview {
                        activity_type: a.activity_type.clone(),
                        date: a.activity_date.format("%Y-%m-%d").to_string(),
                        asset_symbol: a.asset_id.clone(),
                    })
                    .collect();
                recent.reverse();

                let mut warnings = Vec::new();
                if account.is_default {
                    warnings.push(
                        "This is your default account — another account will inherit the \
                         default flag after deletion.".to_string(),
                    );
                }
                if activities.len() > 20 {
                    warnings.push(format!(
                        "Deleting this account will cascade-remove {} activities. \
                         Consider archiving instead if you want to keep history.",
                        activities.len()
                    ));
                }

                Ok(DeleteAccountOutput {
                    target: snapshot(&account),
                    impact: DeletionImpact {
                        activity_count: activities.len(),
                        recent_activities: recent,
                        is_default: account.is_default,
                    },
                    reason: args.reason,
                    validation: ValidationResult {
                        resolved: true,
                        candidates: Vec::new(),
                        warnings,
                        is_valid: true,
                    },
                })
            }
            Resolution::Ambiguous(candidates) => Ok(DeleteAccountOutput {
                target: AccountSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} accounts. Ask the user which one they mean \
                         by name + currency before retrying.",
                        args.account_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                },
            }),
            Resolution::NotFound => Ok(DeleteAccountOutput {
                target: AccountSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No account named \"{}\" found. Double-check the name or list \
                         accounts first with get_accounts.",
                        args.account_ref
                    )],
                    is_valid: false,
                },
            }),
            Resolution::Missing => Ok(DeleteAccountOutput {
                target: AccountSnapshot::default(),
                impact: DeletionImpact::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![
                        "accountRef is required — give the account name or id.".to_string(),
                    ],
                    is_valid: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteAccountTool<E> {
    const NAME: &'static str = "delete_account";

    type Error = AiError;
    type Args = DeleteAccountArgs;
    type Output = DeleteAccountOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Propose deletion of an existing account. Use when the user says \
                 'delete', 'remove', 'get rid of', or 'kill' an account they no longer want \
                 tracked (e.g. a closed brokerage, a test account they made by mistake, an \
                 institution they no longer have a relationship with). \
                 Returns a DRAFT preview with an impact summary (activities that will \
                 cascade-delete) for the user to confirm — does not write directly. \
                 If you're not sure whether they want to delete vs archive, ask first; archive \
                 keeps history while delete is irreversible. When the user just wants to \
                 disable an account temporarily, use update_account with isActive=false instead."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountRef": {
                        "type": "string",
                        "description": "Account id or name match (exact or substring). E.g. \
                         'Vanguard taxable', 'wise', or the canonical id from get_accounts."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason the user gave — surfaced on the confirm \
                         card so the user can sanity-check the AI's intent. E.g. 'I closed this \
                         account last month'."
                    }
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
    use crate::env::test_env::{MockAccountService, MockEnvironment};
    use mizan_core::accounts::Account;

    fn make_account(id: &str, name: &str, is_default: bool) -> Account {
        Account {
            id: id.to_string(),
            name: name.to_string(),
            account_type: "SECURITIES".to_string(),
            currency: "USD".to_string(),
            is_default,
            is_active: true,
            ..Default::default()
        }
    }

    fn tool_with_accounts(accounts: Vec<Account>) -> DeleteAccountTool<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.account_service = Arc::new(MockAccountService { accounts });
        DeleteAccountTool::new(Arc::new(env))
    }

    #[tokio::test]
    async fn resolves_by_exact_name() {
        let tool = tool_with_accounts(vec![make_account("a1", "Vanguard taxable", false)]);
        let out = tool
            .build_output(DeleteAccountArgs {
                account_ref: "Vanguard taxable".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.validation.resolved);
        assert_eq!(out.target.id, "a1");
    }

    #[tokio::test]
    async fn ambiguous_match_blocks_confirm() {
        let tool = tool_with_accounts(vec![
            make_account("a1", "Vanguard taxable", false),
            make_account("a2", "Vanguard retirement", false),
        ]);
        let out = tool
            .build_output(DeleteAccountArgs {
                account_ref: "Vanguard".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(!out.validation.resolved);
        assert_eq!(out.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn missing_ref_blocks_confirm() {
        let tool = tool_with_accounts(vec![make_account("a1", "Vanguard", false)]);
        let out = tool
            .build_output(DeleteAccountArgs {
                account_ref: "  ".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.warnings.iter().any(|w| w.contains("required")));
    }

    #[tokio::test]
    async fn unknown_ref_returns_actionable_warning() {
        let tool = tool_with_accounts(vec![make_account("a1", "Vanguard", false)]);
        let out = tool
            .build_output(DeleteAccountArgs {
                account_ref: "nonexistent".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out.validation.warnings.iter().any(|w| w.contains("No account")));
    }

    #[tokio::test]
    async fn default_account_surfaces_inheritance_warning() {
        let tool = tool_with_accounts(vec![make_account("a1", "Main", true)]);
        let out = tool
            .build_output(DeleteAccountArgs {
                account_ref: "Main".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert!(out.impact.is_default);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("default account")));
    }
}
