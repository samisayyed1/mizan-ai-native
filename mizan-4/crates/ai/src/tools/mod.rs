//! AI assistant tools for portfolio data access.
//!
//! This module provides tools that implement rig-core's Tool trait:
//! - GetAccountsTool: Fetch active investment accounts
//! - GetHoldingsTool: Fetch portfolio holdings
//! - GetAssetAllocationTool: Calculate portfolio allocation by category
//! - GetPerformanceTool: Fetch portfolio performance metrics
//! - GetValuationHistoryTool: Fetch portfolio valuation history
//! - SearchActivitiesTool: Search transactions
//! - GetIncomeTool: Fetch income summaries (dividends, interest, other income)
//! - GetGoalsTool: Fetch investment goals with progress
//! - RecordActivityTool: Create activity drafts from natural language
//! - RecordActivitiesTool: Create multiple activity drafts from natural language
//!
//! All tools are designed to work with the AiEnvironment trait for dependency injection.

pub mod accounts;
pub mod activities;
pub mod add_alternative_asset;
pub mod allocation;
pub mod cash_balances;
pub mod computation_safety;
pub mod constants;
pub mod create_account;
pub mod create_goal;
pub mod create_liability;
pub mod csv_intel;
pub mod delete_account;
pub mod delete_alternative_asset;
pub mod delete_goal;
pub mod delete_liability;
pub mod goals;
pub mod health;
pub mod holding_safety;
pub mod holdings;
pub mod import_csv;
pub mod income;
pub mod lifecycle_safety;
pub mod memory_safety;
pub mod performance;
pub mod research_asset;
pub mod record_activities;
pub mod record_activity;
pub mod scenario_alert_safety;
pub mod update_account;
pub mod update_liability;
pub mod valuation;

// Re-export constants
pub use constants::*;

// Re-export tools
pub use accounts::GetAccountsTool;
pub use activities::SearchActivitiesTool;
pub use add_alternative_asset::AddAlternativeAssetTool;
pub use allocation::GetAssetAllocationTool;
pub use cash_balances::GetCashBalancesTool;
pub use create_account::CreateAccountTool;
pub use create_goal::CreateGoalTool;
pub use create_liability::CreateLiabilityTool;
pub use delete_account::DeleteAccountTool;
pub use delete_alternative_asset::DeleteAlternativeAssetTool;
pub use delete_goal::DeleteGoalTool;
pub use delete_liability::DeleteLiabilityTool;
pub use goals::GetGoalsTool;
pub use health::GetHealthStatusTool;
pub use holdings::GetHoldingsTool;
pub use import_csv::ImportCsvTool;
pub use income::GetIncomeTool;
pub use performance::GetPerformanceTool;
pub use research_asset::ResearchAssetTool;
pub use record_activities::RecordActivitiesTool;
pub use record_activity::RecordActivityTool;
pub use update_account::UpdateAccountTool;
pub use update_liability::UpdateLiabilityTool;
pub use valuation::GetValuationHistoryTool;

use std::sync::Arc;

use crate::env::AiEnvironment;

/// Container for all AI tools, simplifying tool registration across providers.
pub struct ToolSet<E: AiEnvironment> {
    pub holdings: GetHoldingsTool<E>,
    pub allocation: GetAssetAllocationTool<E>,
    pub accounts: GetAccountsTool<E>,
    pub cash_balances: GetCashBalancesTool<E>,
    pub activities: SearchActivitiesTool<E>,
    pub income: GetIncomeTool<E>,
    pub valuation: GetValuationHistoryTool<E>,
    pub goals: GetGoalsTool<E>,
    pub performance: GetPerformanceTool<E>,
    pub research_asset: ResearchAssetTool<E>,
    pub record_activity: RecordActivityTool<E>,
    pub record_activities: RecordActivitiesTool<E>,
    pub import_csv: ImportCsvTool<E>,
    pub health_status: GetHealthStatusTool<E>,
    pub create_account: CreateAccountTool<E>,
    pub update_account: UpdateAccountTool<E>,
    pub delete_account: DeleteAccountTool<E>,
    pub create_goal: CreateGoalTool<E>,
    pub delete_goal: DeleteGoalTool<E>,
    pub create_liability: CreateLiabilityTool<E>,
    pub update_liability: UpdateLiabilityTool<E>,
    pub delete_liability: DeleteLiabilityTool<E>,
    pub add_alternative_asset: AddAlternativeAssetTool<E>,
    pub delete_alternative_asset: DeleteAlternativeAssetTool<E>,
}

impl<E: AiEnvironment> ToolSet<E> {
    /// Create a new tool set with all portfolio tools.
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self {
            holdings: GetHoldingsTool::new(env.clone(), base_currency.clone()),
            allocation: GetAssetAllocationTool::new(env.clone(), base_currency.clone()),
            accounts: GetAccountsTool::new(env.clone()),
            cash_balances: GetCashBalancesTool::new(env.clone(), base_currency.clone()),
            activities: SearchActivitiesTool::new(env.clone()),
            income: GetIncomeTool::new(env.clone()),
            valuation: GetValuationHistoryTool::new(env.clone(), base_currency.clone()),
            goals: GetGoalsTool::new(env.clone()),
            performance: GetPerformanceTool::new(env.clone(), base_currency.clone()),
            research_asset: ResearchAssetTool::new(env.clone()),
            record_activity: RecordActivityTool::new(env.clone()),
            record_activities: RecordActivitiesTool::new(env.clone()),
            import_csv: ImportCsvTool::new(env.clone(), base_currency),
            health_status: GetHealthStatusTool::new(env.clone()),
            create_account: CreateAccountTool::new(env.clone()),
            update_account: UpdateAccountTool::new(env.clone()),
            delete_account: DeleteAccountTool::new(env.clone()),
            create_goal: CreateGoalTool::new(env.clone()),
            delete_goal: DeleteGoalTool::new(env.clone()),
            create_liability: CreateLiabilityTool::new(env.clone()),
            update_liability: UpdateLiabilityTool::new(env.clone()),
            delete_liability: DeleteLiabilityTool::new(env.clone()),
            add_alternative_asset: AddAlternativeAssetTool::new(env.clone()),
            delete_alternative_asset: DeleteAlternativeAssetTool::new(env),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::MockEnvironment;

    #[test]
    fn test_tool_set_creation() {
        let env = Arc::new(MockEnvironment::new());
        let _tools = ToolSet::new(env, "USD".to_string());
    }
}
