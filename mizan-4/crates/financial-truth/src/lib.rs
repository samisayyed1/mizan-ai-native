//! Mizan Financial Truth — immutable hash-chained ledger.
//!
//! Per `docs/working-agreement.md` §0 rule 1 (Truth Ledger sanctity), this
//! crate is the ONLY home for the hash-chained immutable financial-state
//! ledger that backs every "what's my X?" answer in Mizan. Holdings,
//! balances, performance, net worth — all derived from the ledger, never
//! from mutable cells.
//!
//! ## Crate scope (PR-H3.a)
//!
//! This first slice extracts the Truth Engine itself (`model` + `service`)
//! out of `mizan-core` per Track H PR-H3.a. The FIFO cost-basis / TWR /
//! IRR / realized P&L computations that live in `mizan-core/src/portfolio`
//! follow in PR-H3.a.2 once consumers of the truth engine settle.
//!
//! ## Public surface
//!
//! - [`LedgerEntry`], [`LedgerEntryKind`], [`LedgerIntegrityError`] —
//!   model types
//! - [`TruthLedger`], [`TruthLedgerRetryQueue`] — async traits storage
//!   adapters implement
//! - [`AppendInput`] — the only way to write a ledger entry
//! - [`canonical_payload`], [`derive_entry_hash`], [`GENESIS_PREV_HASH`] —
//!   primitives the storage adapters use to compute entry hashes
//! - [`InMemoryTruthLedger`] — gated behind the `test-utils` feature;
//!   used by every test path that wires a fake ledger
//!
//! ## Error semantics
//!
//! [`FinancialTruthError`] is the single error type the crate's
//! fallible APIs return. Variants are domain-shaped — the crate never
//! leaks `serde_json::Error` or other dependency types into its public
//! surface (per the working agreement's "each crate owns its types"
//! principle). The internal `#[from]` impl on `Serde` lets the crate's
//! own code use the `?` operator without ceremony.
//!
//! ## Coverage floor
//!
//! Per `docs/working-agreement.md` §6 + §5, this crate has a 95% line +
//! branch coverage floor. Mutation testing runs nightly via
//! `cargo mutants` — surviving mutants in financial-truth or zakat
//! are P0 fix items.

#![forbid(unsafe_code)]

mod model;
mod service;
mod zakat_helper;

pub use model::{
    canonical_payload, derive_entry_hash, LedgerEntry, LedgerEntryKind, LedgerIntegrityError,
    GENESIS_PREV_HASH,
};
pub use service::{AppendInput, TruthLedger, TruthLedgerRetryQueue};
pub use zakat_helper::{build_zakat_append_input, ZakatLedgerInputs};

#[cfg(any(test, feature = "test-utils"))]
pub use service::InMemoryTruthLedger;

/// Errors raised by the financial-truth crate's fallible APIs.
///
/// The crate never leaks third-party error types in its public surface.
/// Internal `From` impls let the crate's own code use `?` on
/// `serde_json::Error` and similar; consumers see only this enum.
#[derive(Debug, thiserror::Error)]
pub enum FinancialTruthError {
    /// The ledger chain itself is internally inconsistent — e.g. a
    /// `prev_hash` doesn't match the previous entry's `entry_hash`.
    /// Carries a structured pointer to the entry id at which the chain
    /// broke; see [`LedgerIntegrityError`] for the variants.
    #[error("ledger integrity violation: {0}")]
    LedgerIntegrity(#[from] LedgerIntegrityError),

    /// Canonicalising an entry's payload failed at the JSON layer.
    /// We wrap the serde_json error as a `String` so the public type
    /// surface stays dependency-free (working-agreement §4.1: each
    /// crate owns its types).
    ///
    /// The internal `From<serde_json::Error>` conversion calls
    /// `.to_string()` on the source, so call sites can use the `?`
    /// operator without explicit boilerplate.
    #[error("ledger payload serialisation failed: {0}")]
    SerializationFailed(String),

    /// Input to `append` failed validation before any hash work — e.g.
    /// an empty `kind` discriminator or a malformed actor id.
    #[error("invalid append input: {0}")]
    InvalidInput(String),

    /// The retry queue (used when persistent storage rejects a write)
    /// has reached its capacity. The caller should surface this to the
    /// user as a transient failure and re-attempt later; the queue
    /// drains on the next sweep.
    #[error("ledger retry queue is at capacity")]
    RetryQueueFull,
}

/// Internal-only conversion. Consumers see `SerializationFailed(String)`;
/// the crate's own code uses `?` on `serde_json::Error` and rides this
/// `From` impl. The string preserves the original error's message.
impl From<serde_json::Error> for FinancialTruthError {
    fn from(err: serde_json::Error) -> Self {
        FinancialTruthError::SerializationFailed(err.to_string())
    }
}

/// Crate-local `Result` alias. Identical shape to `std::result::Result`
/// with the error type pinned to [`FinancialTruthError`].
pub type Result<T> = std::result::Result<T, FinancialTruthError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_json_error_converts_to_serialization_failed() {
        // The crate's public surface promises `SerializationFailed(String)`
        // and never leaks a third-party error type. The internal `From`
        // impl is what lets the rest of the crate use `?` on
        // serde_json::Error — pin its behaviour here so a future refactor
        // doesn't accidentally swap to `From<serde_json::Error>` of a
        // different variant.
        //
        // Trigger a real serde_json::Error by parsing malformed JSON.
        let err = serde_json::from_str::<serde_json::Value>("{ this is not json }")
            .expect_err("malformed");
        let converted: FinancialTruthError = err.into();
        match converted {
            FinancialTruthError::SerializationFailed(msg) => {
                assert!(!msg.is_empty(), "preserves source message");
            }
            other => panic!("expected SerializationFailed, got {other:?}"),
        }
    }

    #[test]
    fn error_variants_display_correctly() {
        // The Display impls go on the wire (tracing logs, user-facing
        // error toasts). Each variant's format string is contractual.
        assert_eq!(
            format!(
                "{}",
                FinancialTruthError::InvalidInput("bad input".to_string())
            ),
            "invalid append input: bad input"
        );
        assert_eq!(
            format!(
                "{}",
                FinancialTruthError::SerializationFailed("encode failed".to_string())
            ),
            "ledger payload serialisation failed: encode failed"
        );
        assert_eq!(
            format!("{}", FinancialTruthError::RetryQueueFull),
            "ledger retry queue is at capacity"
        );
    }
}
