//! Delete Liability tool — propose removal of an existing liability.
//!
//! Companion to `create_liability` / `update_liability`. Liabilities are
//! stored as `Asset` rows with `kind = LIABILITY`; this tool resolves a
//! reference + returns a draft preview the user confirms before the
//! cascade-delete fires. The tool never writes — the confirm card runs
//! the actual delete on click.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLiabilityArgs {
    /// Asset id (preferred — get_holdings returns it) or a name match.
    pub liability_ref: String,
    /// Optional reason carried onto the confirm card.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLiabilityOutput {
    pub target: LiabilitySnapshot,
    pub reason: Option<String>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilitySnapshot {
    pub id: String,
    pub name: String,
    pub liability_type: Option<String>,
    pub currency: String,
    pub principal: Option<f64>,
    pub display_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<LiabilitySnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
}

fn snapshot(h: &mizan_core::holdings::Holding) -> LiabilitySnapshot {
    let liability_type = h
        .metadata
        .as_ref()
        .and_then(|m| m.get("liability_type"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let principal = h
        .metadata
        .as_ref()
        .and_then(|m| m.get("principal"))
        .and_then(|v| v.as_f64());
    let name = h
        .instrument
        .as_ref()
        .and_then(|i| i.name.clone())
        .unwrap_or_else(|| "Liability".to_string());
    let display_code = h.instrument.as_ref().map(|i| i.symbol.clone()).filter(|s| !s.is_empty());
    let asset_id = h
        .instrument
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| h.id.clone());
    LiabilitySnapshot {
        id: asset_id,
        name,
        liability_type,
        currency: h.local_currency.clone(),
        principal,
        display_code,
    }
}

fn is_liability(h: &mizan_core::holdings::Holding) -> bool {
    matches!(h.asset_kind, Some(mizan_core::assets::AssetKind::Liability))
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
    let liabilities: Vec<&mizan_core::holdings::Holding> =
        holdings.iter().filter(|h| is_liability(h)).collect();

    if let Some(h) = liabilities.iter().find(|h| asset_id_of(h) == r) {
        return Resolution::Single(Box::new((*h).clone()));
    }
    let lower = r.to_lowercase();
    let exact: Vec<_> = liabilities
        .iter()
        .filter(|h| {
            name_of(h).to_lowercase() == lower || symbol_of(h).to_lowercase() == lower
        })
        .map(|h| (*h).clone())
        .collect();
    if exact.len() == 1 {
        return Resolution::Single(Box::new(exact.into_iter().next().unwrap()));
    }
    if exact.len() > 1 {
        return Resolution::Ambiguous(exact);
    }
    let substring: Vec<_> = liabilities
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

pub struct DeleteLiabilityTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteLiabilityTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: DeleteLiabilityArgs,
    ) -> Result<DeleteLiabilityOutput, AiError> {
        debug!(
            "delete_liability called: ref={:?}, reason={:?}",
            args.liability_ref, args.reason
        );

        let base_currency = self.env.base_currency();
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID, &base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolution = resolve(&holdings, &args.liability_ref);

        match resolution {
            Resolution::Single(a) => Ok(DeleteLiabilityOutput {
                target: snapshot(&a),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: true,
                    candidates: Vec::new(),
                    warnings: vec![
                        "Deleting a liability is irreversible. The amortization schedule and \
                         any payoff history attached to it go away too. If you've actually \
                         paid this off, consider marking it inactive instead so the history \
                         survives — ask before deleting if unsure."
                            .to_string(),
                    ],
                    is_valid: true,
                },
            }),
            Resolution::Ambiguous(candidates) => Ok(DeleteLiabilityOutput {
                target: LiabilitySnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: candidates.iter().map(snapshot).collect(),
                    warnings: vec![format!(
                        "\"{}\" matched {} liabilities. Ask the user which one they mean \
                         before retrying.",
                        args.liability_ref,
                        candidates.len()
                    )],
                    is_valid: false,
                },
            }),
            Resolution::NotFound => Ok(DeleteLiabilityOutput {
                target: LiabilitySnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![format!(
                        "No liability matching \"{}\" found. Use get_holdings to list \
                         liabilities first.",
                        args.liability_ref
                    )],
                    is_valid: false,
                },
            }),
            Resolution::Missing => Ok(DeleteLiabilityOutput {
                target: LiabilitySnapshot::default(),
                reason: args.reason,
                validation: ValidationResult {
                    resolved: false,
                    candidates: Vec::new(),
                    warnings: vec![
                        "liabilityRef is required — give the liability name, display code, or id."
                            .to_string(),
                    ],
                    is_valid: false,
                },
            }),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteLiabilityTool<E> {
    const NAME: &'static str = "delete_liability";

    type Error = AiError;
    type Args = DeleteLiabilityArgs;
    type Output = DeleteLiabilityOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Propose deletion of an existing liability (mortgage, loan, credit card, \
                 etc.). Use when the user paid one off, refinanced it into a new one, or \
                 created an example that should be removed. Returns a DRAFT preview the \
                 user confirms — does not write. If the user paid off but wants the \
                 history kept, prefer update_liability with principal=0 instead. \
                 Resolves the reference among LIABILITY-kind assets only."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "liabilityRef": {
                        "type": "string",
                        "description": "Asset id, name, or display_code of the liability to delete."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason from the user — surfaced on the \
                         confirm card. E.g. 'I paid off the mortgage', 'this was a duplicate'."
                    }
                },
                "required": ["liabilityRef"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.build_output(args).await
    }
}
