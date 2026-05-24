//! Structured Mizan error type — the "no silent failure" contract.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A24 "Error Message Standard":
//! every user-visible error must answer six questions:
//!   1. What failed?
//!   2. Why did it likely fail?
//!   3. Is user data safe?
//!   4. What can the user do next?
//!   5. Can the action be retried (and how)?
//!   6. What's the diagnostic code support can grep?
//!
//! Bad:  "Something went wrong."
//! Good: "SnapTrade sync failed because Schwab authorization expired.
//!        Your existing Mizan data is safe. Reconnect Schwab to refresh
//!        positions. [code: SNAPTRADE_AUTH_EXPIRED]"
//!
//! Tauri commands return `Result<T, String>`. To carry the structured
//! shape across that boundary we serialise [`MizanError`] to a JSON
//! string with a `__mizan_error: true` discriminator, and the frontend
//! decoder ([`apps/frontend/src/lib/mizan-error.ts`]) recognises that
//! shape and renders the right UI.

use serde::{Deserialize, Serialize};

/// Severity buckets the UI uses to colour-code toasts / banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MizanErrorSeverity {
    /// The action failed but the system continues unaffected — retry is
    /// usually safe (transient network / provider hiccup).
    Warning,
    /// The action failed and may have left some intermediate state behind
    /// that the user should reconcile, but no data was lost.
    Error,
    /// User-data-loss or data-corruption risk. Bright red toast + persistent
    /// banner. Rare — most errors should be `Warning` or `Error`.
    Critical,
}

/// Whether the user's existing data was modified by the failed operation.
/// Drives the "Your Mizan data is safe." line in the toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSafetyStatus {
    /// No writes happened. Existing data unchanged.
    Untouched,
    /// Partial writes happened but were rolled back inside a transaction.
    /// Effectively `Untouched` from the user's perspective.
    RolledBack,
    /// Partial writes were committed before the failure. The user should
    /// inspect what landed before retrying.
    PartiallyApplied,
}

/// Whether a retry button makes sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryPolicy {
    /// Same args, same outcome — don't bother showing a retry button.
    NotRetryable,
    /// Same call may succeed (transient failure). Show "Try again".
    Retryable,
    /// Retry requires a different prerequisite first (e.g. reconnect Plaid
    /// before re-running the sync). The `next_steps` lines describe it.
    RetryAfterPrereq,
}

/// Structured error every Tauri command can return.
///
/// Construct via [`MizanError::new`] or the [`MizanError`] builder methods.
/// Convert to the `String` Tauri expects via [`MizanError::to_command_error`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MizanError {
    /// Always `true`. The frontend decoder checks this discriminator.
    #[serde(rename = "__mizan_error")]
    pub _discriminator: bool,
    /// Stable diagnostic code (SCREAMING_SNAKE_CASE). Example:
    /// `"CSV_PARSE_FAILED"`, `"PLAID_AUTH_EXPIRED"`. Used by support and
    /// telemetry; never localised.
    pub code: String,
    pub severity: MizanErrorSeverity,
    /// Short headline shown in the toast title. ~6 words. User-facing.
    pub summary: String,
    /// One-sentence explanation of the most likely cause. Optional — omit
    /// when the cause is genuinely unknown (don't fabricate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    pub data_safety: DataSafetyStatus,
    /// 1-3 short, actionable steps the user can take next.
    pub next_steps: Vec<String>,
    pub retry: RetryPolicy,
    /// Optional raw error from a lower layer — included in support bundles
    /// but NEVER shown directly in the UI. Use this to capture stack traces,
    /// provider messages, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

impl MizanError {
    pub fn new(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            _discriminator: true,
            code: code.into(),
            severity: MizanErrorSeverity::Error,
            summary: summary.into(),
            why: None,
            data_safety: DataSafetyStatus::Untouched,
            next_steps: Vec::new(),
            retry: RetryPolicy::Retryable,
            raw: None,
        }
    }

    pub fn severity(mut self, s: MizanErrorSeverity) -> Self {
        self.severity = s;
        self
    }

    pub fn why(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }

    pub fn data_safety(mut self, s: DataSafetyStatus) -> Self {
        self.data_safety = s;
        self
    }

    pub fn next_step(mut self, step: impl Into<String>) -> Self {
        self.next_steps.push(step.into());
        self
    }

    pub fn next_steps(mut self, steps: Vec<String>) -> Self {
        self.next_steps = steps;
        self
    }

    pub fn retry(mut self, r: RetryPolicy) -> Self {
        self.retry = r;
        self
    }

    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.raw = Some(raw.into());
        self
    }

    /// Render to the `String` shape Tauri commands return. Falls back to a
    /// plain summary if JSON serialisation somehow fails (it shouldn't,
    /// the struct is pure data).
    pub fn to_command_error(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.summary.clone())
    }
}

impl std::fmt::Display for MizanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.summary)
    }
}

impl std::error::Error for MizanError {}

/// Convert a generic error into a structured MizanError, attaching the raw
/// message so support bundles can grep for the original wording.
pub fn wrap<E: std::fmt::Display>(
    code: impl Into<String>,
    summary: impl Into<String>,
    source: E,
) -> MizanError {
    MizanError::new(code, summary).raw(source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_to_string_with_discriminator() {
        let err = MizanError::new("CSV_PARSE_FAILED", "Couldn't parse the CSV.")
            .why("The first row has more columns than the header.")
            .data_safety(DataSafetyStatus::Untouched)
            .next_step("Open the file in Excel and check the header row.")
            .next_step("Re-export from your broker if the file looks corrupted.")
            .retry(RetryPolicy::Retryable);

        let s = err.to_command_error();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["__mizan_error"], true);
        assert_eq!(parsed["code"], "CSV_PARSE_FAILED");
        assert_eq!(parsed["severity"], "error");
        assert_eq!(parsed["dataSafety"], "untouched");
        assert_eq!(parsed["retry"], "retryable");
        assert_eq!(parsed["nextSteps"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn omits_optional_fields() {
        let err = MizanError::new("X", "Y");
        let s = err.to_command_error();
        assert!(!s.contains("\"why\""));
        assert!(!s.contains("\"raw\""));
    }

    #[test]
    fn wrap_includes_raw() {
        let err = wrap("X", "Y", std::io::Error::other("boom"));
        let s = err.to_command_error();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["raw"], "boom");
    }
}
