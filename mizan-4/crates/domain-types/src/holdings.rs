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
