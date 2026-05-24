//! Valuation history tool - fetch portfolio valuation history using rig-core Tool trait.

use chrono::NaiveDate;
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::constants::{DEFAULT_VALUATIONS_DAYS, MAX_VALUATIONS_POINTS};
use crate::env::AiEnvironment;
use crate::error::AiError;

// ============================================================================
// Tool Arguments and Output
// ============================================================================

/// Arguments for the get_valuation_history tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetValuationHistoryArgs {
    /// Account ID, or "TOTAL" for all accounts aggregated.
    #[serde(default = "default_account_id")]
    pub account_id: String,
    /// Start date for the valuation history (YYYY-MM-DD format).
    #[serde(default)]
    pub start_date: Option<String>,
    /// End date for the valuation history (YYYY-MM-DD format).
    #[serde(default)]
    pub end_date: Option<String>,
}

fn default_account_id() -> String {
    "TOTAL".to_string()
}

/// DTO for a single valuation point in tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationPointDto {
    pub date: String,
    pub total_value: f64,
    pub net_contribution: f64,
    pub currency: String,
}

/// Output envelope for valuation history tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetValuationHistoryOutput {
    pub valuations: Vec<ValuationPointDto>,
    pub account_scope: String,
    pub currency: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_count: Option<usize>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to get portfolio valuation history.
pub struct GetValuationHistoryTool<E: AiEnvironment> {
    env: Arc<E>,
    base_currency: String,
}

impl<E: AiEnvironment> GetValuationHistoryTool<E> {
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self { env, base_currency }
    }
}

impl<E: AiEnvironment> Clone for GetValuationHistoryTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            base_currency: self.base_currency.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for GetValuationHistoryTool<E> {
    const NAME: &'static str = "get_valuation_history";

    type Error = AiError;
    type Args = GetValuationHistoryArgs;
    type Output = GetValuationHistoryOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get historical portfolio valuations over time. Returns daily valuation points with total value and net contributions. Use account_id='TOTAL' for aggregate valuations across all accounts. Useful for analyzing portfolio growth, performance trends, and comparing value vs contributions.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountId": {
                        "type": "string",
                        "description": "Account ID to get valuations for, or 'TOTAL' for all accounts aggregated",
                        "default": "TOTAL"
                    },
                    "startDate": {
                        "type": "string",
                        "description": "Start date in YYYY-MM-DD format. Defaults to 365 days ago."
                    },
                    "endDate": {
                        "type": "string",
                        "description": "End date in YYYY-MM-DD format. Defaults to today."
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let account_id = &args.account_id;

        // Parse dates with defaults
        let end_date = args
            .end_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let start_date = args
            .start_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| end_date - chrono::Duration::days(DEFAULT_VALUATIONS_DAYS));

        // Fetch valuations based on account scope
        let valuations = if account_id == "TOTAL" {
            // Get all active accounts and aggregate their valuations
            let accounts = self
                .env
                .account_service()
                .get_active_accounts()
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

            // Aggregate in Decimal so multi-account totals don't accumulate
            // float rounding error. The conversion to f64 happens once, at
            // the JSON DTO boundary below.
            let mut aggregated: HashMap<NaiveDate, (Decimal, Decimal)> = HashMap::new();

            for account in accounts {
                let account_valuations = self
                    .env
                    .valuation_service()
                    .get_historical_valuations(&account.id, Some(start_date), Some(end_date))
                    .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

                for v in account_valuations {
                    // Convert account-currency amounts to base currency in
                    // Decimal — lossless multiply, no float drift.
                    let total_in_base = v.total_value * v.fx_rate_to_base;
                    let contribution_in_base = v.net_contribution * v.fx_rate_to_base;
                    let entry = aggregated
                        .entry(v.valuation_date)
                        .or_insert((Decimal::ZERO, Decimal::ZERO));
                    entry.0 += total_in_base;
                    entry.1 += contribution_in_base;
                }
            }

            // Convert aggregated data to sorted vector. f64 conversion
            // happens here, once per date, never compounded.
            let mut result: Vec<ValuationPointDto> = aggregated
                .into_iter()
                .map(
                    |(date, (total_value, net_contribution))| ValuationPointDto {
                        date: date.format("%Y-%m-%d").to_string(),
                        total_value: total_value.to_f64().unwrap_or(0.0),
                        net_contribution: net_contribution.to_f64().unwrap_or(0.0),
                        currency: self.base_currency.clone(),
                    },
                )
                .collect();
            result.sort_by_key(|a| a.date.clone());
            result
        } else {
            // Single account valuations
            let account_valuations = self
                .env
                .valuation_service()
                .get_historical_valuations(account_id, Some(start_date), Some(end_date))
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

            account_valuations
                .into_iter()
                .map(|v| {
                    // Multiply in Decimal first, convert to f64 only at the
                    // JSON boundary so per-row precision is preserved.
                    let total_in_base = v.total_value * v.fx_rate_to_base;
                    let contribution_in_base = v.net_contribution * v.fx_rate_to_base;
                    ValuationPointDto {
                        date: v.valuation_date.format("%Y-%m-%d").to_string(),
                        total_value: total_in_base.to_f64().unwrap_or(0.0),
                        net_contribution: contribution_in_base.to_f64().unwrap_or(0.0),
                        currency: self.base_currency.clone(),
                    }
                })
                .collect()
        };

        let original_count = valuations.len();

        // Apply limit
        let valuations: Vec<ValuationPointDto> =
            valuations.into_iter().take(MAX_VALUATIONS_POINTS).collect();

        let returned_count = valuations.len();
        let truncated = original_count > returned_count;

        Ok(GetValuationHistoryOutput {
            valuations,
            account_scope: account_id.clone(),
            currency: self.base_currency.clone(),
            start_date: start_date.format("%Y-%m-%d").to_string(),
            end_date: end_date.format("%Y-%m-%d").to_string(),
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
    async fn test_get_valuation_history_tool() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetValuationHistoryTool::new(env, "USD".to_string());

        let result = tool
            .call(GetValuationHistoryArgs {
                account_id: "TOTAL".to_string(),
                start_date: None,
                end_date: None,
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.account_scope, "TOTAL");
        assert_eq!(output.currency, "USD");
    }

    #[tokio::test]
    async fn test_get_valuation_history_with_account_id() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetValuationHistoryTool::new(env, "USD".to_string());

        let result = tool
            .call(GetValuationHistoryArgs {
                account_id: "acc-123".to_string(),
                start_date: Some("2024-01-01".to_string()),
                end_date: Some("2024-12-31".to_string()),
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.account_scope, "acc-123");
        assert_eq!(output.start_date, "2024-01-01");
        assert_eq!(output.end_date, "2024-12-31");
    }

    #[tokio::test]
    async fn test_get_valuation_history_with_dates() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetValuationHistoryTool::new(env, "EUR".to_string());

        let result = tool
            .call(GetValuationHistoryArgs {
                account_id: "TOTAL".to_string(),
                start_date: Some("2024-06-01".to_string()),
                end_date: Some("2024-06-30".to_string()),
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.currency, "EUR");
        assert_eq!(output.start_date, "2024-06-01");
        assert_eq!(output.end_date, "2024-06-30");
    }

    /// Regression test for the f64-aggregation precision bug.
    ///
    /// Before the fix, TOTAL aggregation across accounts did:
    ///   total += account_value.to_f64() * fx_rate.to_f64()
    /// which compounded float-rounding error across N accounts. With
    /// even a modestly-decimal FX rate (1.07) and 100 accounts each
    /// holding 100.10 EUR, the sum in f64 drifts off the true integer-cent
    /// answer in the last bit. After the fix, multiplication and
    /// accumulation stay in Decimal — exact — and to_f64 happens once,
    /// at the JSON boundary.
    #[tokio::test]
    async fn aggregates_decimal_then_converts_to_f64_once() {
        use crate::env::test_env::{MockAccountService, MockEnvironment, MockValuationService};
        use mizan_core::accounts::Account;
        use mizan_core::portfolio::valuation::DailyAccountValuation;

        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        // Same valuation returned for every account by MockValuationService
        // (it ignores the account_id arg). 100 accounts × 100.10 EUR × 1.07
        // USD/EUR = 10,710.70 USD exactly, and 100 × 80 × 1.07 = 8,560.00.
        let fixture = DailyAccountValuation {
            id: "v1".into(),
            account_id: "ignored".into(),
            valuation_date: date,
            account_currency: "EUR".into(),
            base_currency: "USD".into(),
            fx_rate_to_base: Decimal::new(107, 2), // 1.07
            cash_balance: Decimal::ZERO,
            investment_market_value: Decimal::new(10010, 2), // 100.10
            total_value: Decimal::new(10010, 2),             // 100.10
            cost_basis: Decimal::new(50, 0),                 // 50
            net_contribution: Decimal::new(80, 0),           // 80
            calculated_at: chrono::Utc::now(),
        };

        let accounts: Vec<Account> = (0..100)
            .map(|i| Account {
                id: format!("a{i}"),
                is_active: true,
                currency: "EUR".into(),
                ..Default::default()
            })
            .collect();

        let mut env = MockEnvironment::new();
        env.account_service = Arc::new(MockAccountService { accounts });
        env.valuation_service = Arc::new(MockValuationService {
            valuations: vec![fixture],
        });

        let tool = GetValuationHistoryTool::new(Arc::new(env), "USD".to_string());
        let out = tool
            .call(GetValuationHistoryArgs {
                account_id: "TOTAL".to_string(),
                start_date: Some("2025-01-01".to_string()),
                end_date: Some("2025-01-01".to_string()),
            })
            .await
            .expect("tool call");

        assert_eq!(out.valuations.len(), 1);
        // Bit-exact equality: 10710.70 has a unique nearest f64, and
        // Decimal::to_f64 rounds to that same f64. Any float drift in the
        // aggregation would land us on a different bit pattern.
        assert_eq!(out.valuations[0].total_value, 10710.70_f64);
        assert_eq!(out.valuations[0].net_contribution, 8560.00_f64);
    }
}
