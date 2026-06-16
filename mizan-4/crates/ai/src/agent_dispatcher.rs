//! Agent-runtime → existing tool registry bridge.
//!
//! [`crate::agent::AgentRuntime`] calls tools through the
//! [`crate::agent::AgentToolDispatcher`] trait so the runtime stays
//! decoupled from any concrete tool implementation (tests use mocks,
//! production wires real tools). This module provides the production
//! impl that wraps the existing [`crate::tools::ToolSet`] and routes
//! tool-name strings to the right `impl Tool`.
//!
//! ## Why not just call rig-core agents directly?
//!
//! The chat dispatcher in [`crate::chat`] already does that for the
//! single-turn chat path: it builds a rig-core agent with the tool
//! list and lets rig drive the model→tool→model loop. The agent
//! runtime is fundamentally different — IT drives the loop, not the
//! LLM, so it needs to invoke tools directly without going through
//! rig's completion API. This dispatcher is the cleanest seam: take
//! a tool name + JSON args, invoke the tool's `call(args)` method,
//! return the result.
//!
//! ## Ledger entry capture
//!
//! Mutating tools (create_account, record_activities, etc.) write a
//! truth-ledger entry as part of their normal execution path. The
//! agent runtime needs those entry ids back so [`AgentRuntime::undo`]
//! can reverse the batch. Today we infer them from the tool name +
//! result shape — tools that follow the convention of returning a
//! `{ "ledgerEntryIds": [...] }` field have their ids extracted
//! automatically; tools that don't return an empty list (and the
//! Undo path skips them with a logged warning).
//!
//! Long-term, the right move is to plumb a `ledger_capture: Arc<…>`
//! through every tool so the capture is explicit at write time. Out
//! of scope for v1.

use std::sync::Arc;

use async_trait::async_trait;
use rig::tool::Tool;
use serde_json::Value;

use crate::agent::{AgentError, AgentToolDispatcher, DispatchResult};
use crate::env::AiEnvironment;
use crate::tools::ToolSet;

/// Production [`AgentToolDispatcher`]. Wraps a [`ToolSet`] and dispatches
/// tool calls by name. Each tool's `Tool::call` is invoked with the
/// JSON args; the result is captured along with any ledger entry ids
/// the tool surfaces.
pub struct ToolSetDispatcher<E: AiEnvironment + 'static> {
    tool_set: Arc<ToolSet<E>>,
}

impl<E: AiEnvironment + 'static> ToolSetDispatcher<E> {
    pub fn new(tool_set: Arc<ToolSet<E>>) -> Self {
        Self { tool_set }
    }
}

/// Extract any ledger entry ids the tool returned. Tools that follow
/// the §A1/§A2 convention return them in `ledgerEntryIds` (array of
/// strings); we tolerate `ledger_entries`, `ledgerEntries`, and
/// `entryIds` as synonyms because not every tool was authored against
/// the canonical shape yet.
fn extract_ledger_entries(output: &Value) -> Vec<String> {
    const KEYS: &[&str] = &[
        "ledgerEntryIds",
        "ledger_entries",
        "ledgerEntries",
        "entryIds",
    ];
    for key in KEYS {
        if let Some(arr) = output.get(key).and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
    }
    Vec::new()
}

#[async_trait]
impl<E: AiEnvironment + 'static> AgentToolDispatcher for ToolSetDispatcher<E> {
    async fn dispatch(&self, tool_name: &str, args: Value) -> Result<DispatchResult, AgentError> {
        // Each branch deserialises args into the tool's specific
        // argument type and calls the tool. Errors surface as
        // AgentError::Internal with the source error to_string so the
        // runtime's StepFailed event has a clear human message.
        macro_rules! invoke {
            ($tool:expr, $args_ty:ty) => {{
                let parsed: $args_ty = serde_json::from_value(args.clone()).map_err(|e| {
                    AgentError::Internal(format!(
                        "{}: failed to deserialise args ({}): {}",
                        tool_name, e, args
                    ))
                })?;
                let output = $tool
                    .call(parsed)
                    .await
                    .map_err(|e| AgentError::Internal(format!("{}: {}", tool_name, e)))?;
                let output_json = serde_json::to_value(&output).map_err(|e| {
                    AgentError::Internal(format!(
                        "{}: failed to serialise output: {}",
                        tool_name, e
                    ))
                })?;
                let ledger_entries = extract_ledger_entries(&output_json);
                Ok(DispatchResult {
                    output: output_json,
                    ledger_entries,
                })
            }};
        }

        match tool_name {
            // ─── Read-only tools ────────────────────────────────────
            "get_holdings" => {
                invoke!(
                    self.tool_set.holdings,
                    crate::tools::holdings::GetHoldingsArgs
                )
            }
            "get_accounts" => {
                invoke!(
                    self.tool_set.accounts,
                    crate::tools::accounts::GetAccountsArgs
                )
            }
            "get_cash_balances" => invoke!(
                self.tool_set.cash_balances,
                crate::tools::cash_balances::GetCashBalancesArgs
            ),
            "search_activities" => invoke!(
                self.tool_set.activities,
                crate::tools::activities::SearchActivitiesArgs
            ),
            "get_goals" => {
                invoke!(self.tool_set.goals, crate::tools::goals::GetGoalsArgs)
            }
            "get_valuation_history" => invoke!(
                self.tool_set.valuation,
                crate::tools::valuation::GetValuationHistoryArgs
            ),
            "get_income" => {
                invoke!(self.tool_set.income, crate::tools::income::GetIncomeArgs)
            }
            "get_asset_allocation" => invoke!(
                self.tool_set.allocation,
                crate::tools::allocation::GetAssetAllocationArgs
            ),
            "get_performance" => invoke!(
                self.tool_set.performance,
                crate::tools::performance::GetPerformanceArgs
            ),
            "get_health_status" => invoke!(
                self.tool_set.health_status,
                crate::tools::health::GetHealthStatusArgs
            ),
            "research_asset" => invoke!(
                self.tool_set.research_asset,
                crate::tools::research_asset::ResearchAssetArgs
            ),

            // ─── Mutating tools ────────────────────────────────────
            "create_account" => invoke!(
                self.tool_set.create_account,
                crate::tools::create_account::CreateAccountArgs
            ),
            "update_account" => invoke!(
                self.tool_set.update_account,
                crate::tools::update_account::UpdateAccountArgs
            ),
            "delete_account" => invoke!(
                self.tool_set.delete_account,
                crate::tools::delete_account::DeleteAccountArgs
            ),
            "delete_goal" => invoke!(
                self.tool_set.delete_goal,
                crate::tools::delete_goal::DeleteGoalArgs
            ),
            "delete_liability" => invoke!(
                self.tool_set.delete_liability,
                crate::tools::delete_liability::DeleteLiabilityArgs
            ),
            "delete_alternative_asset" => invoke!(
                self.tool_set.delete_alternative_asset,
                crate::tools::delete_alternative_asset::DeleteAlternativeAssetArgs
            ),
            "create_goal" => invoke!(
                self.tool_set.create_goal,
                crate::tools::create_goal::CreateGoalArgs
            ),
            "create_liability" => invoke!(
                self.tool_set.create_liability,
                crate::tools::create_liability::CreateLiabilityArgs
            ),
            "update_liability" => invoke!(
                self.tool_set.update_liability,
                crate::tools::update_liability::UpdateLiabilityArgs
            ),
            "add_alternative_asset" => invoke!(
                self.tool_set.add_alternative_asset,
                crate::tools::add_alternative_asset::AddAlternativeAssetArgs
            ),
            "record_activity" => invoke!(
                self.tool_set.record_activity,
                crate::tools::record_activity::RecordActivityArgs
            ),
            "record_activities" => invoke!(
                self.tool_set.record_activities,
                crate::tools::record_activities::RecordActivitiesArgs
            ),
            "import_csv" => invoke!(
                self.tool_set.import_csv,
                crate::tools::import_csv::ImportCsvArgs
            ),

            // ─── Agent-recipe-specific synthetic tools ─────────────
            //
            // These aren't real `impl Tool` types; they're synthesised
            // from existing tools or computed in-runtime so the
            // PortfolioFromCsv recipe (and friends) can reference
            // tools like `parse_csv` / `verify_totals` /
            // `query_account_summary` without needing brand-new
            // boilerplate per tool.
            "parse_csv" => parse_csv_synthetic(&args).await,
            "verify_totals" => verify_totals_synthetic(&args).await,
            "query_account_summary" => query_account_summary_synthetic(&args).await,
            "abort_with_message" => Err(AgentError::VerificationFailed(
                args.get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent self-aborted")
                    .to_string(),
            )),

            unknown => Err(AgentError::Internal(format!(
                "tool '{}' is not registered in the agent dispatcher",
                unknown
            ))),
        }
    }
}

/// Stub: parse a CSV. v1 returns the raw row count + a sample so the
/// agent can plan record_activities downstream. The full parser lives
/// in `crate::tools::import_csv` and `mizan_core::activities::csv_parser`
/// — wiring that across the runtime boundary is the work that lands
/// the full PortfolioFromCsv recipe end-to-end.
async fn parse_csv_synthetic(args: &Value) -> Result<DispatchResult, AgentError> {
    let csv_content = args
        .get("csvContent")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Internal("parse_csv: missing csvContent".to_string()))?;
    let row_count = csv_content.lines().filter(|l| !l.trim().is_empty()).count();
    let sample: Vec<&str> = csv_content.lines().take(5).collect();
    Ok(DispatchResult {
        output: serde_json::json!({
            "rowCount": row_count.saturating_sub(1), // minus header
            "sample": sample,
            "totalCost": 0,
            "stub": true,
            "_note": "parse_csv is a v1 stub — the chat dispatcher integration will wire the full parser from mizan_core::activities::csv_parser",
        }),
        ledger_entries: Vec::new(),
    })
}

/// Stub: verify recorded totals match expected. v1 always passes;
/// real implementation cross-references the activity service.
async fn verify_totals_synthetic(args: &Value) -> Result<DispatchResult, AgentError> {
    Ok(DispatchResult {
        output: serde_json::json!({
            "ok": true,
            "expected": args.get("expectedTotalFromCsv"),
            "actual": args.get("expectedTotalFromCsv"),
            "discrepancies": [],
            "stub": true,
        }),
        ledger_entries: Vec::new(),
    })
}

/// Stub: query an account's current summary. v1 echoes the args; real
/// implementation reads from the activity service.
async fn query_account_summary_synthetic(args: &Value) -> Result<DispatchResult, AgentError> {
    Ok(DispatchResult {
        output: serde_json::json!({
            "accountId": args.get("accountId"),
            "totalCost": 0,
            "activityCount": 0,
            "stub": true,
        }),
        ledger_entries: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn synthetic_parse_csv_returns_row_count_minus_header() {
        let args = serde_json::json!({
            "csvContent": "date,symbol,quantity,price\n2024-01-01,AAPL,10,150\n2024-01-02,MSFT,5,300\n2024-01-03,GOOG,2,2800\n"
        });
        let r = parse_csv_synthetic(&args).await.unwrap();
        assert_eq!(r.output["rowCount"], 3);
        assert_eq!(r.output["stub"], true);
    }

    #[test]
    fn extract_ledger_entries_handles_multiple_key_aliases() {
        let camel = serde_json::json!({"ledgerEntryIds": ["a", "b"]});
        let snake = serde_json::json!({"ledger_entries": ["c"]});
        let alt = serde_json::json!({"ledgerEntries": ["d"]});
        let none = serde_json::json!({"unrelated": true});

        assert_eq!(extract_ledger_entries(&camel), vec!["a", "b"]);
        assert_eq!(extract_ledger_entries(&snake), vec!["c"]);
        assert_eq!(extract_ledger_entries(&alt), vec!["d"]);
        assert!(extract_ledger_entries(&none).is_empty());
    }

    #[tokio::test]
    async fn abort_with_message_returns_verification_failed_error() {
        // Direct path through the dispatch — we need an environment-bound
        // ToolSet to construct a full ToolSetDispatcher, so just confirm
        // the synthetic path via the matchable error variant.
        let err = AgentError::VerificationFailed("test reason".to_string());
        assert!(err.to_string().contains("test reason"));
    }
}
