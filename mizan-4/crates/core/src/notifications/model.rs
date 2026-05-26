//! Typed model for a single user-visible notification row.

use serde::{Deserialize, Serialize};

/// Stable category slug. Persisted as TEXT in SQLite (`notifications.kind`).
/// Frontend uses this for the row icon + grouping.
///
/// NEW VARIANT POLICY: only add — never rename or remove. Persisted
/// rows from older app versions must remain readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// A single position moved more than ±5% in a day.
    BigMove,
    /// Allocation drifted more than 10pp vs target.
    AllocationDrift,
    /// Goal hit a milestone (25/50/75/90/100%).
    GoalMilestone,
    /// Cash > 10% of net worth for 30+ days.
    CashDrag,
    /// A dividend / interest credit was posted by broker sync.
    DividendPosted,
    /// Portfolio hit a new all-time high (closing basis).
    NewAth,
    /// Net worth dipped > 3% week-over-week.
    NetWorthDip,
    /// Broker / market-data / FX sync failed and is degrading data quality.
    SyncFailure,
    /// AI-generated 2-sentence digest summarising the day's insights.
    /// Emitted at most once per user per day.
    AiDigest,
}

impl NotificationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BigMove => "big_move",
            Self::AllocationDrift => "allocation_drift",
            Self::GoalMilestone => "goal_milestone",
            Self::CashDrag => "cash_drag",
            Self::DividendPosted => "dividend_posted",
            Self::NewAth => "new_ath",
            Self::NetWorthDip => "net_worth_dip",
            Self::SyncFailure => "sync_failure",
            Self::AiDigest => "ai_digest",
        }
    }

    /// Parse from the SQLite TEXT column. Unknown values map to
    /// `BigMove` and the caller logs — never panic, because a future
    /// app version may write kinds the current build doesn't know.
    pub fn from_str_lenient(s: &str) -> Self {
        match s {
            "big_move" => Self::BigMove,
            "allocation_drift" => Self::AllocationDrift,
            "goal_milestone" => Self::GoalMilestone,
            "cash_drag" => Self::CashDrag,
            "dividend_posted" => Self::DividendPosted,
            "new_ath" => Self::NewAth,
            "net_worth_dip" => Self::NetWorthDip,
            "sync_failure" => Self::SyncFailure,
            "ai_digest" => Self::AiDigest,
            _ => Self::BigMove,
        }
    }
}

/// How prominently to surface the row. Drives badge color + whether
/// the scheduler fires an OS notification (only `Warning` + `Critical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Critical,
}

impl NotificationSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s {
            "info" => Self::Info,
            "success" => Self::Success,
            "warning" => Self::Warning,
            "critical" => Self::Critical,
            _ => Self::Info,
        }
    }

    /// Does this severity warrant a native OS notification, or does it
    /// stay in-app only? Threshold is `Warning` so we don't annoy the
    /// user with positive news (Success/Info) in their notification
    /// center — they see those in the in-app bell on next launch.
    pub fn should_push_to_os(self) -> bool {
        matches!(self, Self::Warning | Self::Critical)
    }
}

/// A single notification row. Mirrors the SQLite `notifications` table
/// with millis-since-epoch timestamps for portable serialisation
/// across the Tauri IPC boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub kind: NotificationKind,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
    /// Free-form JSON the rule emits; deserialised by the AI digest
    /// synthesiser + (optionally) rendered as structured chips by
    /// the UI. Stays a JSON string at this layer so we don't bind
    /// to any one rule's shape.
    pub payload_json: String,
    /// Idempotency key — see migration header comment.
    pub dedupe_key: String,
    pub created_at_ms: i64,
    pub read_at_ms: Option<i64>,
    pub dismissed_at_ms: Option<i64>,
}

impl Notification {
    pub fn is_unread(&self) -> bool {
        self.read_at_ms.is_none() && self.dismissed_at_ms.is_none()
    }
}

/// Pagination wrapper for the bell-panel list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsPage {
    pub items: Vec<Notification>,
    pub unread_count: i64,
}
