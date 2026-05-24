// Activity Commands
import { ImportType } from "@/lib/types";
import type {
  Activity,
  ActivityBulkMutationRequest,
  ActivityBulkMutationResult,
  ActivityCreate,
  ActivityDetails,
  ActivitySearchResponse,
  ActivityUpdate,
  ActivityImport,
  ImportAssetCandidate,
  ImportAssetPreviewItem,
  ImportActivitiesResult,
  ImportMappingData,
  ImportTemplateData,
  BrokerSyncProfileData,
  SaveBrokerSyncProfileRulesRequest,
} from "@/lib/types";

import { invoke, logger } from "./platform";

interface ActivityFilters {
  accountIds?: string | string[];
  activityTypes?: string | string[];
  symbol?: string;
  needsReview?: boolean;
  dateFrom?: string; // YYYY-MM-DD format
  dateTo?: string; // YYYY-MM-DD format
  instrumentTypes?: string | string[];
}

interface ActivitySort {
  id: string;
  desc?: boolean;
}

function normalizeStringArray(input?: string | string[]): string[] | undefined {
  if (!input) return undefined;
  if (Array.isArray(input)) {
    return input.length > 0 ? input : undefined;
  }
  return input.length > 0 ? [input] : undefined;
}

export const getActivities = async (accountId?: string): Promise<ActivityDetails[]> => {
  try {
    const response = await searchActivities(
      0,
      Number.MAX_SAFE_INTEGER,
      accountId ? { accountIds: [accountId] } : {},
      "",
      { id: "date", desc: true },
    );
    return response.data;
  } catch (err) {
    logger.error("Error fetching all activities.");
    throw err;
  }
};

export const searchActivities = async (
  page: number,
  pageSize: number,
  filters: ActivityFilters,
  searchKeyword: string,
  sort?: ActivitySort,
): Promise<ActivitySearchResponse> => {
  const accountIdFilter = normalizeStringArray(filters?.accountIds);
  const activityTypeFilter = normalizeStringArray(filters?.activityTypes);
  const instrumentTypeFilter = normalizeStringArray(filters?.instrumentTypes);
  const assetIdKeywordRaw = filters?.symbol ?? searchKeyword;
  const assetIdKeyword = assetIdKeywordRaw?.trim() ? assetIdKeywordRaw.trim() : undefined;
  const sortOption = sort?.id
    ? { id: sort.id, desc: sort.desc ?? false }
    : { id: "date", desc: true };
  const needsReviewFilter = filters?.needsReview;
  const dateFrom = filters?.dateFrom;
  const dateTo = filters?.dateTo;

  try {
    return await invoke<ActivitySearchResponse>("search_activities", {
      page,
      pageSize,
      accountIdFilter,
      activityTypeFilter,
      assetIdKeyword,
      sort: sortOption,
      needsReviewFilter,
      dateFrom,
      dateTo,
      instrumentTypeFilter,
    });
  } catch (err) {
    logger.error("Error fetching activities.");
    throw err;
  }
};

export const createActivity = async (activity: ActivityCreate): Promise<Activity> => {
  try {
    return await invoke<Activity>("create_activity", { activity });
  } catch (err) {
    logger.error("Error creating activity.");
    throw err;
  }
};

/**
 * Fixed-deposit creation request. Mirrors the Rust `CreateFixedDepositRequest`
 * struct on the Tauri command — Tauri's serde binding camelCases field
 * names so we send `accountId`, `annualRatePercent`, etc.
 */
export type FdPaymentFrequency = "monthly" | "quarterly" | "semi_annual" | "annual" | "at_maturity";

export interface CreateFixedDepositRequest {
  accountId: string;
  /** Decimal as string is also accepted by serde; sending number works for
   *  the principal sizes we expect (< 1e15 USD). */
  principal: string | number;
  /** 5 = 5%. */
  annualRatePercent: string | number;
  termMonths: number;
  paymentFrequency: FdPaymentFrequency;
  /** YYYY-MM-DD. */
  startDate: string;
  currency: string;
  notes?: string | null;
}

/**
 * Schedule a fixed deposit. Backend emits a series of INTEREST activities
 * at the configured cadence and returns them in payment-date order.
 */
export const createFixedDeposit = async (
  request: CreateFixedDepositRequest,
): Promise<Activity[]> => {
  try {
    return await invoke<Activity[]>("create_fixed_deposit", { request });
  } catch (err) {
    logger.error("Error creating fixed deposit.");
    throw err;
  }
};

/**
 * Recurring stock-purchase plan creation request. Mirrors the Rust
 * `CreateRecurringBuyPlanRequest` struct on the Tauri command — Tauri's
 * serde binding camelCases field names so we send `accountId`,
 * `amountPerBuy`, etc.
 */
export type RspFrequency =
  | "weekly"
  | "biweekly"
  | "monthly"
  | "quarterly"
  | "semi_annual"
  | "annual";

export interface CreateRecurringBuyPlanRequest {
  accountId: string;
  /** Ticker symbol (e.g. "VTI", "AAPL"). Resolved to an asset on insert. */
  symbol: string;
  /** Optional MIC code for dual-listed tickers. */
  exchangeMic?: string | null;
  /** Cash spent per scheduled buy. */
  amountPerBuy: string | number;
  /** Per-share price applied to every emitted BUY. quantity = amount / price. */
  unitPrice: string | number;
  frequency: RspFrequency;
  /** YYYY-MM-DD. */
  startDate: string;
  /** Number of buys to generate (1..=240). */
  installments: number;
  currency: string;
  notes?: string | null;
}

/**
 * Schedule a recurring stock-purchase plan. Backend emits a series of
 * BUY activities at the configured cadence and returns them in
 * purchase-date order.
 */
export const createRecurringBuyPlan = async (
  request: CreateRecurringBuyPlanRequest,
): Promise<Activity[]> => {
  try {
    return await invoke<Activity[]>("create_recurring_buy_plan", { request });
  } catch (err) {
    logger.error("Error creating recurring buy plan.");
    throw err;
  }
};

export const updateActivity = async (activity: ActivityUpdate): Promise<Activity> => {
  try {
    return await invoke<Activity>("update_activity", { activity });
  } catch (err) {
    logger.error("Error updating activity.");
    throw err;
  }
};

export const saveActivities = async (
  request: ActivityBulkMutationRequest,
): Promise<ActivityBulkMutationResult> => {
  const payload: ActivityBulkMutationRequest = {
    creates: request.creates ?? [],
    updates: request.updates ?? [],
    deleteIds: request.deleteIds ?? [],
  };
  try {
    return await invoke<ActivityBulkMutationResult>("save_activities", { request: payload });
  } catch (err) {
    logger.error("Error saving activities.");
    throw err;
  }
};

export const deleteActivity = async (activityId: string): Promise<Activity> => {
  try {
    return await invoke<Activity>("delete_activity", { activityId });
  } catch (err) {
    logger.error("Error deleting activity.");
    throw err;
  }
};

export const linkTransferActivities = async (
  activityAId: string,
  activityBId: string,
): Promise<[Activity, Activity]> => {
  try {
    return await invoke<[Activity, Activity]>("link_transfer_activities", {
      activityAId,
      activityBId,
    });
  } catch (err) {
    logger.error("Error linking transfer activities.");
    throw err;
  }
};

export const unlinkTransferActivities = async (
  activityAId: string,
  activityBId: string,
): Promise<[Activity, Activity]> => {
  try {
    return await invoke<[Activity, Activity]>("unlink_transfer_activities", {
      activityAId,
      activityBId,
    });
  } catch (err) {
    logger.error("Error unlinking transfer activities.");
    throw err;
  }
};

// ============================================================================
// Activity Import Commands
// ============================================================================

/**
 * Import activities into the system.
 * Expects activities that already passed backend check/preview resolution.
 * Apply is persistence-only and rejects missing resolved symbol fields.
 * Returns ImportActivitiesResult with activities, import_run_id, and summary.
 */
export const importActivities = async ({
  activities,
}: {
  activities: ActivityImport[];
}): Promise<ImportActivitiesResult> => {
  try {
    return await invoke<ImportActivitiesResult>("import_activities", { activities });
  } catch (err) {
    logger.error(`Error importing activities: ${err}`);
    throw err;
  }
};

/**
 * Check activities before import (read-only validation/preview).
 * This performs read-only validation without creating assets or FX pairs.
 * Asset creation happens during the actual import when user confirms.
 * @param activities - The activities to validate
 */
export const checkActivitiesImport = async ({
  activities,
}: {
  activities: ActivityImport[];
}): Promise<ActivityImport[]> => {
  try {
    return await invoke<ActivityImport[]>("check_activities_import", { activities });
  } catch (err) {
    logger.error(`Error checking activities import: ${err}`);
    throw err;
  }
};

export const listImportTemplates = async (): Promise<ImportTemplateData[]> => {
  try {
    return await invoke<ImportTemplateData[]>("list_import_templates");
  } catch (err) {
    logger.error("Error listing import templates.");
    throw err;
  }
};

export const getImportTemplate = async (id: string): Promise<ImportTemplateData> => {
  try {
    return await invoke<ImportTemplateData>("get_import_template", { id });
  } catch (err) {
    logger.error("Error fetching import template.");
    throw err;
  }
};

export const saveImportTemplate = async (
  template: ImportTemplateData,
): Promise<ImportTemplateData> => {
  try {
    return await invoke<ImportTemplateData>("save_import_template", { template });
  } catch (err) {
    logger.error("Error saving import template.");
    throw err;
  }
};

export const deleteImportTemplate = async (id: string): Promise<void> => {
  try {
    await invoke<void>("delete_import_template", { id });
  } catch (err) {
    logger.error("Error deleting import template.");
    throw err;
  }
};

/**
 * Preview which assets would be created or matched for a set of import candidates.
 */
export const previewImportAssets = async ({
  candidates,
}: {
  candidates: ImportAssetCandidate[];
}): Promise<ImportAssetPreviewItem[]> => {
  try {
    return await invoke<ImportAssetPreviewItem[]>("preview_import_assets", { candidates });
  } catch (err) {
    logger.error(`Error previewing import assets: ${err}`);
    throw err;
  }
};

/**
 * Get the import mapping configuration for an account.
 */
export const getAccountImportMapping = async (
  accountId: string,
  contextKind: string = ImportType.ACTIVITY,
): Promise<ImportMappingData> => {
  try {
    return await invoke<ImportMappingData>("get_account_import_mapping", {
      accountId,
      contextKind,
    });
  } catch (err) {
    logger.error("Error fetching mapping.");
    throw err;
  }
};

/**
 * Link an account to an existing import template.
 */
export const linkAccountTemplate = async (
  accountId: string,
  templateId: string,
  contextKind: string = ImportType.ACTIVITY,
): Promise<void> => {
  try {
    await invoke<void>("link_account_template", { accountId, templateId, contextKind });
  } catch (err) {
    logger.error("Error linking account to template.");
    throw err;
  }
};

/**
 * Save the import mapping configuration for an account.
 */
export const saveAccountImportMapping = async (
  mapping: ImportMappingData,
): Promise<ImportMappingData> => {
  try {
    return await invoke<ImportMappingData>("save_account_import_mapping", { mapping });
  } catch (err) {
    logger.error("Error saving mapping.");
    throw err;
  }
};

/**
 * Check for existing duplicate activities based on idempotency keys.
 * Returns a map of {idempotency_key: existing_activity_id} for duplicates found.
 */
export const checkExistingDuplicates = async (
  idempotencyKeys: string[],
): Promise<Record<string, string>> => {
  try {
    return await invoke<Record<string, string>>("check_existing_duplicates", { idempotencyKeys });
  } catch (err) {
    logger.error("Error checking for duplicate activities.");
    throw err;
  }
};

// ============================================================================
// Broker Sync Profile Commands
// ============================================================================

export const getBrokerSyncProfile = async (
  accountId: string,
  sourceSystem: string,
): Promise<BrokerSyncProfileData> => {
  try {
    return await invoke<BrokerSyncProfileData>("get_broker_sync_profile", {
      accountId,
      sourceSystem,
    });
  } catch (err) {
    logger.error("Error fetching broker sync profile.");
    throw err;
  }
};

export const saveBrokerSyncProfileRules = async (
  request: SaveBrokerSyncProfileRulesRequest,
): Promise<BrokerSyncProfileData> => {
  try {
    return await invoke<BrokerSyncProfileData>("save_broker_sync_profile_rules", { request });
  } catch (err) {
    logger.error("Error saving broker sync profile rules.");
    throw err;
  }
};
