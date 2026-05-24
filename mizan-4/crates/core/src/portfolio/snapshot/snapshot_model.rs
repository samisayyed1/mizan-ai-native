//! Portfolio snapshot domain models.

use chrono::{NaiveDate, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use uuid::Uuid;

use super::Position;

/// Source of a snapshot - how it was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotSource {
    /// Calculated from transaction history
    #[default]
    Calculated,
    /// Manually entered by user
    ManualEntry,
    /// Imported from broker connection
    BrokerImported,
    /// Imported from CSV file
    CsvImport,
    /// Synthetic backfill snapshot (cloned from earliest for history)
    Synthetic,
}

impl SnapshotSource {
    /// Returns true if this is a non-calculated source (manual, broker, csv, or synthetic).
    pub fn is_non_calculated(&self) -> bool {
        !matches!(self, SnapshotSource::Calculated)
    }
}

/// Represents a warning that occurred during holdings calculation.
/// These are non-fatal issues where calculation continued but with potential data quality concerns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsCalculationWarning {
    pub activity_id: String,
    pub account_id: String,
    pub date: NaiveDate,
    pub message: String,
}

impl std::fmt::Display for HoldingsCalculationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Activity {} (account: {}, date: {}): {}",
            self.activity_id, self.account_id, self.date, self.message
        )
    }
}

/// Result of holdings calculation containing both the snapshot and any warnings.
/// The snapshot is always returned (even on partial failures), but warnings indicate
/// which activities could not be processed correctly.
#[derive(Debug, Clone)]
pub struct HoldingsCalculationResult {
    pub snapshot: AccountStateSnapshot,
    pub warnings: Vec<HoldingsCalculationWarning>,
}

impl HoldingsCalculationResult {
    pub fn new(snapshot: AccountStateSnapshot) -> Self {
        Self {
            snapshot,
            warnings: Vec::new(),
        }
    }

    pub fn with_warnings(
        snapshot: AccountStateSnapshot,
        warnings: Vec<HoldingsCalculationWarning>,
    ) -> Self {
        Self { snapshot, warnings }
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Lifetime realized-P&L accumulator for a single asset within an account.
///
/// Built up by every SELL (and disposal-like activity) the calculator has
/// processed up to the snapshot date. We track gross numbers separately
/// (proceeds, cost-basis-of-sold-lots, sell-side fees) so downstream views
/// can compute gain/loss in either currency without going back to the
/// activity stream — and so an annual-realized-gains report can read this
/// entry directly.
///
/// All Decimal fields are signed totals in the named currency. `_account_ccy`
/// is the account's reporting currency at the moment of each sell (FX
/// converted at trade-date); `_base_ccy` is the portfolio's base currency,
/// also at trade-date FX. Carry-forward across daily snapshots is
/// automatic via `AccountStateSnapshot::clone()` in the calculator.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RealizedGainEntry {
    /// Gross proceeds (qty × price × multiplier) — sell-side fee NOT yet deducted.
    pub proceeds_account_ccy: Decimal,
    pub proceeds_base_ccy: Decimal,
    /// FIFO cost basis of the lots removed by this asset's sells.
    pub cost_basis_account_ccy: Decimal,
    pub cost_basis_base_ccy: Decimal,
    /// Total sell-side fees, summed across all sells for this asset.
    pub fees_account_ccy: Decimal,
    pub fees_base_ccy: Decimal,
    /// Total quantity disposed across all sells for this asset.
    pub quantity_sold: Decimal,
    /// Most recent sale date — useful for "last sold" display and YTD reports.
    pub last_sale_date: Option<NaiveDate>,
    /// Lifetime gross dividend income booked for this asset (each DIVIDEND
    /// activity's amount, FX-converted at its pay date). Tracked here so a
    /// holding can show "total dividend income" alongside realized capital
    /// gains. It is income, NOT disposal P&L, so it is intentionally kept
    /// separate from `realized_gain_*` and never folded into it.
    ///
    /// Defaulted so realized-gain entries written before dividends were
    /// tracked still deserialize cleanly.
    #[serde(default)]
    pub dividend_income_account_ccy: Decimal,
    #[serde(default)]
    pub dividend_income_base_ccy: Decimal,
}

impl RealizedGainEntry {
    /// Net realized gain in account currency: proceeds − cost basis − fees.
    pub fn realized_gain_account_ccy(&self) -> Decimal {
        self.proceeds_account_ccy - self.cost_basis_account_ccy - self.fees_account_ccy
    }
    /// Net realized gain in base currency.
    pub fn realized_gain_base_ccy(&self) -> Decimal {
        self.proceeds_base_ccy - self.cost_basis_base_ccy - self.fees_base_ccy
    }
}

/// Represents the comprehensive state of an account at the close of a specific day.
/// This becomes the primary data structure stored and retrieved by the ValuationRepository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountStateSnapshot {
    pub id: String, // Stable UUID derived from (account_id, snapshot_date)
    pub account_id: String,
    pub snapshot_date: NaiveDate,
    pub currency: String, // Account's reporting currency

    // --- Core State ---
    // Use the detailed Position struct for accuracy, including lots.
    #[serde(default)]
    pub positions: HashMap<String, Position>, // asset_id -> Position (holds quantity, lots, cost basis info)

    #[serde(default)]
    pub cash_balances: HashMap<String, Decimal>, // currency -> amount

    // --- Calculated Aggregates (Account Currency) ---
    #[serde(default)]
    pub cost_basis: Decimal, // Sum of cost basis of all positions
    #[serde(default)]
    pub net_contribution: Decimal, // Cumulative net deposits in account currency
    #[serde(default)]
    pub net_contribution_base: Decimal, // portfolio base currency

    /// Cached total of all cash balances converted to account currency.
    /// Computed once at end of daily holdings calculation.
    #[serde(default)]
    pub cash_total_account_currency: Decimal,

    /// Cached total of all cash balances converted to base currency.
    /// Computed once at end of daily holdings calculation.
    #[serde(default)]
    pub cash_total_base_currency: Decimal,

    /// Lifetime realized P&L accumulator, keyed by asset_id. Each SELL the
    /// calculator processes adds to the matching entry (creating one if
    /// the position is being touched for the first time). Carries forward
    /// across daily snapshots automatically; never decreases.
    ///
    /// Defaulted so historical snapshots without this field still
    /// deserialize cleanly — they re-acquire data on the next recalc.
    #[serde(default)]
    pub realized_gains: HashMap<String, RealizedGainEntry>,

    pub calculated_at: NaiveDateTime, // When this snapshot was generated

    /// Source of this snapshot (how it was created)
    #[serde(default)]
    pub source: SnapshotSource,
}

impl Default for AccountStateSnapshot {
    fn default() -> Self {
        AccountStateSnapshot {
            id: String::new(),
            account_id: String::new(),
            snapshot_date: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            currency: String::new(),
            positions: HashMap::new(),
            cash_balances: HashMap::new(),
            cost_basis: Decimal::ZERO,
            net_contribution: Decimal::ZERO,
            net_contribution_base: Decimal::ZERO,
            cash_total_account_currency: Decimal::ZERO,
            cash_total_base_currency: Decimal::ZERO,
            realized_gains: HashMap::new(),
            calculated_at: Utc::now().naive_utc(),
            source: SnapshotSource::default(),
        }
    }
}

impl AccountStateSnapshot {
    /// Returns a stable UUID for a snapshot identity (account_id + snapshot_date).
    pub fn stable_id(account_id: &str, snapshot_date: NaiveDate) -> String {
        // Derive a deterministic UUID from account/date so upserts target the same row.
        let name = format!(
            "mizan:snapshot:{}:{}",
            account_id,
            snapshot_date.format("%Y-%m-%d")
        );
        let digest = sha2::Sha256::digest(name.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        // RFC4122 variant + version 5 bit layout.
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    }

    /// Compares the core content of two snapshots (positions and cash_balances).
    /// Returns true if the holdings are effectively the same, ignoring metadata
    /// like id, snapshot_date, calculated_at, source, etc.
    ///
    /// For positions, compares: asset_id, quantity, average_cost, total_cost_basis.
    /// Ignores: lots, timestamps, inception_date.
    pub fn is_content_equal(&self, other: &Self) -> bool {
        // Compare cash balances
        if self.cash_balances != other.cash_balances {
            return false;
        }

        // Compare positions count
        if self.positions.len() != other.positions.len() {
            return false;
        }

        // Compare each position by key fields
        for (asset_id, pos) in &self.positions {
            match other.positions.get(asset_id) {
                None => return false,
                Some(other_pos) => {
                    if !Self::positions_equal(pos, other_pos) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Compares two positions by their essential financial fields.
    /// Ignores lots, timestamps, and other metadata.
    fn positions_equal(a: &Position, b: &Position) -> bool {
        a.asset_id == b.asset_id
            && a.quantity == b.quantity
            && a.average_cost == b.average_cost
            && a.total_cost_basis == b.total_cost_basis
            && a.currency == b.currency
    }
}
