//! Mutating operations emitted by [`crate::plan`]. Consumers
//! (mizan-core persistence, a Tauri command, a CLI) apply them in
//! order against the local store. Every variant is data — no
//! callbacks, no IO — so the planner stays a pure function and the
//! consumer can dry-run a preview before committing.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::FxSnapshot;

/// One mutating step against the local store. Variants are kept
/// "thick" (carrying all the seed fields) so a consumer that maps to
/// the existing service layer never needs to re-read the seed.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    /// Owner profile — base currency, school, as-of date. Idempotent
    /// upsert keyed on installation.
    SetOwnerProfile {
        name: String,
        base_currency: String,
        secondary_currency: Option<String>,
        school: String,
        as_of: String,
    },

    /// Pin the FX-rates snapshot to the seed's business date. Every
    /// USD conversion downstream pulls from this snapshot instead of
    /// the live FX cache, so a re-import of the same seed is bit-for-
    /// bit reproducible.
    SetFxSnapshot(FxSnapshot),

    CreateSukuk {
        isin: String,
        issuer: String,
        custodian: String,
        issue_date: String,
        maturity: String,
        purchase_value_usd: Decimal,
        quantity: Decimal,
        purchase_price: Decimal,
        current_price: Decimal,
        coupon_rate: Decimal,
        current_value_usd: Decimal,
    },

    /// Aggregate equity subset (e.g. "US Stocks", "SG Stocks") — the
    /// seed doesn't enumerate the underlying tickers, so we create
    /// one rollup tile per subset that the user can later drill into
    /// by adding individual positions.
    CreateEquitySubset {
        label: String,
        current_value_usd: Decimal,
        native_currency_value: Option<Decimal>,
        data_source: Option<String>,
    },

    CreateEtf {
        ticker: String,
        broker: String,
        qty: Decimal,
        current_price: Decimal,
        purchase_price: Decimal,
        current_value_usd: Decimal,
    },

    CreatePrivateEquity {
        vehicle: String,
        currency: String,
        amount_native: Decimal,
        usd_value: Decimal,
        date: Option<String>,
        /// PE quantity is sometimes a number, sometimes a string
        /// ("1.5% equity", "4 properties") — store the verbatim
        /// JSON-string form and let the surface render it.
        quantity_note: String,
        remarks: Option<String>,
    },

    CreateUnitTrust {
        broker: String,
        wrapper: String,
        fund: String,
        qty: Decimal,
        avg_cost: Decimal,
        current_nav: Decimal,
        investment_usd: Decimal,
        current_value_usd: Decimal,
    },

    CreateUnitTrustTranche {
        broker: String,
        wrapper: String,
        date: String,
        investment_native: Decimal,
        native_currency: String,
        current_native: Decimal,
        funds: Vec<String>,
    },

    /// Placeholder for SRS holdings the seed names but doesn't value.
    /// The surface should render these as "Add valuation" tiles
    /// rather than silently materialising at zero.
    CreateUnitTrustPlaceholder {
        broker: String,
        wrapper: String,
        fund: String,
    },

    CreateUlipPolicy {
        company: String,
        holder_name: String,
        policy_number: u64,
        start_date: String,
        maturity: String,
        term_years: u32,
        installment_native: Decimal,
        mode: String,
        invested_native: Decimal,
        fund_value_native: Option<Decimal>,
        currency: String,
    },

    CreateProvidentFundBalance {
        country: String,
        total_usd: Decimal,
        note: Option<String>,
    },

    CreateCashAccount {
        country: String,
        bank: String,
        currency: String,
        balance_native: Decimal,
        balance_usd_equiv: Decimal,
    },

    CreateRealEstateProperty {
        country: String,
        name: String,
        kind: String,
        sqft: Decimal,
        current_value_usd: Option<Decimal>,
        intent: String,
        year_built: Option<u32>,
    },
}

/// Lightweight discriminator used for summary counts in the importer
/// preview UI (e.g. "10 sukuks, 4 ETFs, 12 cash accounts").
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationKind {
    SetOwnerProfile,
    SetFxSnapshot,
    CreateSukuk,
    CreateEquitySubset,
    CreateEtf,
    CreatePrivateEquity,
    CreateUnitTrust,
    CreateUnitTrustTranche,
    CreateUnitTrustPlaceholder,
    CreateUlipPolicy,
    CreateProvidentFundBalance,
    CreateCashAccount,
    CreateRealEstateProperty,
}

impl Operation {
    pub fn kind(&self) -> OperationKind {
        match self {
            Operation::SetOwnerProfile { .. } => OperationKind::SetOwnerProfile,
            Operation::SetFxSnapshot(_) => OperationKind::SetFxSnapshot,
            Operation::CreateSukuk { .. } => OperationKind::CreateSukuk,
            Operation::CreateEquitySubset { .. } => OperationKind::CreateEquitySubset,
            Operation::CreateEtf { .. } => OperationKind::CreateEtf,
            Operation::CreatePrivateEquity { .. } => OperationKind::CreatePrivateEquity,
            Operation::CreateUnitTrust { .. } => OperationKind::CreateUnitTrust,
            Operation::CreateUnitTrustTranche { .. } => OperationKind::CreateUnitTrustTranche,
            Operation::CreateUnitTrustPlaceholder { .. } => {
                OperationKind::CreateUnitTrustPlaceholder
            }
            Operation::CreateUlipPolicy { .. } => OperationKind::CreateUlipPolicy,
            Operation::CreateProvidentFundBalance { .. } => {
                OperationKind::CreateProvidentFundBalance
            }
            Operation::CreateCashAccount { .. } => OperationKind::CreateCashAccount,
            Operation::CreateRealEstateProperty { .. } => OperationKind::CreateRealEstateProperty,
        }
    }
}

/// Bucket the operations by kind for a preview summary.
pub fn summarise(ops: &[Operation]) -> std::collections::BTreeMap<OperationKind, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for op in ops {
        *counts.entry(op.kind()).or_insert(0) += 1;
    }
    counts
}
