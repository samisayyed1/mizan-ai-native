/**
 * Tool UI Registry
 *
 * Maps tool names to their makeAssistantToolUI components.
 * These are rendered by @assistant-ui/react when the AI calls tools.
 */

import { AccountsToolUI } from "./accounts-tool-ui";
import { ActivitiesToolUI } from "./activities-tool-ui";
import { AddAlternativeAssetToolUI } from "./add-alternative-asset-tool-ui";
import { AllocationToolUI } from "./allocation-tool-ui";
import { CreateAccountToolUI } from "./create-account-tool-ui";
import { CreateGoalToolUI } from "./create-goal-tool-ui";
import { CreateLiabilityToolUI } from "./create-liability-tool-ui";
import { GoalsToolUI } from "./goals-tool-ui";
import { UpdateAccountToolUI } from "./update-account-tool-ui";
import { HoldingsToolUI } from "./holdings-tool-ui";
import { ImportCsvToolUI } from "./import-csv-tool-ui";
import { IncomeToolUI } from "./income-tool-ui";
import { PerformanceToolUI } from "./performance-tool-ui";
import { RecordActivityToolUI } from "./record-activity-tool-ui";
import { RecordActivitiesToolUI } from "./record-activities-tool-ui";
import { ValuationToolUI } from "./valuation-tool-ui";

/**
 * Registry of tool UIs keyed by tool name.
 * Used by MessagePrimitive.Parts in thread.tsx.
 */
export const toolUIs = {
  add_alternative_asset: AddAlternativeAssetToolUI,
  create_account: CreateAccountToolUI,
  create_goal: CreateGoalToolUI,
  create_liability: CreateLiabilityToolUI,
  get_accounts: AccountsToolUI,
  get_asset_allocation: AllocationToolUI,
  get_goals: GoalsToolUI,
  get_holdings: HoldingsToolUI,
  get_income: IncomeToolUI,
  get_performance: PerformanceToolUI,
  get_valuation_history: ValuationToolUI,
  import_csv: ImportCsvToolUI,
  record_activity: RecordActivityToolUI,
  record_activities: RecordActivitiesToolUI,
  search_activities: ActivitiesToolUI,
  update_account: UpdateAccountToolUI,
} as const;

export type ToolUIName = keyof typeof toolUIs;

// Re-export individual components for direct imports if needed
export {
  AccountsToolUI,
  ActivitiesToolUI,
  AddAlternativeAssetToolUI,
  AllocationToolUI,
  CreateAccountToolUI,
  CreateGoalToolUI,
  CreateLiabilityToolUI,
  GoalsToolUI,
  HoldingsToolUI,
  ImportCsvToolUI,
  IncomeToolUI,
  PerformanceToolUI,
  RecordActivityToolUI,
  RecordActivitiesToolUI,
  UpdateAccountToolUI,
  ValuationToolUI,
};

// Re-export shared components
export * from "./shared";
