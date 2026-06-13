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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_investment() {
        assert_eq!(AssetKind::default(), AssetKind::Investment);
    }

    #[test]
    fn serde_uses_screaming_snake_case() {
        // The wire format is contractual — the storage layer + the
        // mizan-core compat shim both round-trip these values, so any
        // accidental rename or case-style change here is a breaking
        // change for stored Holdings rows. Test the strings exactly.
        assert_eq!(
            serde_json::to_string(&AssetKind::Investment).expect("ok"),
            "\"INVESTMENT\""
        );
        assert_eq!(
            serde_json::to_string(&AssetKind::Property).expect("ok"),
            "\"PROPERTY\""
        );
        assert_eq!(
            serde_json::to_string(&AssetKind::PreciousMetal).expect("ok"),
            "\"PRECIOUS_METAL\""
        );
        assert_eq!(
            serde_json::to_string(&AssetKind::PrivateEquity).expect("ok"),
            "\"PRIVATE_EQUITY\""
        );
    }

    #[test]
    fn serde_roundtrips_every_variant() {
        for kind in [
            AssetKind::Investment,
            AssetKind::Property,
            AssetKind::Vehicle,
            AssetKind::Collectible,
            AssetKind::PreciousMetal,
            AssetKind::PrivateEquity,
            AssetKind::Liability,
            AssetKind::Other,
            AssetKind::Fx,
        ] {
            let json = serde_json::to_string(&kind).expect("encode");
            let back: AssetKind = serde_json::from_str(&json).expect("decode");
            assert_eq!(kind, back, "round-trip mismatch for {kind:?}");
        }
    }

    #[test]
    fn rejects_unknown_string() {
        let result: Result<AssetKind, _> = serde_json::from_str("\"WIDGETS\"");
        assert!(result.is_err());
    }

    #[test]
    fn clone_and_eq_work() {
        let a = AssetKind::PreciousMetal;
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(a, AssetKind::Investment);
    }
}
