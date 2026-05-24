//! Sync Run Ledger data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where this sync came from. New providers add a variant — they aren't
/// stringified at the boundary so a typo doesn't slip past compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRunProvider {
    Plaid,
    SnapTrade,
    Yahoo,
    TradingView,
    CsvImport,
    /// AI assistant-driven create/update.
    AiTool,
    /// Background FX rate refresh.
    FxRefresh,
    /// User-initiated holdings snapshot.
    Manual,
}

impl SyncRunProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plaid => "plaid",
            Self::SnapTrade => "snaptrade",
            Self::Yahoo => "yahoo",
            Self::TradingView => "tradingview",
            Self::CsvImport => "csv_import",
            Self::AiTool => "ai_tool",
            Self::FxRefresh => "fx_refresh",
            Self::Manual => "manual",
        }
    }
}

/// Sync mode — matches the existing market-data MarketSyncMode +
/// activity-import ImportRunMode taxonomy so existing call sites can
/// drop in without a translation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRunMode {
    /// First run for this provider/account.
    Initial,
    /// Catch-up since last successful sync.
    Incremental,
    /// Full historical re-fetch.
    Backfill,
    /// Targeted fix for a known bad state (e.g. duplicate rows).
    Repair,
    /// One-off — doesn't fit the recurring sync taxonomy.
    OneOff,
}

impl SyncRunMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Incremental => "incremental",
            Self::Backfill => "backfill",
            Self::Repair => "repair",
            Self::OneOff => "one_off",
        }
    }
}

/// Final outcome of the sync attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRunOutcome {
    /// Fully applied. No rows skipped, no errors.
    Applied,
    /// Applied with warnings — some rows skipped or needs-review.
    AppliedWithWarnings,
    /// User cancelled before the sync completed.
    Cancelled,
    /// Sync hit a hard error and applied nothing.
    Failed,
}

impl SyncRunOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::AppliedWithWarnings => "applied_with_warnings",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Convenience for "did this sync land any data?".
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Applied | Self::AppliedWithWarnings)
    }
}

/// Per-run summary counters. All fields default to 0 so call sites can
/// fill in only what's relevant for their provider.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunSummary {
    pub fetched: u32,
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub warnings: u32,
    pub errors: u32,
    pub removed: u32,
}

impl SyncRunSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Derive the outcome from the counters. `errors > 0 && inserted == 0`
    /// → Failed; any warnings/skipped → AppliedWithWarnings; else Applied.
    pub fn derive_outcome(&self) -> SyncRunOutcome {
        if self.errors > 0 && self.inserted == 0 && self.updated == 0 {
            SyncRunOutcome::Failed
        } else if self.warnings > 0 || self.skipped > 0 || self.errors > 0 {
            SyncRunOutcome::AppliedWithWarnings
        } else {
            SyncRunOutcome::Applied
        }
    }
}

/// One row in the append-only ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunEntry {
    /// UUID assigned at run start. Used by callers to correlate logs
    /// with the ledger row.
    pub run_id: String,
    pub provider: SyncRunProvider,
    pub mode: SyncRunMode,
    /// Optional account id this run is scoped to (Plaid + SnapTrade
    /// runs always have one; market-data runs typically don't).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Optional AI thread id when the sync was kicked off by an AI tool
    /// call — connects the §A6 audit trail to the §A4 ledger.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub started_at: DateTime<Utc>,
    /// `None` until the run finishes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub outcome: SyncRunOutcome,
    pub summary: SyncRunSummary,
    /// Serialised MizanError JSON when outcome = Failed. Empty otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SyncRunEntry {
    /// Construct a fresh "started" entry. Caller updates outcome +
    /// summary + finished_at when the run completes.
    pub fn started(
        run_id: impl Into<String>,
        provider: SyncRunProvider,
        mode: SyncRunMode,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            provider,
            mode,
            account_id: None,
            thread_id: None,
            started_at: Utc::now(),
            finished_at: None,
            outcome: SyncRunOutcome::Applied,
            summary: SyncRunSummary::default(),
            error: None,
        }
    }

    pub fn with_account(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Mark the run finished + derive outcome from the summary counters.
    pub fn finish(mut self, summary: SyncRunSummary) -> Self {
        self.outcome = summary.derive_outcome();
        self.summary = summary;
        self.finished_at = Some(Utc::now());
        self
    }

    /// Mark the run failed with a structured-error JSON payload (see §A24).
    pub fn fail(mut self, error_json: impl Into<String>) -> Self {
        self.outcome = SyncRunOutcome::Failed;
        self.error = Some(error_json.into());
        self.finished_at = Some(Utc::now());
        self
    }

    /// Mark the run cancelled.
    pub fn cancel(mut self) -> Self {
        self.outcome = SyncRunOutcome::Cancelled;
        self.finished_at = Some(Utc::now());
        self
    }

    /// Elapsed time — `None` until `finish` / `fail` / `cancel` ran.
    pub fn elapsed(&self) -> Option<chrono::Duration> {
        self.finished_at.map(|f| f - self.started_at)
    }
}
