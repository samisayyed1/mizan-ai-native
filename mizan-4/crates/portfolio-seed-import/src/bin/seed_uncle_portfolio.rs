//! `seed-uncle-portfolio` — production-grade portfolio seeder.
//!
//! Reads a portfolio seed JSON (the shape Sami uses for Uncle Feroz's
//! cross-border position) and applies every `Operation` against the
//! Mizan local SQLite store via direct rusqlite INSERTs.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p mizan-portfolio-seed-import --features seed-cli \
//!     --bin seed-uncle-portfolio -- \
//!     --json ~/Downloads/uncle_portfolio_seed.json \
//!     --db "/Users/${USER}/Library/Application Support/app.mizan/app.db" \
//!     --wipe
//! ```
//!
//! # What it does (in order)
//!
//! 1. `mizan_storage_sqlite::db::run_migrations(db_path)` — ensures the
//!    schema is at HEAD, including the 2026-01-01 v2 asset refactor.
//! 2. If `--wipe` is set, deletes every row from the user-data tables
//!    (`activities`, `quotes`, `assets`, `accounts`, `holdings_snapshots`,
//!    `daily_account_valuation`). System tables (settings, schema
//!    versions, platforms, etc.) are preserved.
//! 3. Parses the JSON via `mizan_portfolio_seed_import::parse_seed` +
//!    `plan` — same parser used by the library tests, so the integration
//!    suite against the real Uncle Feroz fixture guarantees shape.
//! 4. Applies each `Operation` in a single transaction:
//!    - `CreateCashAccount`   → `accounts(CASH)` + `DEPOSIT` activity
//!    - `CreateSukuk`         → `accounts(SECURITIES)` + `assets(INVESTMENT, MANUAL, ISIN in metadata)` + `BUY` + manual `quotes` row
//!    - `CreateEquitySubset`  → `accounts(SECURITIES)` + `assets(INVESTMENT, MANUAL)` + `BUY` + manual quote
//!    - `CreateEtf`           → `accounts(SECURITIES)` + `assets(INVESTMENT, MARKET, ticker)` + `BUY` (MARKET so the Twelve Data / Yahoo path picks up live quotes)
//!    - `CreatePrivateEquity` → `accounts(SECURITIES)` + `assets(PRIVATE_EQUITY, MANUAL)` + `BUY` + manual quote
//!    - `CreateUnitTrust`     → `accounts(SECURITIES)` + `assets(INVESTMENT, MANUAL)` + `BUY` + manual quote
//!    - `CreateUnitTrustTranche`     → one asset per tranche (multi-fund grouped under the tranche date), `BUY` per tranche
//!    - `CreateUnitTrustPlaceholder` → `assets(INVESTMENT, MANUAL)` only — no value, surfaces as "Add valuation" tile
//!    - `CreateUlipPolicy`    → `accounts(SECURITIES, INR)` + `assets(INVESTMENT, MANUAL)` + `BUY` + manual quote
//!    - `CreateProvidentFundBalance` → `accounts(SECURITIES)` + `assets(INVESTMENT, MANUAL)` placeholder + manual quote
//!    - `CreateRealEstateProperty`   → `accounts(SECURITIES)` + `assets(PROPERTY, MANUAL, metadata.property.intent for Maliki/Hanbali school routing)` + `BUY` + manual quote
//!    - `SetOwnerProfile`     → `app_settings` rows for base_currency + school
//!    - `SetFxSnapshot`       → noop here (FX cache is rebuilt from quotes on next launch; the seed's USD-equivalent fields are authoritative)
//!
//! Transaction-scoped: a failure on any single operation rolls back
//! every prior insert. Either the demo DB lands fully populated or it
//! lands untouched — no half-state for Sami to debug at T-5 min.
//!
//! # Why direct SQL and not the service layer
//!
//! The service layer (`AccountService`, `AssetService`, `ActivityService`)
//! is wired through the Tauri `ServiceContext` which pulls in 12+ Arc
//! dependencies. Bootstrapping that graph headlessly is more code than
//! the SQL it would emit. The schema is captured by migrations; this
//! seeder just produces the same rows the services would.
//!
//! Manual quote rows use `source = 'MANUAL'` so the
//! `holdings_valuation_service` short-circuits the FX / market-data
//! lookup and uses the seed value directly — matching the existing
//! "Add valuation" affordance for non-tradable assets.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![allow(clippy::too_many_arguments)] // SQL bind lists are wide

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use log::{error, info, warn};
use rusqlite::{params, Connection, Transaction};
use rust_decimal::Decimal;
use uuid::Uuid;

use mizan_portfolio_seed_import::{plan, Operation, PortfolioSeed, SeedImportError};

const DEFAULT_DB_PATH: &str = "/Users/samisayyed/Library/Application Support/app.mizan/app.db";

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("seed-uncle-portfolio: {e}");
            eprintln!();
            eprintln!("{}", USAGE);
            return ExitCode::from(2);
        }
    };

    match run(args) {
        Ok(summary) => {
            println!();
            println!("✓ Seed applied: {summary}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            error!("seed failed: {e}");
            eprintln!("✗ {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
Usage: seed-uncle-portfolio --json <path> [--db <path>] [--wipe]

  --json <path>   Path to the portfolio seed JSON (required).
  --db <path>     SQLite database path (default: macOS Mizan dev DB).
  --wipe          Delete every row from the user-data tables before
                  seeding. Without this flag the seeder appends, which
                  may produce duplicates if you re-run.";

struct Args {
    json_path: PathBuf,
    db_path: PathBuf,
    wipe: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut json_path: Option<PathBuf> = None;
        let mut db_path: Option<PathBuf> = None;
        let mut wipe = false;
        let mut iter = std::env::args().skip(1);
        while let Some(a) = iter.next() {
            match a.as_str() {
                "--json" => {
                    json_path = Some(PathBuf::from(iter.next().ok_or("--json needs a value")?));
                }
                "--db" => {
                    db_path = Some(PathBuf::from(iter.next().ok_or("--db needs a value")?));
                }
                "--wipe" => wipe = true,
                "--help" | "-h" => return Err("see usage".into()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(Self {
            json_path: json_path.ok_or("--json is required")?,
            db_path: db_path.unwrap_or_else(|| PathBuf::from(DEFAULT_DB_PATH)),
            wipe,
        })
    }
}

#[derive(Debug)]
struct ApplySummary {
    accounts: usize,
    assets: usize,
    activities: usize,
    quotes: usize,
}

impl std::fmt::Display for ApplySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} accounts, {} assets, {} activities, {} manual quotes",
            self.accounts, self.assets, self.activities, self.quotes
        )
    }
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("read JSON {0}: {1}")]
    ReadJson(PathBuf, std::io::Error),
    #[error("parse seed: {0}")]
    Parse(#[from] SeedImportError),
    #[error("migrations failed: {0}")]
    Migrate(String),
    #[error("sqlite: {0}")]
    Sql(#[from] rusqlite::Error),
}

fn run(args: Args) -> Result<ApplySummary, AppError> {
    let raw = std::fs::read_to_string(&args.json_path)
        .map_err(|e| AppError::ReadJson(args.json_path.clone(), e))?;
    let seed: PortfolioSeed = mizan_portfolio_seed_import::parse_seed(&raw)?;
    info!(
        "loaded seed for {} ({}); as_of {}",
        seed.owner.name, seed.owner.base_currency, seed.owner.as_of
    );

    let ops = plan(&seed)?;
    info!("planned {} operations", ops.len());
    log_op_summary(&ops);

    // Run migrations against the target DB. Idempotent — a no-op if
    // schema is already at HEAD.
    let db_str = args
        .db_path
        .to_str()
        .ok_or_else(|| AppError::Migrate("DB path is not valid UTF-8".into()))?;
    mizan_storage_sqlite::db::run_migrations(db_str)
        .map_err(|e| AppError::Migrate(e.to_string()))?;

    let mut conn = Connection::open(&args.db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let tx = conn.transaction()?;
    if args.wipe {
        info!("wiping user-data tables (preserving settings, platforms, schema)");
        wipe_user_tables(&tx)?;
    }
    let summary = apply_operations(&tx, &ops)?;
    tx.commit()?;
    Ok(summary)
}

fn log_op_summary(ops: &[Operation]) {
    let counts = mizan_portfolio_seed_import::operations::summarise(ops);
    for (kind, n) in counts {
        info!("  {kind:?}: {n}");
    }
}

/// Tables that hold user data the seeder owns. Order matters: respect
/// foreign-key direction so each DELETE can complete without violating
/// integrity (activities → quotes → assets → accounts).
const WIPE_TABLES: &[&str] = &[
    "activities",
    "quotes",
    "holdings_snapshots",
    "daily_account_valuation",
    "assets",
    "accounts",
];

fn wipe_user_tables(tx: &Transaction<'_>) -> Result<(), rusqlite::Error> {
    for t in WIPE_TABLES {
        // Tolerate the table not existing (older schemas during dev).
        if let Err(e) = tx.execute(&format!("DELETE FROM {t}"), []) {
            warn!("DELETE FROM {t} failed (may not exist yet): {e}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Operation application
// ---------------------------------------------------------------------

fn apply_operations(
    tx: &Transaction<'_>,
    ops: &[Operation],
) -> Result<ApplySummary, rusqlite::Error> {
    let mut summary = ApplySummary {
        accounts: 0,
        assets: 0,
        activities: 0,
        quotes: 0,
    };
    let mut base_currency = String::from("USD");

    for op in ops {
        match op {
            Operation::SetOwnerProfile {
                base_currency: bc,
                school,
                name,
                ..
            } => {
                base_currency = bc.clone();
                upsert_setting(tx, "base_currency", bc)?;
                upsert_setting(tx, "user.name", name)?;
                upsert_setting(tx, "zakat.school", &school.to_lowercase())?;
            }

            Operation::SetFxSnapshot(_snap) => {
                // Authoritative USD-equivalent values are baked into
                // every per-holding op (we stored both native and
                // USD-equivalent). The Tauri startup re-derives the
                // live FX cache from market data on next launch — we
                // intentionally do NOT pre-populate the FX cache here
                // to avoid masking a real FX-service failure on demo
                // day with stale rates from the seed file.
            }

            Operation::CreateCashAccount {
                country,
                bank,
                currency,
                balance_native,
                ..
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{bank} ({country})"),
                    "CASH",
                    currency,
                    Some("Cash"),
                )?;
                summary.accounts += 1;
                insert_cash_activity(tx, &acct_id, *balance_native, currency)?;
                summary.activities += 1;
            }

            Operation::CreateSukuk {
                isin,
                issuer,
                custodian,
                issue_date,
                maturity,
                quantity,
                purchase_price,
                current_price,
                current_value_usd: _,
                ..
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{custodian} — Sukuk"),
                    "SECURITIES",
                    "USD",
                    Some("Bonds & Sukuks"),
                )?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: &format!("{issuer} {maturity} Sukuk"),
                        display_code: isin,
                        quote_mode: "MANUAL",
                        quote_ccy: "USD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "identifiers": { "isin": isin },
                            "fixed_income": {
                                "issuer": issuer,
                                "issue_date": issue_date,
                                "maturity": maturity,
                            },
                            "asset_class_hint": "BONDS_SUKUKS",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    *quantity,
                    *purchase_price,
                    "USD",
                    issue_date,
                )?;
                summary.activities += 1;
                insert_manual_quote(tx, &asset_id, *current_price, "USD")?;
                summary.quotes += 1;
            }

            Operation::CreateEquitySubset {
                label,
                current_value_usd,
                ..
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{label} — Brokerage"),
                    "SECURITIES",
                    "USD",
                    Some("Equities"),
                )?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: label,
                        display_code: label,
                        quote_mode: "MANUAL",
                        quote_ccy: "USD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "asset_class_hint": "EQUITIES",
                            "is_rollup": true,
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    *current_value_usd,
                    "USD",
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                insert_manual_quote(tx, &asset_id, *current_value_usd, "USD")?;
                summary.quotes += 1;
            }

            Operation::CreateEtf {
                ticker,
                broker,
                qty,
                current_price,
                purchase_price,
                ..
            } => {
                let acct_id = insert_or_get_account(tx, broker, "SECURITIES", "USD", Some("ETFs"))?;
                summary.accounts += match insert_or_get_account_was_new(&acct_id) {
                    true => 1,
                    false => 0,
                };
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: ticker,
                        display_code: ticker,
                        // MARKET so the Twelve Data / Yahoo provider
                        // tries to fetch a live quote; if it fails the
                        // valuation service falls back to the manual
                        // quote we also write below.
                        quote_mode: "MARKET",
                        quote_ccy: "USD",
                        instrument_type: Some("EQUITY"),
                        instrument_symbol: Some(ticker),
                        metadata_json: Some(serde_json::json!({
                            "asset_class_hint": "ETFS",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    *qty,
                    *purchase_price,
                    "USD",
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                // Seed a MANUAL quote as a fallback for cold-start
                // (before market-data refresh) and for offline demo.
                insert_manual_quote(tx, &asset_id, *current_price, "USD")?;
                summary.quotes += 1;
            }

            Operation::CreatePrivateEquity {
                vehicle,
                currency,
                amount_native,
                usd_value,
                remarks,
                ..
            } => {
                let acct_id =
                    insert_account(tx, vehicle, "SECURITIES", currency, Some("Private Equity"))?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "PRIVATE_EQUITY",
                        name: vehicle,
                        display_code: vehicle,
                        quote_mode: "MANUAL",
                        quote_ccy: currency,
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "remarks": remarks,
                            "asset_class_hint": "PRIVATE_EQUITY",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    *amount_native,
                    currency,
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                // Seed both the native-currency manual quote and a USD
                // equivalent under the same asset id so the dashboard's
                // base-currency converter has a reliable basis.
                insert_manual_quote(tx, &asset_id, *amount_native, currency)?;
                summary.quotes += 1;
                if currency != "USD" {
                    insert_manual_quote_usd_overlay(tx, &asset_id, *usd_value)?;
                    summary.quotes += 1;
                }
            }

            Operation::CreateUnitTrust {
                broker,
                wrapper,
                fund,
                qty,
                avg_cost,
                current_nav,
                ..
            } => {
                let acct_id = insert_or_get_account(
                    tx,
                    &format!("{broker} — {wrapper}"),
                    "SECURITIES",
                    "USD",
                    Some("Unit Trusts"),
                )?;
                summary.accounts += match insert_or_get_account_was_new(&acct_id) {
                    true => 1,
                    false => 0,
                };
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: fund,
                        display_code: fund,
                        quote_mode: "MANUAL",
                        quote_ccy: "USD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "wrapper": wrapper,
                            "asset_class_hint": "UNIT_TRUSTS",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    *qty,
                    *avg_cost,
                    "USD",
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                insert_manual_quote(tx, &asset_id, *current_nav, "USD")?;
                summary.quotes += 1;
            }

            Operation::CreateUnitTrustTranche {
                broker,
                wrapper,
                date,
                investment_native,
                native_currency,
                current_native,
                funds,
            } => {
                let acct_id = insert_or_get_account(
                    tx,
                    &format!("{broker} — {wrapper}"),
                    "SECURITIES",
                    native_currency,
                    Some("Unit Trusts"),
                )?;
                summary.accounts += match insert_or_get_account_was_new(&acct_id) {
                    true => 1,
                    false => 0,
                };
                // Each tranche is a single dated investment that holds
                // multiple funds. We model it as one asset (the tranche)
                // with the fund list in metadata so the holdings drill-
                // down can render the underlying mix without inflating
                // the per-asset count.
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: &format!("{wrapper} Tranche {date}"),
                        display_code: &format!("UT-{date}"),
                        quote_mode: "MANUAL",
                        quote_ccy: native_currency,
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "wrapper": wrapper,
                            "tranche_date": date,
                            "funds": funds,
                            "asset_class_hint": "UNIT_TRUSTS",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    *investment_native,
                    native_currency,
                    date,
                )?;
                summary.activities += 1;
                insert_manual_quote(tx, &asset_id, *current_native, native_currency)?;
                summary.quotes += 1;
            }

            Operation::CreateUnitTrustPlaceholder {
                broker,
                wrapper,
                fund,
            } => {
                let acct_id = insert_or_get_account(
                    tx,
                    &format!("{broker} — {wrapper}"),
                    "SECURITIES",
                    "SGD",
                    Some("Unit Trusts"),
                )?;
                summary.accounts += match insert_or_get_account_was_new(&acct_id) {
                    true => 1,
                    false => 0,
                };
                let _asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: fund,
                        display_code: fund,
                        quote_mode: "MANUAL",
                        quote_ccy: "SGD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "wrapper": wrapper,
                            "needs_valuation": true,
                            "asset_class_hint": "UNIT_TRUSTS",
                        })),
                    },
                )?;
                summary.assets += 1;
                // No activity, no quote — surfaces as "Add valuation".
            }

            Operation::CreateUlipPolicy {
                company,
                holder_name,
                policy_number,
                start_date,
                maturity,
                term_years,
                installment_native,
                mode,
                invested_native,
                fund_value_native,
                currency,
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{company} — {holder_name}"),
                    "SECURITIES",
                    currency,
                    Some("Insurance (ULIPs)"),
                )?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: &format!("{company} ULIP — {holder_name}"),
                        display_code: &format!("ULIP-{policy_number}"),
                        quote_mode: "MANUAL",
                        quote_ccy: currency,
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "policy_number": policy_number,
                            "holder_name": holder_name,
                            "start_date": start_date,
                            "maturity": maturity,
                            "term_years": term_years,
                            "installment_native": installment_native.to_string(),
                            "installment_mode": mode,
                            "asset_class_hint": "INSURANCE",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    *invested_native,
                    currency,
                    start_date,
                )?;
                summary.activities += 1;
                // ULIP fund value may be NULL (not yet started); only
                // emit a manual quote when we have one.
                if let Some(fv) = fund_value_native {
                    insert_manual_quote(tx, &asset_id, *fv, currency)?;
                    summary.quotes += 1;
                }
            }

            Operation::CreateProvidentFundBalance {
                country,
                total_usd,
                note,
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{country} Provident Fund"),
                    "SECURITIES",
                    "USD",
                    Some("Provident Funds"),
                )?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "INVESTMENT",
                        name: &format!("{country} CPF balance"),
                        display_code: &format!("PF-{country}"),
                        quote_mode: "MANUAL",
                        quote_ccy: "USD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "country": country,
                            "note": note,
                            "asset_class_hint": "PROVIDENT_FUNDS",
                        })),
                    },
                )?;
                summary.assets += 1;
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    *total_usd,
                    "USD",
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                insert_manual_quote(tx, &asset_id, *total_usd, "USD")?;
                summary.quotes += 1;
            }

            Operation::CreateRealEstateProperty {
                country,
                name,
                kind,
                sqft,
                current_value_usd,
                intent,
                year_built,
            } => {
                let acct_id = insert_account(
                    tx,
                    &format!("{country} Real Estate"),
                    "SECURITIES",
                    "USD",
                    Some("Real Estate"),
                )?;
                summary.accounts += 1;
                let asset_id = insert_asset(
                    tx,
                    AssetIn {
                        kind: "PROPERTY",
                        name,
                        display_code: name,
                        quote_mode: "MANUAL",
                        quote_ccy: "USD",
                        instrument_type: None,
                        instrument_symbol: None,
                        metadata_json: Some(serde_json::json!({
                            "country": country,
                            "type": kind,
                            "sqft": sqft.to_string(),
                            "year_built": year_built,
                            // The `property.intent` shape is what
                            // mizan-zakat::extract_property_intent reads
                            // for school-aware Zakat routing (Maliki
                            // excludes for_rent, Hanbali treats
                            // primary_residence as fully exempt, etc.).
                            "property": { "intent": intent },
                            "asset_class_hint": "REAL_ESTATE",
                        })),
                    },
                )?;
                summary.assets += 1;
                // Some Indian properties have no valuation (Sami needs
                // an appraisal). Surface them via the asset row + an
                // empty BUY at zero so they appear in the panel but
                // signal "Add valuation".
                let value = current_value_usd.unwrap_or(Decimal::ZERO);
                insert_activity_buy(
                    tx,
                    &acct_id,
                    &asset_id,
                    Decimal::ONE,
                    value,
                    "USD",
                    today_iso().as_str(),
                )?;
                summary.activities += 1;
                if let Some(v) = current_value_usd {
                    insert_manual_quote(tx, &asset_id, *v, "USD")?;
                    summary.quotes += 1;
                }
            }
        }
    }

    // Persist base currency on the off chance SetOwnerProfile wasn't
    // emitted (tests with bare seeds).
    upsert_setting(tx, "base_currency", &base_currency)?;
    Ok(summary)
}

// ---------------------------------------------------------------------
// Insertion helpers
// ---------------------------------------------------------------------

struct AssetIn<'a> {
    kind: &'a str,
    name: &'a str,
    display_code: &'a str,
    quote_mode: &'a str,
    quote_ccy: &'a str,
    instrument_type: Option<&'a str>,
    instrument_symbol: Option<&'a str>,
    metadata_json: Option<serde_json::Value>,
}

fn now_iso() -> String {
    // Seconds-precision RFC3339 UTC; matches the migration's default
    // and avoids drift between created_at and the row's wall time.
    let now = Utc::now();
    now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn today_iso() -> String {
    Utc::now().format("%Y-%m-%dT00:00:00Z").to_string()
}

fn today_day() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn insert_account(
    tx: &Transaction<'_>,
    name: &str,
    account_type: &str,
    currency: &str,
    group: Option<&str>,
) -> Result<String, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    tx.execute(
        "INSERT INTO accounts (id, name, account_type, \"group\", currency, is_default, is_active, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, ?6, ?6)",
        params![id, name, account_type, group, currency, now],
    )?;
    Ok(id)
}

/// Dedupe-aware account creator. Returns the existing id when an
/// account with the same name + currency already exists, else inserts
/// a new row. Used for grouped operations (ETFs, UT tranches, SRS
/// placeholders) where multiple ops collapse into one account.
///
/// Encodes the "was new" bit into the returned id by prepending a
/// "NEW:" sentinel, decoded by `insert_or_get_account_was_new`.
fn insert_or_get_account(
    tx: &Transaction<'_>,
    name: &str,
    account_type: &str,
    currency: &str,
    group: Option<&str>,
) -> Result<String, rusqlite::Error> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM accounts WHERE name = ?1 AND currency = ?2 LIMIT 1",
            params![name, currency],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = insert_account(tx, name, account_type, currency, group)?;
    Ok(format!("NEW:{id}"))
}

fn insert_or_get_account_was_new(id: &str) -> bool {
    id.starts_with("NEW:")
}

fn insert_asset(tx: &Transaction<'_>, a: AssetIn<'_>) -> Result<String, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let metadata = a.metadata_json.map(|v| v.to_string());
    tx.execute(
        "INSERT INTO assets (id, kind, name, display_code, metadata, is_active, quote_mode, quote_ccy, instrument_type, instrument_symbol, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            id,
            a.kind,
            a.name,
            a.display_code,
            metadata,
            a.quote_mode,
            a.quote_ccy,
            a.instrument_type,
            a.instrument_symbol,
            now,
        ],
    )?;
    Ok(strip_new_prefix(&id))
}

fn strip_new_prefix(id: &str) -> String {
    id.strip_prefix("NEW:").unwrap_or(id).to_string()
}

fn insert_activity_buy(
    tx: &Transaction<'_>,
    account_id: &str,
    asset_id: &str,
    quantity: Decimal,
    unit_price: Decimal,
    currency: &str,
    activity_date: &str,
) -> Result<(), rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let acct = strip_new_prefix(account_id);
    let asset = strip_new_prefix(asset_id);
    let date_iso = if activity_date.contains('T') {
        activity_date.to_string()
    } else {
        format!("{activity_date}T00:00:00Z")
    };
    tx.execute(
        "INSERT INTO activities (id, account_id, asset_id, activity_type, status, activity_date, quantity, unit_price, currency, created_at, updated_at, is_user_modified, needs_review) \
         VALUES (?1, ?2, ?3, 'BUY', 'POSTED', ?4, ?5, ?6, ?7, ?8, ?8, 0, 0)",
        params![
            id,
            acct,
            asset,
            date_iso,
            quantity.to_string(),
            unit_price.to_string(),
            currency,
            now,
        ],
    )?;
    Ok(())
}

fn insert_cash_activity(
    tx: &Transaction<'_>,
    account_id: &str,
    balance_native: Decimal,
    currency: &str,
) -> Result<(), rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let acct = strip_new_prefix(account_id);
    let date_iso = today_iso();
    // Cash is asset_id=NULL per the v2 refactor (CASH no longer has
    // asset rows). DEPOSIT activity with amount = balance.
    tx.execute(
        "INSERT INTO activities (id, account_id, asset_id, activity_type, status, activity_date, amount, currency, created_at, updated_at, is_user_modified, needs_review) \
         VALUES (?1, ?2, NULL, 'DEPOSIT', 'POSTED', ?3, ?4, ?5, ?6, ?6, 0, 0)",
        params![
            id,
            acct,
            date_iso,
            balance_native.to_string(),
            currency,
            now,
        ],
    )?;
    Ok(())
}

fn insert_manual_quote(
    tx: &Transaction<'_>,
    asset_id: &str,
    price: Decimal,
    currency: &str,
) -> Result<(), rusqlite::Error> {
    let asset = strip_new_prefix(asset_id);
    let id = format!("{asset}_{}_MANUAL", today_day());
    let now = now_iso();
    let day = today_day();
    // ON CONFLICT — re-runs (without --wipe) and overlay-USD writes
    // (which generate the same id with MANUAL source) need to update,
    // not duplicate.
    tx.execute(
        "INSERT INTO quotes (id, asset_id, day, source, close, currency, created_at, timestamp) \
         VALUES (?1, ?2, ?3, 'MANUAL', ?4, ?5, ?6, ?6) \
         ON CONFLICT(id) DO UPDATE SET close = excluded.close, currency = excluded.currency, timestamp = excluded.timestamp",
        params![id, asset, day, price.to_string(), currency, now],
    )?;
    Ok(())
}

fn insert_manual_quote_usd_overlay(
    tx: &Transaction<'_>,
    asset_id: &str,
    usd_value: Decimal,
) -> Result<(), rusqlite::Error> {
    let asset = strip_new_prefix(asset_id);
    let id = format!("{asset}_{}_MANUAL_USD", today_day());
    let now = now_iso();
    let day = today_day();
    tx.execute(
        "INSERT INTO quotes (id, asset_id, day, source, close, currency, created_at, timestamp) \
         VALUES (?1, ?2, ?3, 'MANUAL_USD', ?4, 'USD', ?5, ?5) \
         ON CONFLICT(id) DO UPDATE SET close = excluded.close, timestamp = excluded.timestamp",
        params![id, asset, day, usd_value.to_string(), now],
    )?;
    Ok(())
}

fn upsert_setting(tx: &Transaction<'_>, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    // Settings table is a KV store post-migration 2024-09-21. Use
    // INSERT OR REPLACE so the seeder is idempotent.
    tx.execute(
        "INSERT INTO app_settings (setting_key, setting_value) VALUES (?1, ?2) \
         ON CONFLICT(setting_key) DO UPDATE SET setting_value = excluded.setting_value",
        params![key, value],
    )?;
    Ok(())
}

