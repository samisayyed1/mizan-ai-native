//! Diesel row model for `sync_run_ledger`.
//!
//! Storage is timestamp-as-millis; the domain `SyncRunEntry` exposes
//! `chrono::DateTime<Utc>` — `From`/`Into` adapt between them.

use chrono::{DateTime, TimeZone, Utc};
use diesel::prelude::*;
use serde_json;

use crate::schema::sync_run_ledger;
use mizan_core::sync_ledger::{
    SyncRunEntry, SyncRunMode, SyncRunOutcome, SyncRunProvider, SyncRunSummary,
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = sync_run_ledger)]
#[diesel(treat_none_as_null = true)]
pub struct SyncRunLedgerDB {
    pub run_id: String,
    pub provider: String,
    pub mode: String,
    pub account_id: Option<String>,
    pub thread_id: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub outcome: String,
    pub summary_json: String,
    pub error_json: Option<String>,
    pub created_at: i64,
}

fn provider_from_str(s: &str) -> SyncRunProvider {
    match s {
        "plaid" => SyncRunProvider::Plaid,
        "snaptrade" => SyncRunProvider::SnapTrade,
        "yahoo" => SyncRunProvider::Yahoo,
        "tradingview" => SyncRunProvider::TradingView,
        "market_data" => SyncRunProvider::MarketData,
        "csv_import" => SyncRunProvider::CsvImport,
        "ai_tool" => SyncRunProvider::AiTool,
        "fx_refresh" => SyncRunProvider::FxRefresh,
        "manual" => SyncRunProvider::Manual,
        // Unknown providers persisted by older binaries default to Manual
        // so the row is still returnable to the UI without crashing.
        _ => SyncRunProvider::Manual,
    }
}

fn mode_from_str(s: &str) -> SyncRunMode {
    match s {
        "initial" => SyncRunMode::Initial,
        "incremental" => SyncRunMode::Incremental,
        "backfill" => SyncRunMode::Backfill,
        "repair" => SyncRunMode::Repair,
        "one_off" => SyncRunMode::OneOff,
        _ => SyncRunMode::OneOff,
    }
}

fn outcome_from_str(s: &str) -> SyncRunOutcome {
    match s {
        "applied" => SyncRunOutcome::Applied,
        "applied_with_warnings" => SyncRunOutcome::AppliedWithWarnings,
        "cancelled" => SyncRunOutcome::Cancelled,
        "failed" => SyncRunOutcome::Failed,
        _ => SyncRunOutcome::Failed,
    }
}

fn millis_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

fn dt_to_millis(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

impl From<&SyncRunEntry> for SyncRunLedgerDB {
    fn from(e: &SyncRunEntry) -> Self {
        let summary_json = serde_json::to_string(&e.summary).unwrap_or_else(|_| "{}".to_string());
        Self {
            run_id: e.run_id.clone(),
            provider: e.provider.as_str().to_string(),
            mode: e.mode.as_str().to_string(),
            account_id: e.account_id.clone(),
            thread_id: e.thread_id.clone(),
            started_at: dt_to_millis(e.started_at),
            finished_at: e.finished_at.map(dt_to_millis),
            outcome: e.outcome.as_str().to_string(),
            summary_json,
            error_json: e.error.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl From<SyncRunLedgerDB> for SyncRunEntry {
    fn from(r: SyncRunLedgerDB) -> Self {
        let summary: SyncRunSummary =
            serde_json::from_str(&r.summary_json).unwrap_or_else(|_| SyncRunSummary::default());
        Self {
            run_id: r.run_id,
            provider: provider_from_str(&r.provider),
            mode: mode_from_str(&r.mode),
            account_id: r.account_id,
            thread_id: r.thread_id,
            started_at: millis_to_dt(r.started_at),
            finished_at: r.finished_at.map(millis_to_dt),
            outcome: outcome_from_str(&r.outcome),
            summary,
            error: r.error_json,
        }
    }
}
