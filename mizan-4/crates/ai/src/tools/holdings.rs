//! Holdings tool - fetch portfolio holdings using rig-core Tool trait.

use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::constants::MAX_HOLDINGS;
use crate::env::AiEnvironment;
use crate::error::AiError;

// ============================================================================
// Tool Arguments and Output
// ============================================================================

/// Arguments for the get_holdings tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHoldingsArgs {
    /// Account ID, or "TOTAL" for all accounts.
    #[serde(default = "default_account_id")]
    pub account_id: String,

    /// View mode: "table", "treemap", or "both". Default is "treemap".
    /// - "table": Show holdings as a detailed list with values and gains
    /// - "treemap": Show portfolio composition chart with daily performance colors
    /// - "both": Show treemap first, then table below
    #[serde(default = "default_view_mode")]
    pub view_mode: String,
}

fn default_account_id() -> String {
    "TOTAL".to_string()
}

fn default_view_mode() -> String {
    "treemap".to_string()
}

/// DTO for holding data in tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingDto {
    pub account: String,
    pub symbol: String,
    pub name: Option<String>,
    pub holding_type: String,
    pub quantity: f64,
    pub market_value_base: f64,
    pub cost_basis_base: Option<f64>,
    pub unrealized_gain_pct: Option<f64>,
    pub day_change_pct: Option<f64>,
    pub weight: f64,
    pub currency: String,
}

/// Output envelope for holdings tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHoldingsOutput {
    pub holdings: Vec<HoldingDto>,
    pub total_value: f64,
    pub currency: String,
    pub account_scope: String,
    /// View mode requested: "table", "treemap", or "both"
    pub view_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_count: Option<usize>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to get portfolio holdings.
pub struct GetHoldingsTool<E: AiEnvironment> {
    env: Arc<E>,
    base_currency: String,
}

impl<E: AiEnvironment> GetHoldingsTool<E> {
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self { env, base_currency }
    }
}

impl<E: AiEnvironment> Clone for GetHoldingsTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            base_currency: self.base_currency.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for GetHoldingsTool<E> {
    const NAME: &'static str = "get_holdings";

    type Error = AiError;
    type Args = GetHoldingsArgs;
    type Output = GetHoldingsOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get portfolio holdings for an account or all accounts. Returns symbol, quantity, market value, cost basis, and gain/loss for each holding. Use account_id='TOTAL' for aggregate holdings across all accounts. Use viewMode to control display: 'treemap' for visual composition chart with daily performance, 'table' for detailed list, or 'both' to show both views.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountId": {
                        "type": "string",
                        "description": "Account ID to get holdings for, or 'TOTAL' for all accounts",
                        "default": "TOTAL"
                    },
                    "viewMode": {
                        "type": "string",
                        "enum": ["table", "treemap", "both"],
                        "description": "Display mode: 'treemap' for composition chart with daily gains (best for 'how is my portfolio today?'), 'table' for detailed list, 'both' for treemap + table",
                        "default": "treemap"
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use std::collections::HashMap;

        let account_id = &args.account_id;

        // Fetch holdings
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(account_id, &self.base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        // Build account_id → name lookup
        let accounts = self
            .env
            .account_service()
            .list_accounts(None, None, None)
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
        let account_names: HashMap<String, String> =
            accounts.into_iter().map(|a| (a.id, a.name)).collect();

        let original_count = holdings.len();

        // Filter + truncate once, then both fold the Decimal totals and
        // build the f64 DTOs from the same source. Previously the total
        // was summed *after* each holding's Decimal value had been
        // converted to f64 — for a portfolio with many positions, that
        // sum drifts by a few ULP and the LLM-facing `total_value`
        // visibly diverges from the dashboard's own total. Same shape
        // as the PR #37 / PR #53 fixes.
        let filtered: Vec<mizan_core::holdings::Holding> = holdings
            .into_iter()
            .filter(|h| h.holding_type != mizan_core::holdings::HoldingType::Cash)
            .take(MAX_HOLDINGS)
            .collect();

        let total_value_decimal: rust_decimal::Decimal =
            filtered.iter().map(|h| h.market_value.base).sum();

        let holdings_dto: Vec<HoldingDto> = filtered
            .into_iter()
            .map(|h| {
                // Extract symbol and name from instrument
                let (symbol, name) = h
                    .instrument
                    .as_ref()
                    .map(|i| (i.symbol.clone(), i.name.clone()))
                    .unwrap_or_else(|| ("CASH".to_string(), None));

                // Convert HoldingType enum to string
                let holding_type = match h.holding_type {
                    mizan_core::holdings::HoldingType::Cash => "Cash",
                    mizan_core::holdings::HoldingType::Security => "Security",
                    mizan_core::holdings::HoldingType::AlternativeAsset => "AlternativeAsset",
                };

                let account = account_names
                    .get(&h.account_id)
                    .cloned()
                    .unwrap_or_else(|| h.account_id.clone());

                HoldingDto {
                    account,
                    symbol,
                    name,
                    holding_type: holding_type.to_string(),
                    quantity: h.quantity.to_f64().unwrap_or(0.0),
                    market_value_base: h.market_value.base.to_f64().unwrap_or(0.0),
                    cost_basis_base: h.cost_basis.as_ref().and_then(|c| c.base.to_f64()),
                    unrealized_gain_pct: h.unrealized_gain_pct.and_then(|d| d.to_f64()),
                    day_change_pct: h.day_change_pct.and_then(|d| d.to_f64()),
                    weight: h.weight.to_f64().unwrap_or(0.0),
                    currency: h.local_currency,
                }
            })
            .collect();

        let returned_count = holdings_dto.len();
        let total_value: f64 = total_value_decimal.to_f64().unwrap_or(0.0);
        let truncated = original_count > returned_count;

        Ok(GetHoldingsOutput {
            holdings: holdings_dto,
            total_value,
            currency: self.base_currency.clone(),
            account_scope: account_id.clone(),
            view_mode: args.view_mode.clone(),
            truncated: if truncated { Some(true) } else { None },
            original_count: if truncated {
                Some(original_count)
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::MockEnvironment;

    #[tokio::test]
    async fn test_get_holdings_tool() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetHoldingsTool::new(env, "USD".to_string());

        let result = tool
            .call(GetHoldingsArgs {
                account_id: "TOTAL".to_string(),
                view_mode: "treemap".to_string(),
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.account_scope, "TOTAL");
        assert_eq!(output.currency, "USD");
        assert_eq!(output.view_mode, "treemap");
    }

    #[tokio::test]
    async fn test_get_holdings_with_account_id() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetHoldingsTool::new(env, "USD".to_string());

        let result = tool
            .call(GetHoldingsArgs {
                account_id: "acc-123".to_string(),
                view_mode: "table".to_string(),
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.account_scope, "acc-123");
        assert_eq!(output.view_mode, "table");
    }

    /// Regression test for the holdings-total Decimal-sum-in-f64 bug.
    ///
    /// Pre-fix, the tool summed each holding's `market_value.base` after
    /// converting individually to f64. For MAX_HOLDINGS holdings each
    /// worth 0.10 USD (a value that's not exactly representable in
    /// binary floating point), the cumulative f64 sum drifts away from
    /// the integer-cent total — and the LLM-facing `total_value`
    /// visibly disagrees with the dashboard's own Decimal-summed total.
    ///
    /// After the fix, market values are summed as Decimal and converted
    /// to f64 once, so the answer is exactly 10.0 (its nearest f64).
    #[tokio::test]
    async fn aggregates_total_value_in_decimal_not_in_f64() {
        use crate::env::test_env::{MockEnvironment, MockHoldingsService};
        use mizan_core::holdings::{Holding, HoldingType};
        use mizan_core::portfolio::holdings::MonetaryValue;
        use rust_decimal::Decimal;

        // MAX_HOLDINGS (100) × 0.10 USD = 10.00 USD exactly. (The tool
        // applies `.take(MAX_HOLDINGS)` so any extras are truncated.)
        let cents = Decimal::new(10, 2); // 0.10
        let holdings: Vec<Holding> = (0..MAX_HOLDINGS)
            .map(|i| Holding {
                id: format!("h{i}"),
                account_id: "acc-1".into(),
                holding_type: HoldingType::Security,
                quantity: Decimal::ONE,
                local_currency: "USD".into(),
                base_currency: "USD".into(),
                cost_basis: None,
                instrument: None,
                asset_kind: None,
                open_date: None,
                lots: None,
                contract_multiplier: Decimal::ONE,
                weight: Decimal::ZERO,
                as_of_date: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                market_value: MonetaryValue {
                    local: cents,
                    base: cents,
                },
                price: None,
                purchase_price: None,
                fx_rate: None,
                unrealized_gain: None,
                unrealized_gain_pct: None,
                day_change: None,
                day_change_pct: None,
                prev_close_value: None,
                realized_gain: None,
                realized_gain_pct: None,
                dividend_income: None,
                total_gain: None,
                total_gain_pct: None,
                metadata: None,
            })
            .collect();

        let mut env = MockEnvironment::new();
        env.holdings_service = Arc::new(MockHoldingsService { holdings });
        let tool = GetHoldingsTool::new(Arc::new(env), "USD".to_string());

        let out = tool
            .call(GetHoldingsArgs {
                account_id: "TOTAL".to_string(),
                view_mode: "table".to_string(),
            })
            .await
            .expect("tool call");

        // Bit-exact: 10.0 is representable in f64, and Decimal::to_f64
        // returns the nearest f64. The old `sum-in-f64` path landed on
        // ~9.999999999999986 instead (every "+= 0.10" in f64 inherits
        // the same 1e-17 representation error and it accumulates).
        assert_eq!(out.total_value, 10.0_f64);
    }
}
