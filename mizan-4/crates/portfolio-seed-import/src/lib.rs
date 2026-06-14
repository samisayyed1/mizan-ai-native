//! Portfolio seed JSON importer (Track Demo — PR-DEMO-AI-2 FLOW A).
//!
//! Parses a hand-curated JSON portfolio seed (the shape Sami uses for
//! Uncle Feroz's $1.71M portfolio) into a stream of typed
//! [`Operation`]s that downstream consumers (mizan-core's portfolio
//! service, or a Tauri command for one-shot demo seeding) can apply
//! against the local SQLite store.
//!
//! # Crate boundary (Track H pattern)
//!
//! This is a **true leaf** — only depends on `serde`, `serde_json`,
//! `thiserror`, and `rust_decimal`. No mizan-core or mizan-domain-types
//! dep, so it stays cheap to test and the persistence layer can pick
//! it up via a `From<Operation>` shim without forming a cycle.
//!
//! # FX rates
//!
//! The seed carries an FX-rates table (`owner.fx_rates`). The parser
//! emits the FX rates as a separate [`FxSnapshot`] so the importer can
//! pin them to a single business date — never re-deriving them from
//! the per-holding `current_value` (which would silently round-trip
//! through float).
//!
//! # Float vs Decimal
//!
//! Source values in the seed JSON are floats — the upstream XLSX
//! already lost precision. We read them via `rust_decimal::serde::float`
//! and emit `Decimal` downstream so the rest of the pipeline stays in
//! the money-correct path. The lossiness is bounded at the seed
//! boundary and explicit — see [`SeedImportError::InvalidNumber`] for
//! the failure mode when the float won't fit in a Decimal.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod operations;

pub use operations::Operation;

/// Top-level seed file. Mirrors the JSON shape Sami's
/// `uncle_portfolio_seed.json` uses verbatim, with optional fields
/// guarded so partial seeds (e.g. demo with only one asset class)
/// still parse.
#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioSeed {
    pub owner: Owner,
    pub summary_target_total_usd: Option<f64>,
    pub sukuks: Option<AssetClassGroup<SukukHolding>>,
    pub stocks: Option<StocksGroup>,
    pub etfs: Option<AssetClassGroup<EtfHolding>>,
    pub private_equity: Option<AssetClassGroup<PrivateEquityHolding>>,
    pub unit_trusts_tiger: Option<UnitTrustsTiger>,
    pub unit_trusts_cpf_phillip: Option<UnitTrustsTranches>,
    pub unit_trusts_srs_dbs: Option<UnitTrustsSrs>,
    pub ulip_india: Option<UlipGroup>,
    pub cpf_balance: Option<CpfBalance>,
    pub cash: Option<CashGroup>,
    pub real_estate: Option<RealEstateGroup>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Owner {
    pub name: String,
    pub base_currency: String,
    pub secondary_currency: Option<String>,
    pub tertiary_currency: Option<String>,
    pub school: String,
    pub as_of: String,
    #[serde(default)]
    pub fx_rates: BTreeMap<String, f64>,
}

/// Generic per-asset-class wrapper used by sukuks / ETFs / PE.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetClassGroup<H> {
    pub asset_class: String,
    pub total_usd: Option<f64>,
    pub holdings: Vec<H>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SukukHolding {
    pub isin: String,
    pub issuer: String,
    pub custodian: String,
    pub issue_date: String,
    pub maturity: String,
    pub purchase_value: f64,
    pub quantity: f64,
    pub purchase_price: f64,
    pub current_price: f64,
    pub coupon: f64,
    pub current_value: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StocksGroup {
    pub asset_class: String,
    pub total_usd: Option<f64>,
    pub subsets: Vec<StocksSubset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StocksSubset {
    pub label: String,
    pub data_source: Option<String>,
    pub current_value_usd: f64,
    pub current_value_sgd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EtfHolding {
    pub ticker: String,
    pub broker: String,
    pub qty: f64,
    pub current_price: f64,
    pub purchase_price: f64,
    pub current_value: f64,
    /// Either a number ("250", "1000") or "Lumpsum" — accept both.
    #[serde(default)]
    pub rsp_per_week: Option<RspPerWeek>,
}

/// The seed allows `rsp_per_week` to be either a number ("USD 1000")
/// or a string ("Lumpsum"). Untagged enum dispatches on first match.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RspPerWeek {
    Amount(f64),
    Label(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivateEquityHolding {
    pub vehicle: String,
    pub currency: String,
    pub amount: f64,
    pub usd_value: f64,
    pub date: Option<String>,
    /// Some PE holdings express quantity as a number, others as
    /// "1.5% equity" or "4 properties". Accept both.
    pub quantity: serde_json::Value,
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitTrustsTiger {
    pub asset_class: String,
    pub broker: String,
    pub total_usd: Option<f64>,
    pub holdings: Vec<UnitTrustHolding>,
    pub cash_balance_usd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitTrustHolding {
    pub fund: String,
    pub qty: f64,
    pub avg_cost: f64,
    pub current_nav: f64,
    pub investment_usd: f64,
    pub current_value_usd: f64,
    pub pnl: f64,
    #[serde(default)]
    pub sip: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitTrustsTranches {
    pub asset_class: String,
    pub broker: String,
    pub total_usd: Option<f64>,
    pub total_sgd_approx: Option<f64>,
    pub tranches: Vec<UnitTrustTranche>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitTrustTranche {
    pub date: String,
    pub investment_sgd: f64,
    pub current_sgd: f64,
    pub funds: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitTrustsSrs {
    pub asset_class: String,
    pub broker: String,
    pub total_usd: Option<f64>,
    pub total_sgd_approx: Option<f64>,
    pub holdings_template: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UlipGroup {
    pub asset_class: String,
    pub country: String,
    pub currency: String,
    pub total_invested_inr: Option<f64>,
    pub total_usd_approx: Option<f64>,
    pub policies: Vec<UlipPolicy>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UlipPolicy {
    pub company: String,
    pub name: String,
    pub policy: u64,
    pub start_date: String,
    pub maturity: String,
    pub term: u32,
    pub installment_inr: f64,
    pub mode: String,
    pub invested_inr: f64,
    pub fund_value_inr: Option<f64>,
    pub gain_loss_inr: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CpfBalance {
    pub asset_class: String,
    pub country: String,
    pub total_usd: f64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashGroup {
    pub asset_class: String,
    pub total_usd: Option<f64>,
    pub accounts: Vec<CashAccount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashAccount {
    pub country: String,
    pub bank: String,
    pub currency: String,
    pub balance: f64,
    pub usd_equiv: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RealEstateGroup {
    pub asset_class: String,
    pub properties: Vec<RealEstateProperty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RealEstateProperty {
    pub country: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub sqft: f64,
    pub year: Option<u32>,
    pub current_value_usd: Option<f64>,
    pub intent: String,
    #[serde(default)]
    pub purchase_aed: Option<f64>,
    #[serde(default)]
    pub current_aed: Option<f64>,
}

/// FX snapshot lifted out of `owner.fx_rates` so the importer can pin
/// every USD conversion to the seed's stated business date instead of
/// re-fetching at import time. The map keys are pairs like `"USD_SGD"`.
#[derive(Debug, Clone, PartialEq)]
pub struct FxSnapshot {
    pub as_of: String,
    pub rates: BTreeMap<String, Decimal>,
}

/// Errors from parsing or planning a seed file.
#[derive(Debug, thiserror::Error)]
pub enum SeedImportError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// A numeric field couldn't be represented as a `Decimal` (e.g.
    /// NaN, +/-Inf, or a float whose mantissa overflows). The seed is
    /// hand-curated so this should not happen in practice — surfaced
    /// instead of silently coercing to zero.
    #[error("invalid number for {field}: {value}")]
    InvalidNumber { field: String, value: f64 },
}

/// Parse a seed JSON blob into the typed shape.
pub fn parse_seed(json: &str) -> Result<PortfolioSeed, SeedImportError> {
    Ok(serde_json::from_str(json)?)
}

/// Lift the FX-rate table out of the seed and lock to the owner's
/// `as_of` date. Returns `None` when the seed has no rates table.
pub fn fx_snapshot(seed: &PortfolioSeed) -> Result<Option<FxSnapshot>, SeedImportError> {
    if seed.owner.fx_rates.is_empty() {
        return Ok(None);
    }
    let mut rates = BTreeMap::new();
    for (pair, value) in &seed.owner.fx_rates {
        rates.insert(pair.clone(), to_decimal(pair, *value)?);
    }
    Ok(Some(FxSnapshot {
        as_of: seed.owner.as_of.clone(),
        rates,
    }))
}

/// Planner — walks the seed and produces the ordered list of mutating
/// operations the persistence layer should apply. Order matters: the
/// owner profile and FX snapshot come first so per-holding ops can
/// reference them.
pub fn plan(seed: &PortfolioSeed) -> Result<Vec<Operation>, SeedImportError> {
    let mut ops = Vec::new();

    ops.push(Operation::SetOwnerProfile {
        name: seed.owner.name.clone(),
        base_currency: seed.owner.base_currency.clone(),
        secondary_currency: seed.owner.secondary_currency.clone(),
        school: seed.owner.school.clone(),
        as_of: seed.owner.as_of.clone(),
    });

    if let Some(fx) = fx_snapshot(seed)? {
        ops.push(Operation::SetFxSnapshot(fx));
    }

    if let Some(group) = &seed.sukuks {
        for h in &group.holdings {
            ops.push(Operation::CreateSukuk {
                isin: h.isin.clone(),
                issuer: h.issuer.clone(),
                custodian: h.custodian.clone(),
                issue_date: h.issue_date.clone(),
                maturity: h.maturity.clone(),
                purchase_value_usd: to_decimal("sukuk.purchase_value", h.purchase_value)?,
                quantity: to_decimal("sukuk.quantity", h.quantity)?,
                purchase_price: to_decimal("sukuk.purchase_price", h.purchase_price)?,
                current_price: to_decimal("sukuk.current_price", h.current_price)?,
                coupon_rate: to_decimal("sukuk.coupon", h.coupon)?,
                current_value_usd: to_decimal("sukuk.current_value", h.current_value)?,
            });
        }
    }

    if let Some(stocks) = &seed.stocks {
        for subset in &stocks.subsets {
            ops.push(Operation::CreateEquitySubset {
                label: subset.label.clone(),
                current_value_usd: to_decimal(
                    "stocks.current_value_usd",
                    subset.current_value_usd,
                )?,
                native_currency_value: match subset.current_value_sgd {
                    Some(v) => Some(to_decimal("stocks.current_value_sgd", v)?),
                    None => None,
                },
                data_source: subset.data_source.clone(),
            });
        }
    }

    if let Some(group) = &seed.etfs {
        for h in &group.holdings {
            ops.push(Operation::CreateEtf {
                ticker: h.ticker.clone(),
                broker: h.broker.clone(),
                qty: to_decimal("etf.qty", h.qty)?,
                current_price: to_decimal("etf.current_price", h.current_price)?,
                purchase_price: to_decimal("etf.purchase_price", h.purchase_price)?,
                current_value_usd: to_decimal("etf.current_value", h.current_value)?,
            });
        }
    }

    if let Some(group) = &seed.private_equity {
        for h in &group.holdings {
            ops.push(Operation::CreatePrivateEquity {
                vehicle: h.vehicle.clone(),
                currency: h.currency.clone(),
                amount_native: to_decimal("pe.amount", h.amount)?,
                usd_value: to_decimal("pe.usd_value", h.usd_value)?,
                date: h.date.clone(),
                quantity_note: h.quantity.to_string(),
                remarks: h.remarks.clone(),
            });
        }
    }

    if let Some(g) = &seed.unit_trusts_tiger {
        for h in &g.holdings {
            ops.push(Operation::CreateUnitTrust {
                broker: g.broker.clone(),
                wrapper: "Tiger".into(),
                fund: h.fund.clone(),
                qty: to_decimal("ut.qty", h.qty)?,
                avg_cost: to_decimal("ut.avg_cost", h.avg_cost)?,
                current_nav: to_decimal("ut.current_nav", h.current_nav)?,
                investment_usd: to_decimal("ut.investment_usd", h.investment_usd)?,
                current_value_usd: to_decimal("ut.current_value_usd", h.current_value_usd)?,
            });
        }
        if let Some(cash) = g.cash_balance_usd {
            ops.push(Operation::CreateCashAccount {
                country: "Singapore".into(),
                bank: g.broker.clone(),
                currency: "USD".into(),
                balance_native: to_decimal("ut.tiger.cash_balance_usd", cash)?,
                balance_usd_equiv: to_decimal("ut.tiger.cash_balance_usd", cash)?,
            });
        }
    }

    if let Some(g) = &seed.unit_trusts_cpf_phillip {
        for tranche in &g.tranches {
            ops.push(Operation::CreateUnitTrustTranche {
                broker: g.broker.clone(),
                wrapper: "CPF".into(),
                date: tranche.date.clone(),
                investment_native: to_decimal("ut.cpf.investment_sgd", tranche.investment_sgd)?,
                native_currency: "SGD".into(),
                current_native: to_decimal("ut.cpf.current_sgd", tranche.current_sgd)?,
                funds: tranche.funds.clone(),
            });
        }
    }

    if let Some(g) = &seed.unit_trusts_srs_dbs {
        // The SRS section only carries a template (fund names) without
        // per-fund per-holding values. Emit one placeholder op per
        // fund so the wire-up can render them as "Add valuation"
        // tiles — never silently materialised at the group total.
        for fund in &g.holdings_template {
            ops.push(Operation::CreateUnitTrustPlaceholder {
                broker: g.broker.clone(),
                wrapper: "SRS".into(),
                fund: fund.clone(),
            });
        }
    }

    if let Some(g) = &seed.ulip_india {
        for p in &g.policies {
            ops.push(Operation::CreateUlipPolicy {
                company: p.company.clone(),
                holder_name: p.name.clone(),
                policy_number: p.policy,
                start_date: p.start_date.clone(),
                maturity: p.maturity.clone(),
                term_years: p.term,
                installment_native: to_decimal("ulip.installment_inr", p.installment_inr)?,
                mode: p.mode.clone(),
                invested_native: to_decimal("ulip.invested_inr", p.invested_inr)?,
                fund_value_native: match p.fund_value_inr {
                    Some(v) => Some(to_decimal("ulip.fund_value_inr", v)?),
                    None => None,
                },
                currency: g.currency.clone(),
            });
        }
    }

    if let Some(c) = &seed.cpf_balance {
        ops.push(Operation::CreateProvidentFundBalance {
            country: c.country.clone(),
            total_usd: to_decimal("cpf.total_usd", c.total_usd)?,
            note: c.note.clone(),
        });
    }

    if let Some(g) = &seed.cash {
        for a in &g.accounts {
            ops.push(Operation::CreateCashAccount {
                country: a.country.clone(),
                bank: a.bank.clone(),
                currency: a.currency.clone(),
                balance_native: to_decimal("cash.balance", a.balance)?,
                balance_usd_equiv: to_decimal("cash.usd_equiv", a.usd_equiv)?,
            });
        }
    }

    if let Some(g) = &seed.real_estate {
        for p in &g.properties {
            ops.push(Operation::CreateRealEstateProperty {
                country: p.country.clone(),
                name: p.name.clone(),
                kind: p.kind.clone(),
                sqft: to_decimal("real_estate.sqft", p.sqft)?,
                current_value_usd: match p.current_value_usd {
                    Some(v) => Some(to_decimal("real_estate.current_value_usd", v)?),
                    None => None,
                },
                intent: p.intent.clone(),
                year_built: p.year,
            });
        }
    }

    Ok(ops)
}

fn to_decimal(field: &str, value: f64) -> Result<Decimal, SeedImportError> {
    Decimal::from_f64_retain(value).ok_or_else(|| SeedImportError::InvalidNumber {
        field: field.to_string(),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY_SEED: &str = r#"{
        "owner": {
            "name": "Test User",
            "base_currency": "USD",
            "school": "Hanafi",
            "as_of": "2026-04-04",
            "fx_rates": { "USD_SGD": 1.2756 }
        }
    }"#;

    #[test]
    fn parses_minimal_seed_with_only_owner() {
        let seed = parse_seed(TINY_SEED).unwrap();
        assert_eq!(seed.owner.name, "Test User");
        assert_eq!(seed.owner.school, "Hanafi");
        assert!(seed.sukuks.is_none());
        assert!(seed.cash.is_none());
    }

    #[test]
    fn fx_snapshot_pins_to_as_of_date() {
        let seed = parse_seed(TINY_SEED).unwrap();
        let snap = fx_snapshot(&seed).unwrap().unwrap();
        assert_eq!(snap.as_of, "2026-04-04");
        assert_eq!(snap.rates.len(), 1);
        // Decimal preserves the float's nearest-representation; the
        // bound is real but bounded to the seed boundary.
        assert!(snap.rates.contains_key("USD_SGD"));
    }

    #[test]
    fn fx_snapshot_returns_none_when_rates_table_missing() {
        let json = r#"{
            "owner": {
                "name": "x", "base_currency": "USD", "school": "Hanafi",
                "as_of": "2026-01-01"
            }
        }"#;
        let seed = parse_seed(json).unwrap();
        assert!(fx_snapshot(&seed).unwrap().is_none());
    }

    #[test]
    fn plan_emits_owner_profile_first_then_fx_then_holdings() {
        let json = r#"{
            "owner": {
                "name": "Test", "base_currency": "USD", "school": "Hanafi",
                "as_of": "2026-04-04",
                "fx_rates": { "USD_SGD": 1.2756 }
            },
            "sukuks": {
                "asset_class": "Bonds & Sukuks",
                "holdings": [{
                    "isin": "XS1", "issuer": "Test Issuer",
                    "custodian": "OCBC", "issue_date": "2024-01-01",
                    "maturity": "2030-01-01", "purchase_value": 100000,
                    "quantity": 1000, "purchase_price": 100,
                    "current_price": 99, "coupon": 0.05,
                    "current_value": 99000, "pnl": -1000
                }]
            }
        }"#;
        let seed = parse_seed(json).unwrap();
        let ops = plan(&seed).unwrap();
        assert!(matches!(ops[0], Operation::SetOwnerProfile { .. }));
        assert!(matches!(ops[1], Operation::SetFxSnapshot(_)));
        assert!(matches!(ops[2], Operation::CreateSukuk { .. }));
    }

    #[test]
    fn invalid_json_returns_json_variant() {
        let err = parse_seed("not json").unwrap_err();
        assert!(matches!(err, SeedImportError::Json(_)));
    }

    #[test]
    fn srs_template_emits_one_placeholder_per_fund_not_a_blended_total() {
        // The SRS section only carries fund names; emitting a total
        // would silently fabricate per-fund values, so we emit
        // placeholder ops instead.
        let json = r#"{
            "owner": {"name":"x","base_currency":"USD","school":"Hanafi","as_of":"2026-01-01"},
            "unit_trusts_srs_dbs": {
                "asset_class": "Unit Trusts (SRS)",
                "broker": "DBS (via SRS)",
                "holdings_template": ["Allianz GIF Global AI", "BlackRock Next Gen"]
            }
        }"#;
        let seed = parse_seed(json).unwrap();
        let ops = plan(&seed).unwrap();
        let placeholders: Vec<_> = ops
            .iter()
            .filter(|o| matches!(o, Operation::CreateUnitTrustPlaceholder { .. }))
            .collect();
        assert_eq!(placeholders.len(), 2);
    }

    #[test]
    fn rsp_per_week_accepts_either_amount_or_label() {
        let json = r#"{
            "owner": {"name":"x","base_currency":"USD","school":"Hanafi","as_of":"2026-01-01"},
            "etfs": {
                "asset_class": "ETFs",
                "holdings": [
                    {"ticker":"A","broker":"B","qty":1,"current_price":1,
                     "purchase_price":1,"current_value":1,"rsp_per_week":1000},
                    {"ticker":"C","broker":"B","qty":1,"current_price":1,
                     "purchase_price":1,"current_value":1,"rsp_per_week":"Lumpsum"}
                ]
            }
        }"#;
        let seed = parse_seed(json).unwrap();
        let etfs = seed.etfs.unwrap();
        assert!(matches!(
            etfs.holdings[0].rsp_per_week,
            Some(RspPerWeek::Amount(_))
        ));
        assert!(matches!(
            etfs.holdings[1].rsp_per_week,
            Some(RspPerWeek::Label(_))
        ));
    }

    #[test]
    fn real_estate_property_with_no_valuation_keeps_value_none() {
        // The seed has 4 Indian properties marked `current_value_usd:
        // null` — those must surface as "Add valuation" tiles, NOT
        // silently zeroed.
        let json = r#"{
            "owner": {"name":"x","base_currency":"USD","school":"Hanafi","as_of":"2026-01-01"},
            "real_estate": {
                "asset_class": "Real Estate",
                "properties": [
                    {"country":"India","name":"Aparna H-104","type":"3BHK","sqft":1800,
                     "year":null,"current_value_usd":null,"intent":"for_rent"}
                ]
            }
        }"#;
        let seed = parse_seed(json).unwrap();
        let ops = plan(&seed).unwrap();
        let re = ops
            .iter()
            .find_map(|o| match o {
                Operation::CreateRealEstateProperty {
                    name,
                    current_value_usd,
                    ..
                } => Some((name, current_value_usd)),
                _ => None,
            })
            .unwrap();
        assert_eq!(re.0, "Aparna H-104");
        assert!(re.1.is_none());
    }
}
