//! Holdings domain types.
//!
//! `HoldingType` lifted from `mizan-core/src/portfolio/holdings/holdings_model.rs`
//! per ADR 0003. mizan-core re-exports as a temporary backward-compat shim.
//!
//! `HoldingsView` is NEW in this crate — a pure read-side struct that
//! downstream domain crates (zakat, insights, synthesis) accept as input
//! instead of pulling in `HoldingsServiceTrait` from mizan-core. The
//! desktop materialises a `Vec<HoldingsView>` via the existing service
//! and hands it to the pure-function APIs of the domain crates.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::AssetKind;

/// Per-position tag distinguishing cash from securities from alternative
/// (non-market-data) assets.
///
/// Alternative assets use MANUAL data source for valuations and are
/// excluded from TWR/IRR calculations.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HoldingType {
    Cash,
    Security,
    /// Property, Vehicle, Collectible, PhysicalPrecious, Liability, Other.
    AlternativeAsset,
}

/// Read-only data shape downstream domain crates (zakat, insights,
/// synthesis) accept as their compute input.
///
/// Carries exactly the fields a pure-function consumer needs to operate
/// on a holdings snapshot, without dragging the full service-trait
/// machinery from mizan-core.
///
/// The desktop produces these via `HoldingsServiceTrait::snapshot()` (in
/// mizan-core) and passes a `&[HoldingsView]` slice to e.g.
/// `mizan_zakat::compute(&holdings, inputs)`.
///
/// **No silent FX:** every monetary field carries an explicit `currency`
/// AND a `_base` variant that's already converted at the producer's
/// chosen FX-rate timestamp. Consumers that need to re-convert read
/// `currency` + their own timestamped `FxRate` source; this struct
/// never bakes in a default rate. Per working-agreement §0 rule 2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsView {
    /// Composite identity used by storage + Mizan Badge metadata
    /// keying. Matches `(account_id, holding_symbol, as_of_date)`.
    pub account_id: String,
    pub holding_symbol: String,
    pub as_of_date: DateTime<Utc>,

    /// Behavioral classification (Investment / Property / Vehicle / ...).
    pub asset_kind: AssetKind,

    /// Position tag (Cash / Security / AlternativeAsset).
    pub holding_type: HoldingType,

    /// Quantity held. Cash positions carry `qty = 1` and `currency =`
    /// the cash currency; `market_value_base` is the cash amount.
    pub qty: Decimal,

    /// Native-currency cost basis.
    pub cost_basis: Decimal,

    /// Native-currency current market value.
    pub market_value: Decimal,

    /// ISO 4217 currency code for `qty`/`cost_basis`/`market_value`.
    /// Wrapped in a `String` here for serde simplicity; a `Currency`
    /// newtype with ISO validation lands in a follow-up PR.
    pub currency: String,

    /// Base-currency cost basis (converted at the producer's FX
    /// timestamp). Always present; never silently defaulted.
    pub cost_basis_base: Decimal,

    /// Base-currency market value (converted at the producer's FX
    /// timestamp). Always present.
    pub market_value_base: Decimal,

    /// Sharia compliance status if screened; `None` if unrated /
    /// screening not yet attempted. Matches the `sharia_status` column
    /// in `holdings_metadata` (Track E PR-E1.a).
    pub sharia_status: Option<ShariaStatus>,
}

/// Sharia compliance verdict — mirrors the `holdings_metadata.sharia_status`
/// enum exactly (ADR 0011). Re-defined here so domain crates that read
/// the view don't need to depend on the storage schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShariaStatus {
    Compliant,
    NonCompliant,
    Mixed,
    Unrated,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    #[test]
    fn holding_type_serde_camel_case() {
        // camelCase contract is shared with the storage layer's enum
        // column — any change here breaks Holdings round-trips.
        assert_eq!(
            serde_json::to_string(&HoldingType::Cash).expect("ok"),
            "\"cash\""
        );
        assert_eq!(
            serde_json::to_string(&HoldingType::Security).expect("ok"),
            "\"security\""
        );
        assert_eq!(
            serde_json::to_string(&HoldingType::AlternativeAsset).expect("ok"),
            "\"alternativeAsset\""
        );
    }

    #[test]
    fn holding_type_roundtrips_every_variant() {
        for ty in [
            HoldingType::Cash,
            HoldingType::Security,
            HoldingType::AlternativeAsset,
        ] {
            let json = serde_json::to_string(&ty).expect("encode");
            let back: HoldingType = serde_json::from_str(&json).expect("decode");
            assert_eq!(ty, back, "round-trip mismatch for {ty:?}");
        }
    }

    #[test]
    fn sharia_status_serde_snake_case() {
        // snake_case is the contract with holdings_metadata.sharia_status.
        assert_eq!(
            serde_json::to_string(&ShariaStatus::Compliant).expect("ok"),
            "\"compliant\""
        );
        assert_eq!(
            serde_json::to_string(&ShariaStatus::NonCompliant).expect("ok"),
            "\"non_compliant\""
        );
        assert_eq!(
            serde_json::to_string(&ShariaStatus::Mixed).expect("ok"),
            "\"mixed\""
        );
        assert_eq!(
            serde_json::to_string(&ShariaStatus::Unrated).expect("ok"),
            "\"unrated\""
        );
    }

    #[test]
    fn sharia_status_roundtrips_every_variant() {
        for s in [
            ShariaStatus::Compliant,
            ShariaStatus::NonCompliant,
            ShariaStatus::Mixed,
            ShariaStatus::Unrated,
        ] {
            let json = serde_json::to_string(&s).expect("encode");
            let back: ShariaStatus = serde_json::from_str(&json).expect("decode");
            assert_eq!(s, back, "round-trip mismatch for {s:?}");
        }
    }

    #[test]
    fn holdings_view_construction_carries_all_required_fields() {
        // Sanity: the struct can be constructed with the documented
        // shape and serdes round-trip cleanly. The dual `_base` fields
        // are explicit per CLAUDE.md §0 rule 2 (no silent FX) — verify
        // they survive the round-trip.
        let view = HoldingsView {
            account_id: "acc_1".to_string(),
            holding_symbol: "AAPL".to_string(),
            as_of_date: chrono::Utc.with_ymd_and_hms(2026, 6, 13, 12, 0, 0).unwrap(),
            asset_kind: AssetKind::Investment,
            holding_type: HoldingType::Security,
            qty: dec!(10),
            cost_basis: dec!(1500),
            market_value: dec!(2000),
            currency: "USD".to_string(),
            cost_basis_base: dec!(1500),
            market_value_base: dec!(2000),
            sharia_status: Some(ShariaStatus::Compliant),
        };
        let json = serde_json::to_string(&view).expect("encode");
        // camelCase rename_all applied — confirm the wire keys exist
        // (the Decimal value formatting is governed by the
        // serde-with-str feature and isn't pinned by this test).
        assert!(json.contains("\"accountId\":\"acc_1\""));
        assert!(json.contains("\"holdingSymbol\":\"AAPL\""));
        assert!(json.contains("\"costBasisBase\""));
        assert!(json.contains("\"marketValueBase\""));
        assert!(json.contains("\"shariaStatus\":\"compliant\""));

        let back: HoldingsView = serde_json::from_str(&json).expect("decode");
        assert_eq!(view, back);
    }

    #[test]
    fn holdings_view_supports_unrated_sharia_status_via_none() {
        // Most positions land here — the unrated path must survive serde.
        let view = HoldingsView {
            account_id: "acc".into(),
            holding_symbol: "X".into(),
            as_of_date: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            asset_kind: AssetKind::Other,
            holding_type: HoldingType::AlternativeAsset,
            qty: dec!(1),
            cost_basis: dec!(0),
            market_value: dec!(0),
            currency: "USD".into(),
            cost_basis_base: dec!(0),
            market_value_base: dec!(0),
            sharia_status: None,
        };
        let json = serde_json::to_string(&view).expect("encode");
        assert!(json.contains("\"shariaStatus\":null"));
        let back: HoldingsView = serde_json::from_str(&json).expect("decode");
        assert_eq!(view, back);
        assert!(back.sharia_status.is_none());
    }

    #[test]
    fn holdings_view_clone_equality() {
        let view = HoldingsView {
            account_id: "a".into(),
            holding_symbol: "S".into(),
            as_of_date: chrono::Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
            asset_kind: AssetKind::Investment,
            holding_type: HoldingType::Security,
            qty: dec!(1),
            cost_basis: dec!(100),
            market_value: dec!(110),
            currency: "EUR".into(),
            cost_basis_base: dec!(108),
            market_value_base: dec!(119),
            sharia_status: Some(ShariaStatus::Mixed),
        };
        let cloned = view.clone();
        assert_eq!(view, cloned);
    }
}
