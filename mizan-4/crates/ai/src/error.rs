//! AI assistant error types.

use mizan_core::{Error as CoreError, MizanError};
use thiserror::Error;

/// AI assistant errors.
#[derive(Debug, Error)]
pub enum AiError {
    /// Invalid input or request.
    #[error("{0}")]
    InvalidInput(String),

    /// Missing API key for a provider.
    #[error("Missing API key for provider {0}")]
    MissingApiKey(String),

    /// Provider error (from rig-core or API).
    #[error("Provider error: {0}")]
    Provider(String),

    /// Tool not found in registry.
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Tool not allowed for this thread.
    #[error("Tool not allowed: {0}")]
    ToolNotAllowed(String),

    /// Tool execution failed.
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// §A24 structured tool error — carries the full diagnostic shape
    /// (code, why, data-safety, next-steps, retry) so the assistant UI
    /// can render the same toast as the rest of the app. Display falls
    /// back to a one-line summary for legacy text contexts.
    #[error("{0}")]
    Structured(#[from] MizanError),

    /// Thread not found.
    #[error("Thread not found: {0}")]
    ThreadNotFound(String),

    /// Invalid cursor for pagination.
    #[error("Invalid cursor: {0}")]
    InvalidCursor(String),

    /// Core error from mizan-core.
    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AiError {
    /// Create a new invalid input error.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a new provider error.
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }

    /// Create a new internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Wrap any lower-layer error in a structured MizanError. Convenience
    /// for tool sites that want to surface a clean toast instead of a
    /// raw provider message.
    pub fn structured(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::Structured(MizanError::new(code, summary))
    }

    /// Convert the error to a serialised wire shape the frontend can
    /// decode. For Structured variants this is the JSON the frontend's
    /// `parseMizanError` understands; for legacy variants it's the
    /// existing free-form text (also accepted by `parseMizanError` via
    /// its legacy fallback).
    pub fn to_wire(&self) -> String {
        match self {
            AiError::Structured(e) => e.to_command_error(),
            _ => self.to_string(),
        }
    }
}

/// Error code for programmatic handling in stream events.
impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            AiError::InvalidInput(_) => "INVALID_INPUT",
            AiError::MissingApiKey(_) => "MISSING_API_KEY",
            AiError::Provider(_) => "PROVIDER_ERROR",
            AiError::ToolNotFound(_) => "TOOL_NOT_FOUND",
            AiError::ToolNotAllowed(_) => "TOOL_NOT_ALLOWED",
            AiError::ToolExecutionFailed(_) => "TOOL_EXECUTION_FAILED",
            AiError::Structured(_) => "STRUCTURED_TOOL_ERROR",
            AiError::ThreadNotFound(_) => "THREAD_NOT_FOUND",
            AiError::InvalidCursor(_) => "INVALID_CURSOR",
            AiError::Core(_) => "CORE_ERROR",
            AiError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mizan_core::{DataSafetyStatus, RetryPolicy};

    #[test]
    fn structured_wire_carries_mizan_error_shape() {
        let err = AiError::Structured(
            MizanError::new("AI_TOOL_QUOTA", "Daily AI usage cap reached.")
                .why("Free tier allows 0 managed-AI credits per day.")
                .data_safety(DataSafetyStatus::Untouched)
                .next_step("Upgrade to Silver, or use your own OpenAI/Anthropic key.")
                .retry(RetryPolicy::RetryAfterPrereq),
        );
        let wire = err.to_wire();
        let parsed: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(parsed["__mizan_error"], true);
        assert_eq!(parsed["code"], "AI_TOOL_QUOTA");
        assert_eq!(parsed["retry"], "retry_after_prereq");
    }

    #[test]
    fn legacy_wire_is_plain_text() {
        let err = AiError::ToolExecutionFailed("boom".to_string());
        let wire = err.to_wire();
        assert!(wire.contains("boom"));
        // Not JSON-shaped — the frontend decoder's legacy fallback handles it.
        assert!(!wire.starts_with("{"));
    }
}
