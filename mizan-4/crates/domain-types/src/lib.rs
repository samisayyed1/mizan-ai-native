//! Mizan domain types — leaf crate of cross-cutting data types every
//! domain crate (financial-truth, zakat, insights, synthesis, csv-import)
//! shares. Per ADR 0003, this crate carries **data types only — no
//! service traits, no I/O, no behaviour**.
//!
//! ## Public surface
//!
//! - [`AssetKind`] — behavioral classification of holdings (Investment, Property, ...)
//! - [`HoldingType`] — per-position tag (Cash, Security, AlternativeAsset)
//! - [`HoldingsView`] — read-only data shape downstream crates accept as input
//!
//! ## Scope discipline
//!
//! - Data structs: yes (with `#[derive(Debug, Clone, Serialize, Deserialize)]`)
//! - Pure helper free functions: yes (e.g. classification predicates)
//! - Service traits: NO — those live in the crate that owns the behaviour
//! - I/O: NO — leaf crate, never touches storage or network
//! - "Utilities" dumping ground: NO — each domain crate has its own `utils.rs`

#![forbid(unsafe_code)]

pub mod assets;
pub mod holdings;

pub use assets::AssetKind;
pub use holdings::{HoldingType, HoldingsView};

/// Errors raised by domain-types validation (e.g. malformed currency codes,
/// invalid lunar-year dates). Most types in this crate are inert data with
/// no fallible constructors yet; this enum is the seed for future
/// validation variants.
#[derive(Debug, thiserror::Error)]
pub enum DomainTypesError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_invalid_input_renders_via_display() {
        let e = DomainTypesError::InvalidInput("malformed currency code".to_string());
        assert_eq!(format!("{e}"), "invalid input: malformed currency code");
    }

    #[test]
    fn re_exports_are_reachable() {
        // Compile-time check that the public surface stays callable
        // from outside the module. If a future refactor accidentally
        // drops one of these re-exports, this fails to build.
        let _kind: AssetKind = AssetKind::default();
        let _ty: HoldingType = HoldingType::Cash;
        let _ = std::mem::size_of::<HoldingsView>();
    }
}
