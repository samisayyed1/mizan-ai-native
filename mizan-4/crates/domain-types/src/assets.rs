//! Asset domain classification.
//!
//! Lifted from `mizan-core/src/assets/assets_model.rs` per ADR 0003.
//! mizan-core re-exports `AssetKind` from here as a temporary
//! backward-compat shim; that shim is removed in PR-H3.b (zakat
//! extraction) when the first downstream consumer starts importing
//! directly from `mizan_domain_types::AssetKind`.

use serde::{Deserialize, Serialize};

/// Asset behavior classification.
///
/// `kind` is a behavioral category — broad for market instruments, granular
/// for alternatives. Market instruments are all `INVESTMENT`; the
/// `instrument_type` field on the owning `Asset` struct (in mizan-core)
/// carries the market-specific classification (EQUITY, CRYPTO, OPTION, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetKind {
    /// All tradable, lot-tracked market instruments (stocks, ETFs, crypto, options).
    #[default]
    Investment,
    /// Real estate.
    Property,
    /// Cars, motorcycles, boats, RVs.
    Vehicle,
    /// Art, wine, watches, jewelry, memorabilia.
    Collectible,
    /// Physical gold/silver bars, coins (not ETFs).
    PreciousMetal,
    /// Private shares, startup equity.
    PrivateEquity,
    /// Debts (mortgages, loans, credit cards).
    Liability,
    /// Catch-all for uncategorized assets.
    Other,
    /// Currency exchange rate (infrastructure, not holdable).
    Fx,
}
