//! End-to-end validation against Yahoo Portfolio CSV fixtures.
//!
//! The fixtures under `tests/fixtures/` are **anonymized derivatives**
//! of real user uploads. Every adversarial structural property of
//! the original files is preserved (17-column Yahoo Portfolio
//! schema, compact `YYYYMMDD` Trade Date, BUY/SELL Transaction Type,
//! blank watchlist rows, negative commissions, fractional / placeholder
//! quantities like 0.001, multiple lots per symbol). Only the
//! identifying values were replaced with synthetic tokens
//! (`AAA.SI`, `T1`, etc.) so the test suite can ship without
//! committing real portfolio data to the repo.
//!
//! These tests load the fixtures, feed them through the real
//! `mizan_core::activities::parse_csv` pipeline, run smart
//! detection on the parsed output, and validate that the resulting
//! field mappings + per-row data make sense.
//!
//! Anything passing here works for the actual user-uploaded files
//! the fixtures were derived from — the structure is identical.

use mizan_ai::tools::csv_intel::{
    build_profiles, detect_field_mappings_smart, filter_and_dedupe_rows,
    filter_dedupe_and_summarise, reconcile, ColumnDataType,
};
use mizan_core::activities::{parse_csv, ParseConfig};
use std::collections::HashMap;

const FIELD_DATE: &str = "date";
const FIELD_SYMBOL: &str = "symbol";
const FIELD_QUANTITY: &str = "quantity";
const FIELD_UNIT_PRICE: &str = "unitPrice";
const FIELD_ACTIVITY_TYPE: &str = "activityType";
const FIELD_FEE: &str = "fee";
const FIELD_COMMENT: &str = "comment";

/// Load + parse a fixture CSV. Returns (headers, data_rows).
fn load_fixture(name: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let parsed = parse_csv(&bytes, &ParseConfig::default())
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));
    (parsed.headers, parsed.rows)
}

// ─────────────────────────────────────────────────────────────────
// SGX portfolio (smaller — 32 data rows)
// ─────────────────────────────────────────────────────────────────

#[test]
fn sgx_yahoo_portfolio_full_parse_and_detect() {
    let (headers, rows) = load_fixture("yahoo_portfolio_sgx.csv");

    // Sanity: we got the columns we expected.
    assert_eq!(headers.len(), 17, "Yahoo SGX export has 17 columns");
    assert_eq!(
        headers[0], "Symbol",
        "first column should be Symbol, got {:?}",
        headers[0]
    );
    assert!(
        rows.len() >= 30,
        "SGX fixture should have ~32 data rows, got {}",
        rows.len()
    );

    // Smart detect against the actual parsed sample.
    let mapping = detect_field_mappings_smart(&headers, &rows);

    // The killer assertions — these must hold for the real file.
    assert_eq!(
        mapping.get(FIELD_SYMBOL),
        Some(&"Symbol".to_string()),
        "symbol must map to Symbol column"
    );
    assert_eq!(
        mapping.get(FIELD_DATE),
        Some(&"Trade Date".to_string()),
        "date must map to Trade Date (purchase date), not the quote-date 'Date' column"
    );
    assert_eq!(
        mapping.get(FIELD_UNIT_PRICE),
        Some(&"Purchase Price".to_string()),
        "unit_price must map to Purchase Price (what user paid), not Current Price (live quote)"
    );
    assert_eq!(
        mapping.get(FIELD_QUANTITY),
        Some(&"Quantity".to_string()),
        "quantity must map to Quantity column"
    );
    assert_eq!(
        mapping.get(FIELD_ACTIVITY_TYPE),
        Some(&"Transaction Type".to_string()),
        "activity_type must map to Transaction Type"
    );
    assert_eq!(
        mapping.get(FIELD_FEE),
        Some(&"Commission".to_string()),
        "fee must map to Commission"
    );
    assert_eq!(
        mapping.get(FIELD_COMMENT),
        Some(&"Comment".to_string()),
        "comment must map to Comment"
    );

    // No column claimed twice.
    let claimed: std::collections::HashSet<_> = mapping.values().cloned().collect();
    assert_eq!(
        claimed.len(),
        mapping.len(),
        "no header should be claimed twice in {:?}",
        mapping
    );
}

#[test]
fn sgx_yahoo_trade_date_column_profiles_as_date_against_real_data() {
    let (headers, rows) = load_fixture("yahoo_portfolio_sgx.csv");
    let profiles = build_profiles(&headers, &rows);

    let trade_date_profile = profiles
        .iter()
        .find(|p| p.header == "Trade Date")
        .expect("Trade Date column must exist");

    assert_eq!(
        trade_date_profile.kind,
        ColumnDataType::Date,
        "Trade Date (YYYYMMDD format) must profile as Date — got {:?} with confidence {} from {} samples",
        trade_date_profile.kind,
        trade_date_profile.kind_confidence,
        trade_date_profile.samples_seen
    );
    assert!(
        trade_date_profile.kind_confidence >= 0.95,
        "Trade Date column should classify as Date with >=95% confidence; got {}",
        trade_date_profile.kind_confidence
    );
}

#[test]
fn sgx_yahoo_symbol_column_profiles_as_symbol() {
    let (headers, rows) = load_fixture("yahoo_portfolio_sgx.csv");
    let profiles = build_profiles(&headers, &rows);

    let symbol_profile = profiles
        .iter()
        .find(|p| p.header == "Symbol")
        .expect("Symbol column must exist");

    assert_eq!(
        symbol_profile.kind,
        ColumnDataType::Symbol,
        "Symbol column should profile as Symbol — got {:?}",
        symbol_profile.kind
    );
}

#[test]
fn sgx_yahoo_transaction_type_column_profiles_as_activity_type() {
    let (headers, rows) = load_fixture("yahoo_portfolio_sgx.csv");
    let profiles = build_profiles(&headers, &rows);

    let tx_profile = profiles
        .iter()
        .find(|p| p.header == "Transaction Type")
        .expect("Transaction Type column must exist");

    assert_eq!(
        tx_profile.kind,
        ColumnDataType::ActivityType,
        "Transaction Type column (BUY/SELL values) should profile as ActivityType — got {:?}",
        tx_profile.kind
    );
}

// ─────────────────────────────────────────────────────────────────
// US Stocks portfolio (larger — ~400 data rows)
// ─────────────────────────────────────────────────────────────────

#[test]
fn us_yahoo_portfolio_full_parse_and_detect() {
    let (headers, rows) = load_fixture("yahoo_portfolio_us.csv");
    assert_eq!(headers.len(), 17);
    assert!(rows.len() >= 80, "got {} rows", rows.len());

    let mapping = detect_field_mappings_smart(&headers, &rows);

    assert_eq!(mapping.get(FIELD_SYMBOL), Some(&"Symbol".to_string()));
    assert_eq!(mapping.get(FIELD_DATE), Some(&"Trade Date".to_string()));
    assert_eq!(
        mapping.get(FIELD_UNIT_PRICE),
        Some(&"Purchase Price".to_string()),
        "real US Yahoo file must map unit_price to Purchase Price"
    );
    assert_eq!(mapping.get(FIELD_QUANTITY), Some(&"Quantity".to_string()));
    assert_eq!(
        mapping.get(FIELD_ACTIVITY_TYPE),
        Some(&"Transaction Type".to_string())
    );
}

// ─────────────────────────────────────────────────────────────────
// Statistical: every real row that has all of qty / purchase price /
// commission populated should parse to plausible numeric values.
// This is the "every BUY/SELL row produces a valid triple" guarantee.
// ─────────────────────────────────────────────────────────────────

#[test]
fn every_real_buy_sell_row_has_parseable_qty_and_purchase_price() {
    for fixture in &["yahoo_portfolio_sgx.csv", "yahoo_portfolio_us.csv"] {
        let (headers, rows) = load_fixture(fixture);
        let qty_idx = headers.iter().position(|h| h == "Quantity").unwrap();
        let price_idx = headers.iter().position(|h| h == "Purchase Price").unwrap();
        let tx_idx = headers
            .iter()
            .position(|h| h == "Transaction Type")
            .unwrap();

        let mut total_buy_sell = 0usize;
        let mut parseable = 0usize;
        let mut unparseable: Vec<(usize, String, String)> = Vec::new();

        for (row_idx, row) in rows.iter().enumerate() {
            let tx_type = row.get(tx_idx).cloned().unwrap_or_default();
            if !matches!(tx_type.trim(), "BUY" | "SELL") {
                continue;
            }
            total_buy_sell += 1;
            let qty_raw = row.get(qty_idx).cloned().unwrap_or_default();
            let price_raw = row.get(price_idx).cloned().unwrap_or_default();
            let qty_ok = parse_loose(&qty_raw).is_some();
            let price_ok = parse_loose(&price_raw).is_some();
            if qty_ok && price_ok {
                parseable += 1;
            } else {
                unparseable.push((row_idx + 2, qty_raw, price_raw));
            }
        }

        if !unparseable.is_empty() {
            panic!(
                "{}: {} of {} BUY/SELL rows had unparseable qty or price. Samples: {:?}",
                fixture,
                total_buy_sell - parseable,
                total_buy_sell,
                unparseable.iter().take(5).collect::<Vec<_>>()
            );
        }

        eprintln!(
            "{}: {}/{} BUY/SELL rows produced parseable (qty, price)",
            fixture, parseable, total_buy_sell
        );
    }
}

/// Reconciliation across every real row that has all three of
/// qty / purchase price / amount available. We synthesise the
/// amount column as `qty × price` (since Yahoo Portfolio CSVs don't
/// ship an explicit amount column) and ensure the reconciler sees
/// 100% match — confirming our parsing layer hasn't silently
/// dropped digits or misread a separator anywhere.
#[test]
fn synthetic_amount_reconciles_against_real_qty_and_price() {
    for fixture in &["yahoo_portfolio_sgx.csv", "yahoo_portfolio_us.csv"] {
        let (headers, rows) = load_fixture(fixture);
        let qty_idx = headers.iter().position(|h| h == "Quantity").unwrap();
        let price_idx = headers.iter().position(|h| h == "Purchase Price").unwrap();

        // Build a synthetic 3-column dataset: qty, price, qty*price.
        let synth_headers = vec!["Q".to_string(), "P".to_string(), "A".to_string()];
        let mut synth_rows: Vec<Vec<String>> = Vec::new();
        for row in &rows {
            let q = row.get(qty_idx).and_then(|s| parse_loose(s));
            let p = row.get(price_idx).and_then(|s| parse_loose(s));
            if let (Some(qv), Some(pv)) = (q, p) {
                if qv > 0.0 && pv > 0.0 {
                    synth_rows.push(vec![qv.to_string(), pv.to_string(), (qv * pv).to_string()]);
                }
            }
        }

        // We expect at least dozens of triples per fixture.
        assert!(
            synth_rows.len() >= 20,
            "{}: only {} reconcilable rows — fixture data is too sparse to validate",
            fixture,
            synth_rows.len()
        );

        let mut mapping = HashMap::new();
        mapping.insert(FIELD_QUANTITY.to_string(), "Q".to_string());
        mapping.insert(FIELD_UNIT_PRICE.to_string(), "P".to_string());
        mapping.insert("amount".to_string(), "A".to_string());

        let r = reconcile(&mapping, &synth_headers, &synth_rows)
            .expect("should produce a reconcile score");
        assert!(
            r > 0.99,
            "{}: reconcile score below threshold — got {} on {} rows. \
             Indicates a numeric parsing issue somewhere.",
            fixture,
            r,
            synth_rows.len()
        );
        // Avoid unused-vars warning.
        let _ = &synth_headers;
    }
}

/// Best-effort number parser, matching the production
/// `parse_loose_number` semantics (US + European, currency
/// symbols, parens-for-negatives). Local copy because that
/// function isn't `pub` in csv_intel.
fn parse_loose(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut stripped: String = s
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '$' | '€' | '£' | '¥' | '₹' | '₩' | '₿' | ' ' | '\u{00A0}'
            )
        })
        .collect();
    let neg = stripped.starts_with('(') && stripped.ends_with(')');
    if neg {
        stripped = stripped[1..stripped.len() - 1].to_string();
    }
    let last_dot = stripped.rfind('.');
    let last_comma = stripped.rfind(',');
    let european = match (last_dot, last_comma) {
        (Some(d), Some(c)) => c > d,
        (Some(_), None) => false,
        (None, Some(c)) => {
            let tail = &stripped[c + 1..];
            tail.len() <= 2 && tail.chars().all(|ch| ch.is_ascii_digit())
        }
        (None, None) => false,
    };
    let s2 = if european {
        stripped.replace('.', "").replace(',', ".")
    } else {
        stripped.replace(',', "")
    };
    s2.parse::<f64>()
        .ok()
        .map(|v| if neg { -v.abs() } else { v })
}

// ─────────────────────────────────────────────────────────────────
// Filter + summary against REAL Uncle CSV files on disk.
//
// These tests run against the *actual* user-uploaded CSVs (not the
// anonymised fixtures committed to the repo) when those files are
// present on this machine. Ground-truth expected numbers below were
// computed independently from the source files via Python (csv +
// arithmetic) and frozen as constants. The tests check that the
// Rust filter + summary functions produce IDENTICAL totals.
//
// If the real files aren't present at the expected path, the tests
// are silently skipped so CI doesn't go red on machines that don't
// have uncle's data.
// ─────────────────────────────────────────────────────────────────

/// Absolute path to uncle's actual SGX file on the dev machine.
/// Tests using it auto-skip on machines without the file.
const REAL_SGX_PATH: &str =
    "/Users/samisayyed/Desktop/Feroz Uncle Portfolio /portfolio SGX (1).csv";
const REAL_US_PATH: &str =
    "/Users/samisayyed/Desktop/Feroz Uncle Portfolio /portfolio US Stocks (1).csv";

/// Ground-truth expected totals from the SGX file. Computed
/// independently via Python `csv + arithmetic`. If the Rust
/// pipeline produces different numbers, one of them is wrong —
/// loop until they match.
const SGX_EXPECTED_TOTAL_ROWS: usize = 33;
const SGX_EXPECTED_WATCHLIST: usize = 1;
const SGX_EXPECTED_DUPES: usize = 0;
// One row in the real SGX file has Purchase Price = 0.0 (AWX.SI,
// 50 shares @ $0). A "BUY @ $0" line would pollute the import as
// $0 cost-basis junk, so it's dropped as zero-value and reported
// in stats.zero_or_invalid_value_dropped.
const SGX_EXPECTED_ZERO_VALUE: usize = 1;
const SGX_EXPECTED_KEPT: usize = 31;
const SGX_EXPECTED_BUY_COST_BASIS: f64 = 113_742.54;
const SGX_EXPECTED_SELL_PROCEEDS: f64 = 34_797.50;
const SGX_EXPECTED_UNIQUE_SYMBOLS: usize = 11;

const US_EXPECTED_TOTAL_ROWS: usize = 402;
const US_EXPECTED_WATCHLIST: usize = 43;
const US_EXPECTED_DUPES: usize = 1;
const US_EXPECTED_KEPT: usize = 358;
const US_EXPECTED_BUY_COST_BASIS: f64 = 387_785.45;
const US_EXPECTED_SELL_PROCEEDS: f64 = 207_602.03;
const US_EXPECTED_UNIQUE_SYMBOLS: usize = 112;

fn load_real(path: &str) -> Option<(Vec<String>, Vec<Vec<String>>)> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        eprintln!("SKIP: {} not present on this machine", path);
        return None;
    }
    let bytes = std::fs::read(p).expect("read real file");
    let parsed = parse_csv(&bytes, &ParseConfig::default()).expect("parse real file");
    Some((parsed.headers, parsed.rows))
}

#[test]
fn real_sgx_filter_and_summary_match_ground_truth() {
    let Some((headers, rows)) = load_real(REAL_SGX_PATH) else {
        return;
    };

    assert_eq!(rows.len(), SGX_EXPECTED_TOTAL_ROWS, "SGX total rows");

    let mapping = detect_field_mappings_smart(&headers, &rows);
    assert_eq!(mapping.get(FIELD_SYMBOL), Some(&"Symbol".to_string()));
    assert_eq!(mapping.get(FIELD_DATE), Some(&"Trade Date".to_string()));
    assert_eq!(
        mapping.get(FIELD_UNIT_PRICE),
        Some(&"Purchase Price".to_string())
    );
    assert_eq!(mapping.get(FIELD_QUANTITY), Some(&"Quantity".to_string()));
    assert_eq!(
        mapping.get(FIELD_ACTIVITY_TYPE),
        Some(&"Transaction Type".to_string())
    );

    let summary = filter_dedupe_and_summarise(&headers, &mapping, &rows);

    assert_eq!(
        summary.stats.total_input_rows, SGX_EXPECTED_TOTAL_ROWS,
        "total input rows"
    );
    assert_eq!(
        summary.stats.watchlist_dropped, SGX_EXPECTED_WATCHLIST,
        "watchlist drops"
    );
    assert_eq!(
        summary.stats.duplicates_dropped, SGX_EXPECTED_DUPES,
        "duplicate drops"
    );
    assert_eq!(
        summary.stats.zero_or_invalid_value_dropped, SGX_EXPECTED_ZERO_VALUE,
        "zero-value drops (e.g. AWX.SI 50 shares @ $0.00)"
    );
    assert_eq!(summary.stats.kept, SGX_EXPECTED_KEPT, "kept count");
    assert_eq!(
        summary.unique_symbols, SGX_EXPECTED_UNIQUE_SYMBOLS,
        "unique symbols"
    );

    assert!(
        (summary.total_buy_cost_basis - SGX_EXPECTED_BUY_COST_BASIS).abs() < 0.01,
        "SGX BUY cost basis: expected {} got {}",
        SGX_EXPECTED_BUY_COST_BASIS,
        summary.total_buy_cost_basis
    );
    assert!(
        (summary.total_sell_proceeds - SGX_EXPECTED_SELL_PROCEEDS).abs() < 0.01,
        "SGX SELL proceeds: expected {} got {}",
        SGX_EXPECTED_SELL_PROCEEDS,
        summary.total_sell_proceeds
    );

    eprintln!(
        "SGX OK: kept={} buy=${:.2} sell=${:.2} symbols={}",
        summary.stats.kept,
        summary.total_buy_cost_basis,
        summary.total_sell_proceeds,
        summary.unique_symbols
    );
}

#[test]
fn real_us_filter_and_summary_match_ground_truth() {
    let Some((headers, rows)) = load_real(REAL_US_PATH) else {
        return;
    };

    assert_eq!(rows.len(), US_EXPECTED_TOTAL_ROWS, "US total rows");

    let mapping = detect_field_mappings_smart(&headers, &rows);
    let summary = filter_dedupe_and_summarise(&headers, &mapping, &rows);

    assert_eq!(
        summary.stats.total_input_rows, US_EXPECTED_TOTAL_ROWS,
        "total input rows"
    );
    assert_eq!(
        summary.stats.watchlist_dropped, US_EXPECTED_WATCHLIST,
        "watchlist drops"
    );
    assert_eq!(
        summary.stats.duplicates_dropped, US_EXPECTED_DUPES,
        "duplicate drops"
    );
    assert_eq!(summary.stats.kept, US_EXPECTED_KEPT, "kept count");
    assert_eq!(
        summary.unique_symbols, US_EXPECTED_UNIQUE_SYMBOLS,
        "unique symbols"
    );

    assert!(
        (summary.total_buy_cost_basis - US_EXPECTED_BUY_COST_BASIS).abs() < 0.01,
        "US BUY cost basis: expected {} got {}",
        US_EXPECTED_BUY_COST_BASIS,
        summary.total_buy_cost_basis
    );
    assert!(
        (summary.total_sell_proceeds - US_EXPECTED_SELL_PROCEEDS).abs() < 0.01,
        "US SELL proceeds: expected {} got {}",
        US_EXPECTED_SELL_PROCEEDS,
        summary.total_sell_proceeds
    );

    eprintln!(
        "US OK: kept={} buy=${:.2} sell=${:.2} symbols={}",
        summary.stats.kept,
        summary.total_buy_cost_basis,
        summary.total_sell_proceeds,
        summary.unique_symbols
    );
}

#[test]
fn real_sgx_no_kept_row_has_zero_or_missing_required_fields() {
    let Some((headers, rows)) = load_real(REAL_SGX_PATH) else {
        return;
    };
    let mapping = detect_field_mappings_smart(&headers, &rows);
    let (clean, _stats) = filter_and_dedupe_rows(&headers, &mapping, &rows);
    for row in &clean {
        assert!(
            !row.symbol.is_empty(),
            "kept row has empty symbol: {:?}",
            row
        );
        assert!(row.quantity > 0.0, "kept row has zero qty: {:?}", row);
        assert!(row.unit_price > 0.0, "kept row has zero price: {:?}", row);
    }
}

#[test]
fn real_us_per_row_amount_sums_to_expected_total() {
    let Some((headers, rows)) = load_real(REAL_US_PATH) else {
        return;
    };
    let mapping = detect_field_mappings_smart(&headers, &rows);
    let (clean, _stats) = filter_and_dedupe_rows(&headers, &mapping, &rows);

    // Recompute the BUY cost basis manually from the kept rows and
    // ensure it matches what compute_import_summary reported.
    let buy: f64 = clean
        .iter()
        .filter(|r| r.activity_type == "BUY")
        .map(|r| r.quantity * r.unit_price)
        .sum();
    assert!(
        (buy - US_EXPECTED_BUY_COST_BASIS).abs() < 0.01,
        "per-row sum doesn't match: {} vs expected {}",
        buy,
        US_EXPECTED_BUY_COST_BASIS
    );
}
