//! Monthly AI Wealth Report commands (M3.6).
//!
//! Both pass through to the cloud's `/v1/reports/monthly` endpoints — the
//! cloud cron writes the rows, this side reads + requests regeneration.

use std::sync::Arc;

use mizan_connect::{MonthlyReport, MonthlyReportsResponse};
use tauri::State;

use crate::context::ServiceContext;

#[tauri::command(rename_all = "camelCase")]
pub async fn list_monthly_reports(
    limit: Option<i64>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<MonthlyReportsResponse, String> {
    state
        .connect_service()
        .list_monthly_reports(limit.unwrap_or(12))
        .await
}

#[tauri::command]
pub async fn request_monthly_report(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<MonthlyReport, String> {
    state.connect_service().request_monthly_report().await
}
