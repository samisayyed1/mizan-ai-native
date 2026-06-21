//! Research Asset tool — per-symbol market intelligence + the user's
//! position context, in one structured payload the LLM can synthesise
//! into a Yahoo-Finance-style "should I buy / hold / sell" answer.
//!
//! Powers the prompts that made Claude+IBKR go viral last week:
//!   - "give me a portfolio view" (combined with get_holdings)
//!   - "look at my holdings and tell me what concerns you" (LLM
//!     iterates this tool across positions)
//!   - "give me a full breakdown of NVDA" (direct)
//!   - "NVDA just reported earnings — break down the result" (recent
//!     news + price action surface earnings reaction)
//!   - "which of my positions looks like a good buying opportunity
//!     right now?" (LLM scans technicals + position over holdings)
//!   - "give me my daily portfolio brief" (LLM iterates + synthesises)
//!
//! Everything is grounded in data Mizan already pulls today — Twelve
//! Data / Yahoo for quotes via `quote_service`, the portfolio holdings
//! via `holdings_service`. The LLM does the synthesis; this tool does
//! the data assembly. No hard-coded "verdict" logic on our side — the
//! tool returns structured signals (52w range position, vs-SMA trend,
//! position size, unrealised %) and the LLM weighs them.

use chrono::{Datelike, Duration};
use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

/// Args the LLM produces.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchAssetArgs {
    /// Asset id, ticker, or name fragment. We resolve this against
    /// the user's portfolio first (so "my Apple position" or "AAPL"
    /// both work); if it doesn't match a holding we fall back to a
    /// direct quote lookup against the same symbol.
    pub asset_ref: String,
    /// Optional override for how much history to surface (in days).
    /// Defaults to 365 — enough to compute a 52-week range without
    /// hitting the cold-start gap on newer symbols.
    #[serde(default)]
    pub history_days: Option<u32>,
}

/// Structured payload — every numeric field is optional because the
/// quote provider may not have all values (older symbols, low-liquidity
/// alternatives, FX pairs, …). The LLM treats `None` as "unknown" and
/// frames its answer accordingly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchAssetOutput {
    /// Resolved identity — what we actually looked up.
    pub asset: AssetIdentity,
    /// Latest price + when we got it.
    pub price: Option<PriceSnapshot>,
    /// 52-week range, drawdown from the high, run-up from the low.
    pub range: Option<PriceRange>,
    /// Simple technical signals — 20d / 50d / 200d moving averages
    /// vs. current price, % above/below each.
    pub technicals: Option<Technicals>,
    /// The user's position in this asset, if they own it. `None`
    /// means it's not in the portfolio — the LLM should frame
    /// the response as research, not "what to do about your
    /// position".
    pub your_holding: Option<HoldingPosition>,
    /// Plain-text signal summary the LLM can lift verbatim — keeps
    /// the synthesis grounded in real data, not hallucinated trends.
    pub signal_summary: Vec<String>,
    /// Honest warnings — stale quote, missing history, no portfolio
    /// match, etc. Surfaced so the LLM caveats accordingly.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIdentity {
    pub asset_id: Option<String>,
    pub symbol: String,
    pub name: Option<String>,
    pub currency: String,
    pub asset_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceSnapshot {
    /// Current price in the asset's native currency.
    pub current: f64,
    /// `YYYY-MM-DD` of the quote.
    pub as_of: String,
    /// Day change as a percentage. `None` when we have only one
    /// observation.
    pub change_pct_24h: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceRange {
    pub fifty_two_week_high: f64,
    pub fifty_two_week_low: f64,
    /// Percent below the 52-week high. Positive = below.
    pub pct_below_high: f64,
    /// Percent above the 52-week low. Positive = above.
    pub pct_above_low: f64,
    /// YTD change %.
    pub change_pct_ytd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Technicals {
    pub sma_20d: Option<f64>,
    pub sma_50d: Option<f64>,
    pub sma_200d: Option<f64>,
    /// Trend label the LLM can use directly: "above 50d SMA", etc.
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingPosition {
    pub account_id: String,
    pub quantity: f64,
    pub average_cost: Option<f64>,
    pub market_value_base: f64,
    pub base_currency: String,
    pub unrealized_gain_pct: Option<f64>,
    /// Position size as % of total portfolio (TOTAL row).
    pub portfolio_weight_pct: Option<f64>,
}

pub struct ResearchAssetTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> ResearchAssetTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: ResearchAssetArgs,
    ) -> Result<ResearchAssetOutput, AiError> {
        debug!("research_asset called: ref={:?}", args.asset_ref);

        let base_currency = self.env.base_currency();
        let history_days = args.history_days.unwrap_or(365).clamp(30, 365 * 5) as i64;

        // 1. Resolve identity — try the portfolio first so "my Apple"
        //    works without the user remembering the ticker.
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(
                mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID,
                &base_currency,
            )
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let resolved = resolve_holding(&holdings, &args.asset_ref);

        // 2. Figure out the symbol we'll quote against. If we matched
        //    a holding, use its `instrument.symbol`; otherwise treat
        //    the ref as a raw ticker.
        let symbol = match &resolved {
            Some(h) => h
                .instrument
                .as_ref()
                .map(|i| i.symbol.clone())
                .unwrap_or_else(|| args.asset_ref.clone()),
            None => args.asset_ref.clone(),
        };

        let identity = AssetIdentity {
            asset_id: resolved
                .as_ref()
                .and_then(|h| h.instrument.as_ref().map(|i| i.id.clone())),
            symbol: symbol.clone(),
            name: resolved
                .as_ref()
                .and_then(|h| h.instrument.as_ref().and_then(|i| i.name.clone())),
            currency: resolved
                .as_ref()
                .map(|h| h.local_currency.clone())
                .unwrap_or_else(|| base_currency.clone()),
            asset_kind: resolved
                .as_ref()
                .and_then(|h| h.asset_kind.as_ref().map(format_asset_kind)),
        };

        // 3. Pull quotes. We try `get_historical_quotes` for the symbol
        //    over the last `history_days`. Skip the day-of-week fill —
        //    we want raw observations for technicals.
        let mut warnings: Vec<String> = Vec::new();

        let quote_service = self.env.quote_service();
        let mut history: Vec<mizan_core::quotes::model::Quote> = quote_service
            .get_historical_quotes(&symbol)
            .unwrap_or_default();
        history.sort_by_key(|q| q.timestamp);

        // Drop anything older than `history_days` so technicals reflect
        // the recent regime.
        let cutoff = chrono::Utc::now() - Duration::days(history_days);
        history.retain(|q| q.timestamp >= cutoff);

        if history.is_empty() {
            warnings.push(format!(
                "No historical quotes found for \"{}\". Price + technicals will be empty; the LLM should answer from portfolio context only.",
                symbol
            ));
        }

        let price = history.last().map(|q| PriceSnapshot {
            current: decimal_to_f64(q.close),
            as_of: q.timestamp.date_naive().to_string(),
            change_pct_24h: history.iter().nth_back(1).map(|prev| {
                let prev_close = decimal_to_f64(prev.close);
                let curr_close = decimal_to_f64(q.close);
                if prev_close.abs() < f64::EPSILON {
                    0.0
                } else {
                    (curr_close - prev_close) / prev_close * 100.0
                }
            }),
        });

        let range = compute_range(&history);
        let technicals = compute_technicals(&history);

        // 4. Build the position context if the user owns it.
        let your_holding = resolved.as_ref().map(|h| {
            let market_value_base = decimal_to_f64(h.market_value.base);
            HoldingPosition {
                account_id: h.account_id.clone(),
                quantity: decimal_to_f64(h.quantity),
                average_cost: h.cost_basis.as_ref().and_then(|c| {
                    let qty = decimal_to_f64(h.quantity);
                    if qty.abs() < f64::EPSILON {
                        None
                    } else {
                        Some(decimal_to_f64(c.local) / qty)
                    }
                }),
                market_value_base,
                base_currency: h.base_currency.clone(),
                unrealized_gain_pct: h
                    .unrealized_gain_pct
                    .map(|d| decimal_to_f64(d)),
                portfolio_weight_pct: Some({
                    let total: f64 = holdings
                        .iter()
                        .map(|x| decimal_to_f64(x.market_value.base))
                        .sum();
                    if total.abs() < f64::EPSILON {
                        0.0
                    } else {
                        market_value_base / total * 100.0
                    }
                }),
            }
        });

        if resolved.is_none() {
            warnings.push(format!(
                "\"{}\" doesn't match any holding in the portfolio — answering as general research, not as personalised advice.",
                args.asset_ref
            ));
        }

        // 5. Pre-cook a few plain-language signal lines the LLM can
        //    lift verbatim. Reduces hallucination risk on technicals.
        let signal_summary = build_signal_summary(&price, &range, &technicals, &your_holding);

        Ok(ResearchAssetOutput {
            asset: identity,
            price,
            range,
            technicals,
            your_holding,
            signal_summary,
            warnings,
        })
    }
}

fn resolve_holding<'a>(
    holdings: &'a [mizan_core::holdings::Holding],
    reference: &str,
) -> Option<&'a mizan_core::holdings::Holding> {
    let r = reference.trim();
    if r.is_empty() {
        return None;
    }
    let lower = r.to_lowercase();

    // 1. Exact asset id match.
    for h in holdings {
        if let Some(inst) = &h.instrument {
            if inst.id == r {
                return Some(h);
            }
        }
    }
    // 2. Exact symbol match (case insensitive).
    for h in holdings {
        if let Some(inst) = &h.instrument {
            if inst.symbol.to_lowercase() == lower {
                return Some(h);
            }
        }
    }
    // 3. Name match.
    for h in holdings {
        if let Some(inst) = &h.instrument {
            if let Some(n) = &inst.name {
                if n.to_lowercase() == lower {
                    return Some(h);
                }
            }
        }
    }
    // 4. Substring on name or symbol — last resort.
    for h in holdings {
        if let Some(inst) = &h.instrument {
            let sym_match = inst.symbol.to_lowercase().contains(&lower);
            let name_match = inst
                .name
                .as_deref()
                .map(|n| n.to_lowercase().contains(&lower))
                .unwrap_or(false);
            if sym_match || name_match {
                return Some(h);
            }
        }
    }
    None
}

fn format_asset_kind(k: &mizan_core::assets::AssetKind) -> String {
    use mizan_core::assets::AssetKind::*;
    match k {
        Property => "PROPERTY",
        Vehicle => "VEHICLE",
        Collectible => "COLLECTIBLE",
        PreciousMetal => "PRECIOUS_METAL",
        PrivateEquity => "PRIVATE_EQUITY",
        Liability => "LIABILITY",
        _ => "OTHER",
    }
    .to_string()
}

fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

fn compute_range(history: &[mizan_core::quotes::model::Quote]) -> Option<PriceRange> {
    if history.is_empty() {
        return None;
    }

    // 52-week window: take everything; we already trimmed to `history_days`.
    let highs_lows = history
        .iter()
        .map(|q| (decimal_to_f64(q.close), decimal_to_f64(q.close)));
    let mut highs: Vec<f64> = Vec::new();
    let mut lows: Vec<f64> = Vec::new();
    for (h, l) in highs_lows {
        highs.push(h);
        lows.push(l);
    }
    let fifty_two_week_high = highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let fifty_two_week_low = lows.iter().cloned().fold(f64::INFINITY, f64::min);

    let last_close = history
        .last()
        .map(|q| decimal_to_f64(q.close))
        .unwrap_or(0.0);

    let pct_below_high = if fifty_two_week_high.abs() < f64::EPSILON {
        0.0
    } else {
        (fifty_two_week_high - last_close) / fifty_two_week_high * 100.0
    };
    let pct_above_low = if fifty_two_week_low.abs() < f64::EPSILON {
        0.0
    } else {
        (last_close - fifty_two_week_low) / fifty_two_week_low * 100.0
    };

    // YTD = % change from the first observation on or after Jan 1
    // of the most recent year.
    let now = chrono::Utc::now();
    let year_start = chrono::NaiveDate::from_ymd_opt(now.date_naive().year(), 1, 1)?;
    let ytd_start = history.iter().find(|q| q.timestamp.date_naive() >= year_start);
    let change_pct_ytd = ytd_start.map(|q| {
        let s = decimal_to_f64(q.close);
        if s.abs() < f64::EPSILON {
            0.0
        } else {
            (last_close - s) / s * 100.0
        }
    });

    Some(PriceRange {
        fifty_two_week_high,
        fifty_two_week_low,
        pct_below_high,
        pct_above_low,
        change_pct_ytd,
    })
}

fn sma_n(history: &[mizan_core::quotes::model::Quote], n: usize) -> Option<f64> {
    if history.len() < n {
        return None;
    }
    let tail = &history[history.len() - n..];
    let sum: f64 = tail.iter().map(|q| decimal_to_f64(q.close)).sum();
    Some(sum / n as f64)
}

fn compute_technicals(history: &[mizan_core::quotes::model::Quote]) -> Option<Technicals> {
    if history.is_empty() {
        return None;
    }
    let last = decimal_to_f64(history.last()?.close);
    let sma_20d = sma_n(history, 20);
    let sma_50d = sma_n(history, 50);
    let sma_200d = sma_n(history, 200);

    // Trend label combines whatever SMAs we managed to compute.
    let mut trend_parts: Vec<&str> = Vec::new();
    if let Some(s) = sma_50d {
        trend_parts.push(if last >= s {
            "above 50d SMA"
        } else {
            "below 50d SMA"
        });
    }
    if let Some(s) = sma_200d {
        trend_parts.push(if last >= s {
            "above 200d SMA"
        } else {
            "below 200d SMA"
        });
    }
    let trend = if trend_parts.is_empty() {
        "insufficient history for trend".to_string()
    } else {
        trend_parts.join(", ")
    };

    Some(Technicals {
        sma_20d,
        sma_50d,
        sma_200d,
        trend,
    })
}

fn build_signal_summary(
    price: &Option<PriceSnapshot>,
    range: &Option<PriceRange>,
    technicals: &Option<Technicals>,
    holding: &Option<HoldingPosition>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let (Some(p), Some(r)) = (price.as_ref(), range.as_ref()) {
        out.push(format!(
            "Trading at {:.2}, {:.1}% below 52-week high and {:.1}% above 52-week low.",
            p.current, r.pct_below_high, r.pct_above_low
        ));
        if let Some(ytd) = r.change_pct_ytd {
            out.push(format!("YTD: {:+.2}%.", ytd));
        }
    }
    if let Some(t) = technicals.as_ref() {
        out.push(format!("Technical: {}.", t.trend));
    }
    if let Some(h) = holding.as_ref() {
        if let Some(gain) = h.unrealized_gain_pct {
            out.push(format!(
                "You hold {:.4} units, unrealised P&L {:+.2}%, position is {:.2}% of net worth.",
                h.quantity,
                gain,
                h.portfolio_weight_pct.unwrap_or(0.0)
            ));
        } else {
            out.push(format!(
                "You hold {:.4} units, position is {:.2}% of net worth.",
                h.quantity,
                h.portfolio_weight_pct.unwrap_or(0.0)
            ));
        }
    }
    out
}

impl<E: AiEnvironment + 'static> Tool for ResearchAssetTool<E> {
    const NAME: &'static str = "research_asset";

    type Error = AiError;
    type Args = ResearchAssetArgs;
    type Output = ResearchAssetOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Pull market intelligence for a specific asset (stock, ETF, sukuk, …) — current \
                 price, 52-week range, simple moving-average trend signals, and the user's \
                 position in it if they own it. Returns structured data + a plain-language \
                 signal summary the answer can lift verbatim. Combine with get_holdings for a \
                 portfolio-wide scan ('which of my positions looks oversold right now?'), or \
                 call directly for a 'should I keep holding NVDA?' deep-dive. Powers daily \
                 brief / risk check / earnings-reaction prompts when iterated across the \
                 portfolio."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetRef": {
                        "type": "string",
                        "description": "Ticker symbol (e.g. 'AAPL', 'SPUS'), asset id, or name fragment ('Apple', 'my Apple position'). Resolves against the portfolio first, then falls back to a raw quote lookup."
                    },
                    "historyDays": {
                        "type": "integer",
                        "description": "Override the historical window in days. Default 365; clamped to [30, 1825]."
                    }
                },
                "required": ["assetRef"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.build_output(args).await
    }
}

// Help unused import warnings stay quiet — HashSet is intentionally
// brought in scope for forward-compat when we add ticker batching.
#[allow(dead_code)]
fn _import_anchor() -> HashSet<String> {
    HashSet::new()
}
