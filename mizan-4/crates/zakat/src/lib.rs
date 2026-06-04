//! Zakat (zakāh) calculation engine.
//!
//! Computes the Shariah-mandated 2.5% annual alms on a Muslim's wealth:
//!
//! ```text
//! net_base = liquid_cash + precious_metals_value + tradable_assets
//!            - short_term_debts
//! zakat_due = if net_base < nisab then 0 else 0.025 * net_base
//! ```
//!
//! `nisab` is the threshold below which no Zakat is owed; the caller
//! supplies it in the user's base currency (the local equivalent of
//! either 85 grams of gold or 595 grams of silver — whichever is lower
//! by majority Hanafi practice).
//!
//! **Not financial advice.** This crate reports an arithmetic result;
//! the user (or their scholar) decides what counts as Zakatable in their
//! jurisprudence. The Hanafi/Shafi'i rules ship today; Maliki + Hanbali
//! arrive in Track F per ADR 0012 + scholarly review by Uncle Ferox.
//!
//! ## Track H PR-H3.b note
//!
//! This is the **initial extraction** — moves the engine out of
//! `mizan-core/src/portfolio/zakat/` into its own crate so the 95%
//! coverage floor + mutation testing can be enforced at the crate
//! boundary (working-agreement §5 + §6). The API still accepts
//! `Arc<dyn mizan_core::portfolio::holdings::HoldingsServiceTrait>`,
//! creating a temporary mizan-zakat → mizan-core dep that violates
//! ADR 0003's leaf-crate goal. PR-H3.b.1 refactors the API to take
//! `&[HoldingsView]` slices (from mizan-domain-types) and drops the
//! mizan-core dep.

#![forbid(unsafe_code)]

mod model;
mod property_intent;
mod service;
mod traits;

pub use model::*;
pub use property_intent::{route_property, sum_property_tradable, PropertyBucket, PropertyIntent};
pub use service::*;
pub use traits::*;

/// Errors raised by the zakat engine. Per working-agreement §4.1, this
/// crate owns its own error type rather than passing through
/// `mizan_core::Error`. Today the engine bubbles up mizan-core errors
/// (e.g. from the HoldingsServiceTrait dep); PR-H3.b.1 replaces those
/// with `ZakatError::HoldingsUnavailable(String)` etc. once the API
/// takes a pre-materialised slice.
#[derive(Debug, thiserror::Error)]
pub enum ZakatError {
    /// Validation failure on the inputs (Nisab value, FX rate, etc.).
    #[error("invalid Zakat input: {0}")]
    InvalidInput(String),

    /// FX rate needed for cross-currency totals is not available at the
    /// requested timestamp. Working-agreement §0 rule 2 — no silent
    /// defaults; the engine surfaces this explicitly.
    #[error("missing FX rate for {pair} at {timestamp}")]
    MissingFxRate { pair: String, timestamp: String },

    /// Spot gold/silver prices (used to derive Nisab when caller doesn't
    /// pass an explicit value) are not available. Engine refuses to
    /// guess — Zakat correctness is religious correctness.
    #[error("Nisab data unavailable: {0}")]
    NisabUnavailable(String),

    /// Scholarly school selector wasn't recognised. Hanafi + Shafi'i
    /// today; Maliki + Hanbali after Track F scholarly sign-off.
    #[error("unknown scholarly school: {0}")]
    UnknownSchool(String),

    /// Holdings could not be loaded (transient or storage failure).
    /// PR-H3.b.1 will subsume this when the API moves to a slice.
    #[error("holdings unavailable: {0}")]
    HoldingsUnavailable(String),
}

/// Crate-local `Result` alias pinned to [`ZakatError`].
pub type Result<T> = std::result::Result<T, ZakatError>;
