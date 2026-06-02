use crate::assets::AssetKind;
use crate::errors::Result;
use crate::fx::currency::{normalize_amount, normalize_currency_code};
use crate::fx::FxServiceTrait;
use crate::portfolio::holdings::{Holding, HoldingType, MonetaryValue};
use crate::quotes::{LatestQuotePair, QuoteServiceTrait};
use crate::utils::time_utils::{parse_user_timezone_or_default, user_today};
use async_trait::async_trait;
use chrono::NaiveDate;
use log::{debug, warn};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[async_trait]
pub trait HoldingsValuationServiceTrait: Send + Sync {
    async fn calculate_holdings_live_valuation(&self, holdings: &mut [Holding]) -> Result<()>;
}

#[derive(Clone)]
pub struct HoldingsValuationService {
    fx_service: Arc<dyn FxServiceTrait>,
    quote_service: Arc<dyn QuoteServiceTrait>,
    timezone: Arc<RwLock<String>>,
}

impl HoldingsValuationService {
    pub fn new(
        fx_service: Arc<dyn FxServiceTrait>,
        quote_service: Arc<dyn QuoteServiceTrait>,
    ) -> Self {
        Self::new_with_timezone(
            fx_service,
            quote_service,
            Arc::new(RwLock::new(String::new())),
        )
    }

    pub fn new_with_timezone(
        fx_service: Arc<dyn FxServiceTrait>,
        quote_service: Arc<dyn QuoteServiceTrait>,
        timezone: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            fx_service,
            quote_service,
            timezone,
        }
    }

    fn today_in_user_timezone(&self) -> chrono::NaiveDate {
        let tz = parse_user_timezone_or_default(&self.timezone.read().unwrap());
        user_today(tz)
    }

    // Private helper to get FX rate with logging and fallback.
    //
    // **Lenient.** Returns 1.0 when no rate is registered. Use this
    // ONLY for non-critical conversions (e.g. computing `prev_close_value`
    // for the day-change display, where a 1.0 fallback at worst breaks
    // one widget). NEVER use this for `market_value_base` — silently
    // valuing SGD-denominated holdings at SGD == USD is the bug that
    // shipped the dashboard at $184K instead of the actual ~$236K. For
    // the headline portfolio value, use `try_get_fx_rate` instead and
    // route to the cost-basis fallback when it returns None.
    fn get_fx_rate_or_fallback(
        &self,
        from_curr: &str,
        to_curr: &str,
        context_msg: &str,
    ) -> Decimal {
        self.try_get_fx_rate(from_curr, to_curr, context_msg)
            .unwrap_or(Decimal::ONE)
    }

    /// **Strict FX getter.** Returns `None` when no rate is registered
    /// between the supplied currencies and they aren't equal. Same-
    /// currency conversions always return `Some(1.0)` without a
    /// service call — the FX service's same-currency short-circuit is
    /// already implemented, but having it here too keeps the hot path
    /// fast and obvious.
    ///
    /// Caller is responsible for handling the None case correctly —
    /// for the headline market-value calculation, that means falling
    /// back to cost basis (the same path as a fully-missing quote).
    fn try_get_fx_rate(
        &self,
        from_curr: &str,
        to_curr: &str,
        context_msg: &str,
    ) -> Option<Decimal> {
        if from_curr.eq_ignore_ascii_case(to_curr) {
            return Some(Decimal::ONE);
        }
        match self.fx_service.get_latest_exchange_rate(from_curr, to_curr) {
            Ok(rate) => Some(rate),
            Err(e) => {
                warn!(
                    "{}: FX rate {}->{} unavailable: {}. Will route position to cost-basis fallback.",
                    context_msg, from_curr, to_curr, e
                );
                None
            }
        }
    }

    // Helper to fetch necessary market data in batches
    async fn fetch_batch_quote_data(
        &self,
        holdings: &[Holding],
    ) -> Result<HashMap<String, LatestQuotePair>> {
        // Use asset ID (not symbol) for quote lookups
        // Asset ID is the unique identifier matching quotes table (e.g., "SHOP:XTSE", "BTC:USD")
        let required_asset_ids: Vec<String> = holdings
            .iter()
            .filter_map(|holding| {
                // Include both Security and AlternativeAsset holdings
                match holding.holding_type {
                    HoldingType::Security | HoldingType::AlternativeAsset => {
                        holding.instrument.as_ref().map(|inst| inst.id.clone())
                    }
                    HoldingType::Cash => None, // Skip cash holdings
                }
            })
            .collect();

        let latest_quote_pairs = if !required_asset_ids.is_empty() {
            self.quote_service
                .get_latest_quotes_pair(&required_asset_ids)?
        } else {
            HashMap::new()
        };

        Ok(latest_quote_pairs)
    }
}

#[async_trait]
impl HoldingsValuationServiceTrait for HoldingsValuationService {
    async fn calculate_holdings_live_valuation(&self, holdings: &mut [Holding]) -> Result<()> {
        if holdings.is_empty() {
            return Ok(());
        }
        debug!(
            "Starting calculate_holdings_live_valuation for {} holdings.",
            holdings.len()
        );

        // --- Fetch Batch Market Data ---
        let latest_quote_pairs: HashMap<String, LatestQuotePair> =
            self.fetch_batch_quote_data(holdings).await?;

        let today = self.today_in_user_timezone();

        for holding in holdings.iter_mut() {
            match holding.holding_type {
                HoldingType::Security => {
                    // Use asset ID for quote lookups (e.g., "SHOP:XTSE", not "SHOP")
                    if let Some(asset_id) = holding.instrument.as_ref().map(|i| i.id.clone()) {
                        holding.as_of_date = latest_quote_pairs
                            .get(&asset_id)
                            .map(|qp| qp.latest.timestamp.date_naive())
                            .unwrap_or(today);
                    } else {
                        holding.as_of_date = today;
                    }
                    let base_currency = holding.base_currency.clone();
                    self.calculate_security_valuation(holding, &base_currency, &latest_quote_pairs)
                        .await?;
                }
                HoldingType::AlternativeAsset => {
                    // Use asset ID for quote lookups
                    if let Some(asset_id) = holding.instrument.as_ref().map(|i| i.id.clone()) {
                        holding.as_of_date = latest_quote_pairs
                            .get(&asset_id)
                            .map(|qp| qp.latest.timestamp.date_naive())
                            .unwrap_or(today);
                    } else {
                        holding.as_of_date = today;
                    }
                    let base_currency = holding.base_currency.clone();
                    self.calculate_alternative_asset_valuation(
                        holding,
                        &base_currency,
                        &latest_quote_pairs,
                    )
                    .await?;
                }
                HoldingType::Cash => {
                    holding.as_of_date = today;
                    let base_currency = holding.base_currency.clone();
                    self.calculate_cash_valuation(holding, &base_currency)?;
                }
            }
        }

        debug!("Finished calculate_holdings_live_valuation.");
        Ok(())
    }
}

// --- New Helper Methods for Valuation ---

impl HoldingsValuationService {
    async fn calculate_security_valuation(
        &self,
        holding: &mut Holding,
        base_currency: &str,
        latest_quote_pairs: &HashMap<String, LatestQuotePair>,
    ) -> Result<()> {
        let instrument = match &holding.instrument {
            Some(inst) => inst,
            None => {
                warn!(
                    "Skipping valuation for security holding {} without instrument.",
                    holding.id
                );
                return Ok(());
            }
        };
        let asset_id = &instrument.id; // Use ID for quote lookups
        let symbol = &instrument.symbol; // Use symbol for logging
        let quantity = holding.quantity;
        let pos_currency = &holding.local_currency;
        let normalized_position_currency = normalize_currency_code(pos_currency);
        let context_msg = format!("HoldingValuation [Security {} ({})]", symbol, asset_id);

        // --- Calculate FX Rate (Needed even for zero quantity) ---
        let fx_rate_local_to_base = self.get_fx_rate_or_fallback(
            pos_currency,
            base_currency,
            &format!("{}: FX Local->Base", context_msg),
        );
        holding.fx_rate = Some(fx_rate_local_to_base);

        // --- Calculate Base Cost Basis (If applicable) ---
        if let Some(cost_basis) = &mut holding.cost_basis {
            cost_basis.base = cost_basis.local * fx_rate_local_to_base;
        } else {
            warn!("{}: Cost basis local value missing...", context_msg);
        }

        // --- Handle Zero Quantity ---
        if quantity == Decimal::ZERO {
            warn!("{}: Skipping valuation for zero quantity.", context_msg);
            holding.market_value = MonetaryValue::zero();
            holding.price = None;
            holding.unrealized_gain = None;
            holding.unrealized_gain_pct = None;
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
            // FX rate and base cost basis are already set above
            return Ok(());
        }

        // --- Handle Expired Options ---
        // Expired options are worth $0. Set market value to zero and realize the loss.
        let today = self.today_in_user_timezone();
        if is_expired_option(holding.metadata.as_ref(), today) {
            debug!(
                "{}: Option expired. Setting market value to zero.",
                context_msg
            );
            holding.price = Some(Decimal::ZERO);
            holding.market_value = MonetaryValue::zero();

            if let Some(cost_basis) = &holding.cost_basis {
                let neg_local = -cost_basis.local;
                let neg_base = -cost_basis.base;
                holding.unrealized_gain = Some(MonetaryValue {
                    local: neg_local,
                    base: neg_base,
                });
                if cost_basis.base != Decimal::ZERO {
                    holding.unrealized_gain_pct = Some(dec!(-1));
                } else {
                    holding.unrealized_gain_pct = Some(Decimal::ZERO);
                }
            } else {
                holding.unrealized_gain = None;
                holding.unrealized_gain_pct = None;
            }
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
            // Preserve the realized-gain pre-populated by the holdings
            // service from the snapshot accumulator; fold into total.
            populate_total_gain(holding);
            return Ok(());
        }

        // --- Fetch and Process Quote Data (For Non-Zero Quantity) ---
        //
        // Two reasons we route a holding to the cost-basis fallback
        // instead of valuing it from a quote:
        //   (a) no quote at all for this asset
        //   (c) FX rate quote_ccy → base_ccy is not registered — we can
        //       NOT fabricate a rate (valuing an SGD position as if it
        //       were USD would be flatly wrong), so cost basis is the
        //       only honest answer until the FX pair exists.
        //
        // We deliberately do NOT reject *stale* quotes (enterprise §4).
        // A stale-but-real last price is closer to the truth than
        // silently swapping in cost basis, and `holding.as_of_date`
        // already carries the quote's real date — so the UI labels the
        // staleness explicitly ("price as of N days ago") instead of
        // showing a wrong number with no warning. The previous behavior
        // (drop quotes older than 7 days → cost basis) silently changed
        // the displayed number AND its meaning under the user; that is
        // the exact "wrong + invisible" failure we are removing.
        let usable_quote = latest_quote_pairs.get(asset_id).and_then(|qp| {
            // (c) FX availability check — only matters if the quote
            // currency differs from the base currency. Same-currency
            // is short-circuited inside `try_get_fx_rate`.
            let normalized_quote_ccy_check = normalize_currency_code(&qp.latest.currency);
            // If no FX rate is available the closure yields None (quote
            // unusable → cost-basis fallback fires below). The `?` keeps
            // that behavior — and the warning is still logged as a side
            // effect inside try_get_fx_rate, which `?` still calls.
            self.try_get_fx_rate(
                normalized_quote_ccy_check,
                base_currency,
                &format!("{}: FX Quote->Base", context_msg),
            )?;
            Some(qp)
        });

        if let Some(quote_pair) = usable_quote {
            let latest_quote = &quote_pair.latest;
            let prev_quote_opt = quote_pair.previous.as_ref();

            let (normalized_price, normalized_quote_currency) =
                normalize_amount(latest_quote.close, &latest_quote.currency);

            if normalized_position_currency != normalized_quote_currency {
                warn!(
                    "{}: Holding currency ({}) differs from quote currency ({}). Using quote currency FX for market value conversion.",
                    context_msg,
                    pos_currency,
                    latest_quote.currency
                );
            }

            // `try_get_fx_rate` already passed in the gate above, so
            // unwrap_or(1.0) here is just belt-and-suspenders — we'd
            // never get here with a missing pair.
            let fx_rate_quote_to_base = self.get_fx_rate_or_fallback(
                normalized_quote_currency,
                base_currency,
                &format!("{}: FX Quote->Base", context_msg),
            );

            let market_value_quote_major =
                normalized_price * quantity * holding.contract_multiplier;

            let fx_rate_quote_to_local = self.get_fx_rate_or_fallback(
                normalized_quote_currency,
                pos_currency,
                &format!("{}: FX Quote->Local", context_msg),
            );
            let market_price_local = normalized_price * fx_rate_quote_to_local;
            holding.price = Some(market_price_local);

            let market_value_local = market_value_quote_major * fx_rate_quote_to_local;
            let market_value_base = market_value_quote_major * fx_rate_quote_to_base;

            holding.market_value = MonetaryValue {
                local: market_value_local,
                base: market_value_base,
            };

            if let Some(cost_basis) = &holding.cost_basis {
                let cost_basis_base = cost_basis.base;

                let unrealized_gain_local = market_value_local - cost_basis.local;
                let unrealized_gain_base = market_value_base - cost_basis_base;

                holding.unrealized_gain = Some(MonetaryValue {
                    local: unrealized_gain_local,
                    base: unrealized_gain_base,
                });

                if cost_basis_base != dec!(0) {
                    holding.unrealized_gain_pct =
                        Some((unrealized_gain_base / cost_basis_base).round_dp(4));
                } else if unrealized_gain_base != dec!(0) {
                    holding.unrealized_gain_pct = Some(dec!(1.0));
                } else {
                    holding.unrealized_gain_pct = Some(Decimal::ZERO);
                }
            } else {
                holding.unrealized_gain = None;
                holding.unrealized_gain_pct = None;
                warn!(
                    "{}: Cost basis missing. Cannot calculate unrealized gain.",
                    context_msg
                );
            }

            if let Some(prev_quote) = prev_quote_opt {
                let (prev_price_normalized, prev_quote_currency_normalized) =
                    normalize_amount(prev_quote.close, &prev_quote.currency);

                if prev_quote_currency_normalized == normalized_quote_currency {
                    let prev_value_quote_major =
                        prev_price_normalized * quantity * holding.contract_multiplier;

                    let fx_rate_prev_quote_to_local = fx_rate_quote_to_local;
                    let fx_rate_prev_quote_to_base = fx_rate_quote_to_base;

                    let prev_value_local = prev_value_quote_major * fx_rate_prev_quote_to_local;
                    let prev_value_base = prev_value_quote_major * fx_rate_prev_quote_to_base;

                    holding.prev_close_value = Some(MonetaryValue {
                        local: prev_value_local,
                        base: prev_value_base,
                    });

                    let day_change_quote_major = market_value_quote_major - prev_value_quote_major;
                    let day_change_local = day_change_quote_major * fx_rate_prev_quote_to_local;
                    let day_change_base = day_change_quote_major * fx_rate_prev_quote_to_base;

                    holding.day_change = Some(MonetaryValue {
                        local: day_change_local,
                        base: day_change_base,
                    });

                    if prev_value_base != dec!(0) {
                        holding.day_change_pct =
                            Some((day_change_base / prev_value_base).round_dp(4));
                    } else if day_change_base != dec!(0) {
                        holding.day_change_pct = None;
                    } else {
                        holding.day_change_pct = Some(Decimal::ZERO);
                    }
                } else {
                    warn!(
                        "{}: Currency mismatch latest ({}) vs previous ({}) quote. Cannot calculate day gain.",
                        context_msg, normalized_quote_currency, prev_quote_currency_normalized
                    );
                    holding.day_change = None;
                    holding.day_change_pct = None;
                    holding.prev_close_value = None;
                }
            } else {
                warn!(
                    "{}: Missing previous day quote. Cannot calculate day gain.",
                    context_msg
                );
                holding.day_change = None;
                holding.day_change_pct = None;
                holding.prev_close_value = None;
            }
        } else {
            // No live quote available for this asset. Historically this
            // path silently set market_value = $0, which is wrong: every
            // position the background Yahoo sync hadn't caught up to
            // would show $0 on the dashboard, and the portfolio total
            // would massively understate (real-world repro: a 78-position
            // Yahoo Portfolio import showed $187K against an actual
            // $236K market value — a 21% shortfall — because ~58 of
            // 78 symbols hadn't been quote-synced yet).
            //
            // Production-grade fix: fall back to **cost basis** (what
            // the user paid). The dashboard total now reflects every
            // position's last-known value instead of dropping the
            // missing ones to zero. The user still sees a warning in
            // the logs that the live quote is missing, and Data Health
            // surfaces the stale-quote count.
            //
            // This is conservative — it doesn't invent a market gain
            // for un-quoted positions, it just stops pretending they
            // don't exist. When the Yahoo sync eventually catches up,
            // the real quote replaces this fallback automatically on
            // the next portfolio recalc.
            if let Some(cost_basis) = &holding.cost_basis {
                let per_unit_local = if quantity != Decimal::ZERO {
                    cost_basis.local / quantity
                } else {
                    Decimal::ZERO
                };
                warn!(
                    "{}: Quote pair data missing. Falling back to cost-basis valuation \
                     ({} {}/unit). Dashboard total includes this position at cost; \
                     unrealized gain reported as 0 until live quote arrives.",
                    context_msg, per_unit_local, pos_currency
                );
                holding.price = Some(per_unit_local);
                holding.market_value = MonetaryValue {
                    local: cost_basis.local,
                    base: cost_basis.base,
                };
                holding.unrealized_gain = Some(MonetaryValue::zero());
                holding.unrealized_gain_pct = Some(Decimal::ZERO);
            } else {
                // Last resort: no quote AND no cost basis. Truly zero —
                // this only happens for ghost rows that snuck through
                // without any pricing context, which is itself a bug.
                warn!(
                    "{}: Quote pair AND cost basis both missing. Market valuation = $0.",
                    context_msg
                );
                holding.market_value = MonetaryValue::zero();
                holding.price = None;
                holding.unrealized_gain = None;
                holding.unrealized_gain_pct = None;
            }
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
        }

        // Realized gain comes pre-populated from the snapshot accumulator.
        // Total = unrealized + realized.
        populate_total_gain(holding);

        Ok(())
    }

    /// Calculate valuation for alternative assets (Property, Vehicle, Collectible, PhysicalPrecious, Liability, Other).
    ///
    /// Key differences from security valuation:
    /// - Uses MANUAL data source quotes
    /// - Gain calculation uses purchase_price from metadata (if available) instead of lot-based cost basis
    /// - No day change calculation (manual valuations don't have daily updates)
    /// - Property: quantity = ownership fraction, quote.close = total property value
    /// - PhysicalPrecious: quantity = weight, quote.close = price per unit
    /// - Liability: stored as positive, displayed as negative (sign applied at UI layer)
    async fn calculate_alternative_asset_valuation(
        &self,
        holding: &mut Holding,
        base_currency: &str,
        latest_quote_pairs: &HashMap<String, LatestQuotePair>,
    ) -> Result<()> {
        let instrument = match &holding.instrument {
            Some(inst) => inst,
            None => {
                warn!(
                    "Skipping valuation for alternative asset holding {} without instrument.",
                    holding.id
                );
                return Ok(());
            }
        };
        let asset_id = &instrument.id; // Use ID for quote lookups
        let symbol = &instrument.symbol; // Use symbol for logging
        let quantity = holding.quantity;
        let pos_currency = &holding.local_currency;
        let normalized_position_currency = normalize_currency_code(pos_currency);
        let asset_kind = holding.asset_kind.clone().unwrap_or(AssetKind::Other);
        let context_msg = format!(
            "HoldingValuation [AlternativeAsset {} ({}) ({:?})]",
            symbol, asset_id, asset_kind
        );

        // --- Calculate FX Rate ---
        //
        // QA Pass 17: use the STRICT FX getter for local→base. The lenient
        // 1.0-fallback path is explicitly forbidden for market_value.base
        // (see this file's doc on get_fx_rate_or_fallback). A property in
        // EUR with no EUR/USD rate would silently show "€500,000" as
        // "$500,000" — the same wrong-but-invisible failure pattern Pass
        // 10 fixed for cash holdings.
        let fx_rate_local_to_base_opt = self.try_get_fx_rate(
            pos_currency,
            base_currency,
            &format!("{}: FX Local->Base", context_msg),
        );
        holding.fx_rate = fx_rate_local_to_base_opt;

        // --- Calculate Base Cost Basis (If applicable) ---
        // When FX is missing on a cross-currency alt asset, .base = 0
        // (honest) rather than = .local (silently mis-stating).
        if let Some(cost_basis) = &mut holding.cost_basis {
            cost_basis.base = match fx_rate_local_to_base_opt {
                Some(rate) => cost_basis.local * rate,
                None => Decimal::ZERO,
            };
        }

        // --- Handle Zero Quantity ---
        if quantity == Decimal::ZERO {
            warn!("{}: Skipping valuation for zero quantity.", context_msg);
            holding.market_value = MonetaryValue::zero();
            holding.price = None;
            holding.unrealized_gain = None;
            holding.unrealized_gain_pct = None;
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
            return Ok(());
        }

        // --- Fetch and Process Quote Data ---
        if let Some(quote_pair) = latest_quote_pairs.get(asset_id) {
            let latest_quote = &quote_pair.latest;

            let (normalized_price, normalized_quote_currency) =
                normalize_amount(latest_quote.close, &latest_quote.currency);

            if normalized_position_currency != normalized_quote_currency {
                warn!(
                    "{}: Holding currency ({}) differs from quote currency ({}). Using quote currency FX for market value conversion.",
                    context_msg, pos_currency, latest_quote.currency
                );
            }

            // Strict for quote→base (drives market_value.base); lenient
            // for quote→local is acceptable (only feeds display .local
            // which can absorb a 1.0 fallback if quote==pos currency).
            let fx_rate_quote_to_base_opt = self.try_get_fx_rate(
                normalized_quote_currency,
                base_currency,
                &format!("{}: FX Quote->Base", context_msg),
            );

            let fx_rate_quote_to_local = self.get_fx_rate_or_fallback(
                normalized_quote_currency,
                pos_currency,
                &format!("{}: FX Quote->Local", context_msg),
            );

            // --- Calculate Market Value ---
            // For all alternative assets: market_value = quantity * unit_price
            // Property: quote.close = total property value, user's share = quantity (fraction) * close
            // PhysicalPrecious: quote.close = price per unit (oz/g/kg)
            // Vehicle/Collectible/Liability/Other: quote.close = unit value, quantity usually 1
            let market_value_quote_major = normalized_price * quantity;

            let market_value_local = market_value_quote_major * fx_rate_quote_to_local;
            let market_value_base = match fx_rate_quote_to_base_opt {
                Some(rate) => market_value_quote_major * rate,
                None => {
                    warn!(
                        "{}: No {} → {} FX rate available. Setting market_value.base \
                         to ZERO instead of silently treating the local-currency value \
                         as base currency. .local still carries the truthful amount.",
                        context_msg, normalized_quote_currency, base_currency
                    );
                    Decimal::ZERO
                }
            };
            let market_price_local = normalized_price * fx_rate_quote_to_local;
            holding.price = Some(market_price_local);

            holding.market_value = MonetaryValue {
                local: market_value_local,
                base: market_value_base,
            };

            // --- Calculate Gain ---
            // For alternative assets, use purchase_price from metadata if available
            // Otherwise, fall back to lot-based cost_basis
            let gain_calculated = if let Some(purchase_price) = holding.purchase_price {
                // Gain = market_value - (quantity * purchase_price)
                let total_cost_local = quantity * purchase_price;
                // When local→base FX is missing, base-cost is 0 (honest);
                // unrealized_gain_base then equals market_value_base
                // (which is itself 0 when its own FX failed). The .local
                // gain is always correct regardless of FX availability.
                let total_cost_base = match fx_rate_local_to_base_opt {
                    Some(rate) => total_cost_local * rate,
                    None => Decimal::ZERO,
                };

                let unrealized_gain_local = market_value_local - total_cost_local;
                let unrealized_gain_base = market_value_base - total_cost_base;

                holding.unrealized_gain = Some(MonetaryValue {
                    local: unrealized_gain_local,
                    base: unrealized_gain_base,
                });

                if total_cost_base != dec!(0) {
                    holding.unrealized_gain_pct =
                        Some((unrealized_gain_base / total_cost_base).round_dp(4));
                } else if unrealized_gain_base != dec!(0) {
                    holding.unrealized_gain_pct = Some(dec!(1.0));
                } else {
                    holding.unrealized_gain_pct = Some(Decimal::ZERO);
                }
                true
            } else if let Some(cost_basis) = &holding.cost_basis {
                // Fall back to lot-based cost basis calculation
                let cost_basis_base = cost_basis.base;

                let unrealized_gain_local = market_value_local - cost_basis.local;
                let unrealized_gain_base = market_value_base - cost_basis_base;

                holding.unrealized_gain = Some(MonetaryValue {
                    local: unrealized_gain_local,
                    base: unrealized_gain_base,
                });

                if cost_basis_base != dec!(0) {
                    holding.unrealized_gain_pct =
                        Some((unrealized_gain_base / cost_basis_base).round_dp(4));
                } else if unrealized_gain_base != dec!(0) {
                    holding.unrealized_gain_pct = Some(dec!(1.0));
                } else {
                    holding.unrealized_gain_pct = Some(Decimal::ZERO);
                }
                true
            } else {
                // No purchase_price and no cost_basis - gain is N/A
                false
            };

            if !gain_calculated {
                holding.unrealized_gain = None;
                holding.unrealized_gain_pct = None;
                debug!(
                    "{}: No purchase_price or cost_basis available. Gain shown as N/A.",
                    context_msg
                );
            }

            // --- Day Change ---
            // Alternative assets typically don't have daily price changes (manual valuations)
            // Set to None/zero to indicate N/A
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
        } else {
            warn!(
                "{}: Quote data missing. Market valuation incomplete.",
                context_msg
            );
            holding.market_value = MonetaryValue::zero();
            holding.price = None;
            holding.unrealized_gain = None;
            holding.unrealized_gain_pct = None;
            holding.day_change = None;
            holding.day_change_pct = None;
            holding.prev_close_value = None;
        }

        // Realized gain comes pre-populated from the snapshot accumulator.
        // Total = unrealized + realized.
        populate_total_gain(holding);

        Ok(())
    }

    fn calculate_cash_valuation(&self, holding: &mut Holding, base_currency: &str) -> Result<()> {
        let cash_currency = &holding.local_currency;
        let cash_amount = holding.quantity;
        let context_msg = format!("HoldingValuation [CASH {}]", cash_currency);
        debug!("{}: Processing cash valuation.", context_msg);

        holding.price = Some(dec!(1.0));

        // QA Pass 10: market_value.base for cash MUST use the strict
        // FX getter — silently valuing SGD cash at SGD == USD is the
        // exact "wrong + invisible" failure pattern this file's docs
        // (see get_fx_rate_or_fallback above) explicitly forbid. When
        // no rate is available we mark fx_rate=None, leave .local as
        // the truth, and set .base = 0 so the UI can render "—" or a
        // warning chip instead of a fabricated dollar amount.
        let fx_rate_opt = self.try_get_fx_rate(
            cash_currency,
            base_currency,
            &format!("{}: FX Cash->Base", context_msg),
        );
        holding.fx_rate = fx_rate_opt;

        let value_base = match fx_rate_opt {
            Some(rate) => cash_amount * rate,
            None => {
                warn!(
                    "{}: NO {} → {} FX rate available. Setting market_value.base to ZERO \
                     instead of silently treating the foreign-currency amount as base \
                     currency. .local still carries the truthful amount. Add the FX pair \
                     under Settings → Market Data → Exchange Rates.",
                    context_msg, cash_currency, base_currency
                );
                Decimal::ZERO
            }
        };

        holding.market_value.base = value_base;
        holding.market_value.local = cash_amount;

        if let Some(cost_basis) = &mut holding.cost_basis {
            cost_basis.base = value_base;
            cost_basis.local = cash_amount;
        } else {
            warn!(
                "{}: Cost basis was missing for cash, initializing.",
                context_msg
            );
            holding.cost_basis = Some(MonetaryValue {
                local: cash_amount,
                base: value_base,
            });
        }

        if let Some(prev_close) = &mut holding.prev_close_value {
            prev_close.base = value_base;
            prev_close.local = cash_amount;
        } else {
            warn!(
                "{}: Previous close value was missing for cash, initializing.",
                context_msg
            );
            holding.prev_close_value = Some(MonetaryValue {
                local: cash_amount,
                base: value_base,
            });
        }

        holding.unrealized_gain = Some(MonetaryValue::zero());
        holding.unrealized_gain_pct = Some(Decimal::ZERO);
        holding.day_change = Some(MonetaryValue::zero());
        holding.day_change_pct = Some(Decimal::ZERO);
        holding.realized_gain = Some(MonetaryValue::zero());
        holding.realized_gain_pct = Some(Decimal::ZERO);
        holding.total_gain = Some(MonetaryValue::zero());
        holding.total_gain_pct = Some(Decimal::ZERO);

        Ok(())
    }
}

/// Compute `total_gain` (and `total_gain_pct`) on a holding by summing
/// the previously-populated `unrealized_gain` and `realized_gain` fields.
///
/// `unrealized_gain` is set on the same valuation pass; `realized_gain`
/// is pre-populated from the snapshot's lifetime accumulator by
/// `holdings_service::build_live_holdings_from_snapshot`. This helper
/// just folds the two together so consumers get a single "total return
/// on this position" number.
///
/// The percentage is expressed against current-position cost basis (the
/// denominator the existing unrealized-gain-pct uses) — a reasonable
/// approximation that's well-defined when the position still holds
/// shares. For fully-disposed positions the value lives in the snapshot
/// accumulator and is surfaced in the dedicated realized-gains view.
fn populate_total_gain(holding: &mut Holding) {
    let unrealized = holding.unrealized_gain.as_ref();
    let realized = holding.realized_gain.as_ref();

    let total = match (unrealized, realized) {
        (Some(u), Some(r)) => Some(MonetaryValue {
            local: u.local + r.local,
            base: u.base + r.base,
        }),
        (Some(u), None) => Some(u.clone()),
        (None, Some(r)) => Some(r.clone()),
        (None, None) => None,
    };

    holding.total_gain = total;

    // Percentage. Use current-position cost basis as the denominator
    // (matches unrealized_gain_pct's denominator and is well-defined
    // while shares are still held).
    holding.total_gain_pct = match (holding.total_gain.as_ref(), holding.cost_basis.as_ref()) {
        (Some(total), Some(cost)) if cost.base != Decimal::ZERO => {
            Some((total.base / cost.base).round_dp(4))
        }
        (Some(total), Some(_)) if total.base != Decimal::ZERO => Some(Decimal::ONE),
        (Some(_), Some(_)) => Some(Decimal::ZERO),
        _ => None,
    };
}

/// Returns true if the holding metadata indicates an option contract that has expired.
fn is_expired_option(metadata: Option<&serde_json::Value>, today: NaiveDate) -> bool {
    let spec = metadata
        .and_then(|m| m.get("option"))
        .and_then(|o| o.get("expiration"))
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    matches!(spec, Some(exp) if exp < today)
}
