use std::collections::HashMap;
use std::sync::Arc;

use crate::context::ServiceContext;
use log::debug;
use mizan_core::activities::{
    Activity, ActivityBulkMutationRequest, ActivityBulkMutationResult, ActivityImport,
    ActivitySearchResponse, ActivityUpdate, FdParams, FdPaymentFrequency, ImportActivitiesResult,
    ImportAssetCandidate, ImportAssetPreviewItem, ImportMappingData, ImportTemplateData,
    NewActivity, ParseConfig, ParsedCsvResult, RspFrequency, RspParams, Sort,
};
use rust_decimal::Decimal;
use tauri::State;

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn search_activities(
    page: i64,                                 // Page number, 0-based
    page_size: i64,                            // Number of items per page
    account_id_filter: Option<Vec<String>>,    // Optional account_id filter
    activity_type_filter: Option<Vec<String>>, // Optional activity_type filter
    asset_id_keyword: Option<String>,          // Optional asset_id keyword for search
    sort: Option<Sort>,
    needs_review_filter: Option<bool>, // Optional needs_review filter for pending review
    date_from: Option<String>,         // Optional start date filter (YYYY-MM-DD, inclusive)
    date_to: Option<String>,           // Optional end date filter (YYYY-MM-DD, inclusive)
    instrument_type_filter: Option<Vec<String>>, // Optional instrument_type filter
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ActivitySearchResponse, String> {
    debug!("Search activities... {}, {}", page, page_size);

    // Parse date strings to NaiveDate
    let date_from_parsed = date_from
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|e| format!("Invalid date_from format: {}", e))?;
    let date_to_parsed = date_to
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|e| format!("Invalid date_to format: {}", e))?;

    Ok(state.activity_service().search_activities(
        page,
        page_size,
        account_id_filter,
        activity_type_filter,
        asset_id_keyword,
        sort,
        needs_review_filter,
        date_from_parsed,
        date_to_parsed,
        instrument_type_filter,
    )?)
}

#[tauri::command]
pub async fn create_activity(
    activity: NewActivity,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Activity, String> {
    debug!("Creating activity...");
    // Domain events handle recalculation and asset enrichment automatically
    state
        .activity_service()
        .create_activity(activity)
        .await
        .map_err(|e| e.to_string())
}

/// Frontend-facing payload for `create_fixed_deposit`. Mirrors `FdParams`
/// but takes the start date as an ISO `YYYY-MM-DD` string (the frontend
/// always sends dates as strings) and the payment frequency as a
/// snake_case enum string the JS layer can build trivially.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFixedDepositRequest {
    pub account_id: String,
    pub principal: Decimal,
    /// Annual rate as a percent (5.0 = 5%).
    pub annual_rate_percent: Decimal,
    pub term_months: u32,
    /// One of: monthly, quarterly, semi_annual, annual, at_maturity.
    pub payment_frequency: FdPaymentFrequency,
    /// `YYYY-MM-DD`.
    pub start_date: String,
    pub currency: String,
    pub notes: Option<String>,
}

/// Schedule a fixed deposit: emits a series of INTEREST activities at
/// the configured cadence into the chosen account. Returns the inserted
/// activities so the frontend can surface counts / sums in the success
/// toast without an extra round-trip.
#[tauri::command]
pub async fn create_fixed_deposit(
    request: CreateFixedDepositRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<Activity>, String> {
    debug!(
        "Creating fixed deposit: account={}, principal={}, rate={}%, term={}mo, freq={:?}",
        request.account_id,
        request.principal,
        request.annual_rate_percent,
        request.term_months,
        request.payment_frequency
    );

    let start_date = chrono::NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid start_date (expected YYYY-MM-DD): {}", e))?;

    let params = FdParams {
        account_id: request.account_id,
        principal: request.principal,
        annual_rate_percent: request.annual_rate_percent,
        term_months: request.term_months,
        payment_frequency: request.payment_frequency,
        start_date,
        currency: request.currency,
        notes: request.notes,
    };

    state
        .activity_service()
        .create_fixed_deposit_schedule(params)
        .await
        .map_err(|e| e.to_string())
}

/// Frontend-facing payload for `create_recurring_buy_plan`. Mirrors
/// `RspParams` but takes the start date as an ISO `YYYY-MM-DD` string
/// (the frontend always sends dates as strings) and the frequency as a
/// snake_case enum string the JS layer can build trivially.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecurringBuyPlanRequest {
    pub account_id: String,
    pub symbol: String,
    pub exchange_mic: Option<String>,
    pub amount_per_buy: Decimal,
    pub unit_price: Decimal,
    /// One of: weekly, biweekly, monthly, quarterly, semi_annual, annual.
    pub frequency: RspFrequency,
    /// `YYYY-MM-DD`.
    pub start_date: String,
    pub installments: u32,
    pub currency: String,
    pub notes: Option<String>,
}

/// Schedule a recurring stock-purchase plan: emits a series of BUY
/// activities at the configured cadence into the chosen account, each
/// for `amount_per_buy / unit_price` shares of the chosen symbol.
/// Returns the inserted activities so the frontend can surface counts /
/// sums in the success toast without an extra round-trip.
#[tauri::command]
pub async fn create_recurring_buy_plan(
    request: CreateRecurringBuyPlanRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<Activity>, String> {
    debug!(
        "Creating recurring buy plan: account={}, symbol={}, amount={}, price={}, freq={:?}, n={}",
        request.account_id,
        request.symbol,
        request.amount_per_buy,
        request.unit_price,
        request.frequency,
        request.installments
    );

    let start_date = chrono::NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid start_date (expected YYYY-MM-DD): {}", e))?;

    let params = RspParams {
        account_id: request.account_id,
        symbol: request.symbol,
        exchange_mic: request.exchange_mic,
        amount_per_buy: request.amount_per_buy,
        unit_price: request.unit_price,
        frequency: request.frequency,
        start_date,
        installments: request.installments,
        currency: request.currency,
        notes: request.notes,
    };

    state
        .activity_service()
        .create_recurring_buy_plan(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_activity(
    activity: ActivityUpdate,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Activity, String> {
    debug!("Updating activity...");
    // Domain events handle recalculation and asset enrichment automatically
    state
        .activity_service()
        .update_activity(activity)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_activity(
    activity_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Activity, String> {
    debug!("Deleting activity...");
    // Domain events handle recalculation automatically
    state
        .activity_service()
        .delete_activity(activity_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn link_transfer_activities(
    activity_a_id: String,
    activity_b_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(Activity, Activity), String> {
    debug!("Linking transfer activities...");
    // Domain events handle recalculation automatically
    state
        .activity_service()
        .link_transfer_activities(activity_a_id, activity_b_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unlink_transfer_activities(
    activity_a_id: String,
    activity_b_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(Activity, Activity), String> {
    debug!("Unlinking transfer activities...");
    // Domain events handle recalculation automatically
    state
        .activity_service()
        .unlink_transfer_activities(activity_a_id, activity_b_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_activities(
    request: ActivityBulkMutationRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ActivityBulkMutationResult, String> {
    let create_count = request.creates.len();
    let update_count = request.updates.len();
    let delete_count = request.delete_ids.len();
    debug!(
        "Bulk activity mutation request: {} creates, {} updates, {} deletes",
        create_count, update_count, delete_count
    );

    // Domain events handle recalculation and asset enrichment automatically
    state
        .activity_service()
        .bulk_mutate_activities(request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_account_import_mapping(
    account_id: String,
    context_kind: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportMappingData, String> {
    debug!("Getting import mapping for account: {}", account_id);
    Ok(state
        .activity_service()
        .get_import_mapping(account_id, context_kind)?)
}

#[tauri::command]
pub async fn save_account_import_mapping(
    mapping: ImportMappingData,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportMappingData, String> {
    debug!("Saving import mapping for account: {}", mapping.account_id);
    state
        .activity_service()
        .save_import_mapping(mapping)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn link_account_template(
    account_id: String,
    template_id: String,
    context_kind: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    debug!("Linking account {} to template {}", account_id, template_id);
    state
        .activity_service()
        .link_account_template(account_id, template_id, context_kind)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_import_templates(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ImportTemplateData>, String> {
    Ok(state.activity_service().list_import_templates()?)
}

#[tauri::command]
pub async fn get_import_template(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportTemplateData, String> {
    Ok(state.activity_service().get_import_template(id)?)
}

#[tauri::command]
pub async fn save_import_template(
    template: ImportTemplateData,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportTemplateData, String> {
    state
        .activity_service()
        .save_import_template(template)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_import_template(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .activity_service()
        .delete_import_template(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_activities_import(
    activities: Vec<ActivityImport>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ActivityImport>, String> {
    debug!("Checking activities import for {} rows", activities.len());
    let result = state
        .activity_service()
        .check_activities_import(activities)
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn preview_import_assets(
    candidates: Vec<ImportAssetCandidate>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ImportAssetPreviewItem>, String> {
    let result = state
        .activity_service()
        .preview_import_assets(candidates)
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn import_activities(
    activities: Vec<ActivityImport>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportActivitiesResult, String> {
    debug!("Importing {} activities", activities.len());

    // CSV import gate. Free tier still gets one import/month; the desktop
    // can't count cleanly without an ImportRun query, so for the boolean
    // gate we just check `csv_imports_monthly > 0`. The cloud's
    // `/v1/usage` ledger does the authoritative monthly-cap enforcement
    // (the desktop reports usage after each successful import — see
    // [`crate::commands::entitlements::report_usage`]).
    let entitlements = crate::commands::entitlements::resolve_entitlements(&state).await;
    crate::commands::entitlements::gated(
        entitlements.csv_imports_monthly != 0,
        "csv_imports",
        "basic",
        &entitlements.plan,
        "CSV import with AI column mapping is included with a Mizan subscription.",
    )?;

    // Domain events handle recalculation and asset enrichment automatically
    let result = state
        .activity_service()
        .import_activities(activities)
        .await
        .map_err(|e| e.to_string())?;

    // Fire-and-forget usage report to the cloud ledger (B5).
    state.connect_service().report_usage("csv_intel", 1).await;

    Ok(result)
}

#[tauri::command]
pub async fn check_existing_duplicates(
    idempotency_keys: Vec<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<HashMap<String, String>, String> {
    debug!(
        "Checking for existing duplicates with {} idempotency keys",
        idempotency_keys.len()
    );
    state
        .activity_service()
        .check_existing_duplicates(idempotency_keys)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_csv(
    content: Vec<u8>,
    config: ParseConfig,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ParsedCsvResult, String> {
    debug!(
        "Parsing CSV with {} bytes, config: {:?}",
        content.len(),
        config
    );
    state
        .activity_service()
        .parse_csv(&content, &config)
        .map_err(|e| {
            debug!("CSV parse error: {}", e);
            e.to_string()
        })
}

/// Result of `analyze_csv_import` — everything the import-preview UI
/// needs to show in one round-trip.
#[derive(Debug, serde::Serialize)]
pub struct CsvImportAnalysis {
    /// Headers detected by the parser.
    pub headers: Vec<String>,
    /// First N data rows (preview only — full row count is in
    /// `summary.stats.total_input_rows`).
    pub sample_rows: Vec<Vec<String>>,
    /// Auto-detected field → header column mapping. Frontend
    /// pre-fills the mapping editor with this; user can still override.
    pub field_mappings: HashMap<String, String>,
    /// Filter + dedupe stats and aggregate totals.
    pub summary: mizan_ai::tools::csv_intel::ImportSummary,
}

/// Parse + analyse a CSV in one round-trip for the import-preview UI.
///
/// This is the "confidence indicator" the user sees before
/// committing: total cost basis, sell proceeds, row counts kept vs
/// dropped (watchlist / zero-value / duplicates). The user can
/// eyeball the BUY total against what they expect their portfolio
/// to be worth and bail out if the number looks wildly off — which
/// usually means the column mapping is wrong.
#[tauri::command]
pub async fn analyze_csv_import(
    content: Vec<u8>,
    config: ParseConfig,
    sample_size: Option<usize>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<CsvImportAnalysis, String> {
    debug!(
        "Analysing CSV import: {} bytes, config: {:?}",
        content.len(),
        config
    );

    let parsed = state
        .activity_service()
        .parse_csv(&content, &config)
        .map_err(|e| {
            debug!("CSV parse error: {}", e);
            e.to_string()
        })?;

    let field_mappings =
        mizan_ai::tools::csv_intel::detect_field_mappings_smart(&parsed.headers, &parsed.rows);

    let summary = mizan_ai::tools::csv_intel::filter_dedupe_and_summarise(
        &parsed.headers,
        &field_mappings,
        &parsed.rows,
    );

    // Trim sample_rows to the requested size so the IPC payload
    // doesn't ship a 400-row CSV body to the frontend. The summary
    // already counted everything; the UI just needs a few rows for
    // the preview table.
    let limit = sample_size.unwrap_or(50).min(parsed.rows.len());
    let sample_rows: Vec<Vec<String>> = parsed.rows.into_iter().take(limit).collect();

    Ok(CsvImportAnalysis {
        headers: parsed.headers,
        sample_rows,
        field_mappings,
        summary,
    })
}
