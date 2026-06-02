//! Estimated historical portfolio valuation from current holdings + quote
//! history.
//!
//! When a broker integration only delivers a current-snapshot view (no
//! lot-level acquisition dates, no transaction history), Mizan can still
//! draw a meaningful historical chart by **assuming the user held their
//! current positions throughout** and pricing them with whatever quote
//! history the market-data layer has accumulated.
//!
//! ```text
//! estimated_value(D) = Σ ( current_qty[asset] × quote(asset, D) × fx(asset_ccy → base, D) )
//!                       across assets that have a quote on D
//! ```
//!
//! The result is **clearly labeled as estimated**. It is *not* the user's
//! actual historical portfolio value — it ignores past contributions,
//! distributions, dividends, splits, and rebalancing. But it gives a
//! recognisable shape (e.g. "VTI was at $90 in 2020 vs $250 today, so
//! my chart slopes up roughly the way the index has") that's far more
//! useful than a chart that just stops at the day the user first synced.
//!
//! For users with full activity history (CSV import, manual entry, or a
//! broker integration that returns lot-level data), the regular
//! transaction-driven snapshot calculator produces a more accurate chart
//! and this module's output is unnecessary. The two paths are
//! complementary.
//!
//! # Crate boundary (Track H PR-H3.d)
//!
//! Extracted out of `mizan-core::portfolio::synthesis` so the coverage
//! floor + mutation-testing rules can be enforced per crate.
//!
//! ⚠️ This crate is **not yet a leaf** — `synthesize_account_history`
//! still depends on six mizan-core surfaces:
//!   - `mizan_core::errors::{Error, Result, CalculatorError}`
//!   - `mizan_core::fx::FxServiceTrait`
//!   - `mizan_core::portfolio::holdings::{Holding, HoldingType}`
//!   - `mizan_core::portfolio::split_adjustment::{split_adjusted_close, SplitFactors}`
//!   - `mizan_core::portfolio::valuation::DailyAccountValuation`
//!   - `mizan_core::quotes::QuoteServiceTrait`
//!
//! Lifting these into `mizan-domain-types` + a future
//! `mizan-portfolio-traits` crate is the broader Track H sweep tracked
//! as PR-H3.d.1 / PR-H3.d.2. That sweep is intentionally larger than a
//! single PR — the holdings model is the cross-cutting type the entire
//! portfolio engine depends on. See ADR 0003 for the leaf-crate plan.

use mizan_core::errors::{Error, Result};
use mizan_core::fx::FxServiceTrait;
use mizan_core::portfolio::holdings::{Holding, HoldingType};
use mizan_core::portfolio::split_adjustment::{split_adjusted_close, SplitFactors};
use mizan_core::portfolio::valuation::DailyAccountValuation;
use mizan_core::quotes::QuoteServiceTrait;

use chrono::{NaiveDate, TimeZone, Utc};
use log::{debug, warn};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Synthesise daily portfolio valuations for an account from its current
/// holdings and the quote history available for those holdings.
///
/// `holdings` should be the *current* live holdings for the account (as
/// returned by HoldingsService::get_holdings).
///
/// `account_currency` is the account's reporting currency. `base_currency`
/// is the portfolio's base currency. `today` is the date the function
/// should treat as the upper bound (typically today in the user's local
/// timezone).
///
/// Returns one `DailyAccountValuation` row per date in `[earliest_quote ..
/// today]` where at least 50% of the holdings have a quote on that date.
/// Below that coverage threshold the row is omitted to avoid drawing a
/// misleadingly low value when most positions are missing prices.
///
/// Cash holdings are added at face value (cash is presumed stable; we
/// don't try to back out historical cash balances from current holdings —
/// that's not recoverable without activities).
#[allow(clippy::too_many_arguments)]
pub async fn synthesize_account_history(
    account_id: &str,
    account_currency: &str,
    base_currency: &str,
    holdings: &[Holding],
    quote_service: &dyn QuoteServiceTrait,
    fx_service: &dyn FxServiceTrait,
    today: NaiveDate,
    split_factors: &SplitFactors,
) -> Result<Vec<DailyAccountValuation>> {
    if holdings.is_empty() {
        return Ok(Vec::new());
    }

    // Collect non-cash, non-alternative positions with positive quantity.
    // Cash and alternatives don't have meaningful quote history.
    let priced_holdings: Vec<&Holding> = holdings
        .iter()
        .filter(|h| matches!(h.holding_type, HoldingType::Security))
        .filter(|h| h.quantity > Decimal::ZERO)
        .collect();

    // Fixed cash floor: sum each cash holding's face value, FX-converted
    // to account currency. Constant across history (we can't reconstruct
    // historical cash without activities).
    //
    // QA Pass 7 fail-loud-skip: when a cash holding's local currency
    // cannot be converted, EXCLUDE it from the floor rather than silently
    // adding the raw foreign-currency amount as if it were account
    // currency (an SGD 50k cash line silently became "USD 50k" on the
    // synthesised history chart pre-fix). The synthesis is an estimate
    // already — under-counting one cash line is far better than a
    // factor-of-2 mis-statement.
    let cash_acct_ccy: Decimal = holdings
        .iter()
        .filter(|h| matches!(h.holding_type, HoldingType::Cash))
        .filter_map(|h| {
            if h.local_currency == account_currency {
                Some(h.quantity)
            } else {
                match fx_service.convert_currency(h.quantity, &h.local_currency, account_currency) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(
                            "Synthesis ({}): excluding cash holding {} {} from history floor — \
                             no {} → {} rate available ({}). Add the FX pair under \
                             Settings → Market Data → Exchange Rates.",
                            account_id,
                            h.quantity,
                            h.local_currency,
                            h.local_currency,
                            account_currency,
                            e
                        );
                        None
                    }
                }
            }
        })
        .sum();

    if priced_holdings.is_empty() {
        return Ok(Vec::new());
    }

    // For each held asset: pull the full quote history (going back to
    // 2000 covers any retail investor's career — quotes table is small
    // per asset, one row per trading day) and index by date for fast
    // per-day lookup below.
    let total_holdings = priced_holdings.len();
    let symbols: HashSet<String> = priced_holdings
        .iter()
        .filter_map(|h| h.instrument.as_ref().map(|i| i.id.clone()))
        .collect();

    if symbols.is_empty() {
        return Ok(Vec::new());
    }

    let earliest_query = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let all_quotes = match quote_service.get_quotes_in_range(&symbols, earliest_query, today) {
        Ok(q) => q,
        Err(e) => {
            warn!(
                "Synthesis: failed to load quote history for {} held assets: {}",
                symbols.len(),
                e
            );
            return Err(Error::Calculation(
                mizan_core::errors::CalculatorError::Calculation(format!(
                    "Could not load quote history: {}",
                    e
                )),
            ));
        }
    };

    let mut quote_index: HashMap<String, BTreeMap<NaiveDate, Decimal>> = HashMap::new();
    let mut overall_earliest: Option<NaiveDate> = None;
    for q in all_quotes {
        let d = q.timestamp.date_naive();
        let entry = quote_index.entry(q.asset_id.clone()).or_default();
        entry.insert(d, q.close);
        overall_earliest = Some(overall_earliest.map_or(d, |o| o.min(d)));
    }

    let Some(start_date) = overall_earliest else {
        return Ok(Vec::new());
    };

    // Coverage threshold: skip a date if fewer than half the priced
    // holdings have a quote on it. Below that, the synthesised total
    // would underestimate badly and look like a fake drawdown.
    let coverage_threshold = total_holdings.div_ceil(2);

    let mut output: Vec<DailyAccountValuation> = Vec::new();
    let mut date = start_date;
    let calculated_at = Utc::now();
    while date <= today {
        let mut investment_value_acct = Decimal::ZERO;
        let mut covered = 0usize;

        for holding in &priced_holdings {
            let Some(asset_id) = holding.instrument.as_ref().map(|i| i.id.as_str()) else {
                continue;
            };
            let Some(asset_quotes) = quote_index.get(asset_id) else {
                continue;
            };

            // Find the most recent quote on or before `date` (handles
            // weekends and market holidays gracefully).
            let Some((_, close)) = asset_quotes.range(..=date).next_back() else {
                continue;
            };

            // `holding.quantity` is the *current* share count, so a pre-split
            // historical close must be expressed in the same current-share
            // basis or the estimate inflates by the split ratio.
            let adjusted_close = match split_factors.get(asset_id) {
                Some(splits) => split_adjusted_close(*close, splits, date),
                None => *close,
            };

            let multiplier = if holding.contract_multiplier > Decimal::ZERO {
                holding.contract_multiplier
            } else {
                Decimal::ONE
            };
            let value_local = holding.quantity * adjusted_close * multiplier;

            // QA Pass 7 fail-loud-skip: when neither dated nor spot FX
            // is available, exclude this position from the daily total
            // rather than booking a raw foreign-currency magnitude as if
            // it were account currency. The `covered` count is the
            // coverage gate downstream — a skipped position simply pushes
            // the day below threshold and out of the synthesised series,
            // which is the right behaviour for "we don't have enough
            // data to compute this day honestly".
            let value_acct = if holding.local_currency == account_currency {
                Some(value_local)
            } else {
                match fx_service.convert_currency_for_date(
                    value_local,
                    &holding.local_currency,
                    account_currency,
                    date,
                ) {
                    Ok(v) => Some(v),
                    Err(_) => fx_service
                        .convert_currency(value_local, &holding.local_currency, account_currency)
                        .ok(),
                }
            };

            if let Some(v) = value_acct {
                investment_value_acct += v;
                covered += 1;
            }
            // (skipped positions intentionally do not increment `covered`
            //  so the coverage gate below filters out incoherent days.)
        }

        if covered >= coverage_threshold {
            let total_acct = investment_value_acct + cash_acct_ccy;
            // QA Pass 7 fail-loud-skip: if we cannot resolve the
            // account→base rate for this date, skip the day entirely
            // rather than booking a 1:1 fallback that silently turns a
            // SAR account's value into "$X" instead of "$X / 3.75". The
            // synthesised series can have gaps; it must never lie.
            let fx_rate_to_base = if account_currency == base_currency {
                Decimal::ONE
            } else {
                match fx_service.get_exchange_rate_for_date(account_currency, base_currency, date) {
                    Ok(r) => r,
                    Err(e) => {
                        debug!(
                            "Synthesis ({}): skipping {} — no {} → {} rate ({}).",
                            account_id, date, account_currency, base_currency, e
                        );
                        let Some(next) = date.succ_opt() else { break };
                        date = next;
                        continue;
                    }
                }
            };

            output.push(DailyAccountValuation {
                id: format!("SYNTH-{}-{}", account_id, date),
                account_id: account_id.to_string(),
                valuation_date: date,
                account_currency: account_currency.to_string(),
                base_currency: base_currency.to_string(),
                fx_rate_to_base,
                cash_balance: cash_acct_ccy,
                investment_market_value: investment_value_acct,
                total_value: total_acct,
                cost_basis: Decimal::ZERO, // Unknown without activities; chart ignores.
                net_contribution: Decimal::ZERO,
                calculated_at: Utc.from_utc_datetime(&calculated_at.naive_utc()),
            });
        }

        let Some(next) = date.succ_opt() else {
            break;
        };
        date = next;
    }

    debug!(
        "Synthesis: account {} produced {} estimated valuations from {} held positions ({} cash holdings folded in)",
        account_id,
        output.len(),
        priced_holdings.len(),
        holdings.iter().filter(|h| matches!(h.holding_type, HoldingType::Cash)).count(),
    );

    if output.is_empty() {
        return Err(Error::Calculation(mizan_core::errors::CalculatorError::Calculation(
            "No quote history available for held assets — cannot estimate historical value. Try \"Recalculate History\" first to backfill quotes.".to_string(),
        )));
    }

    Ok(output)
}
