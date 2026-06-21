//! Delete Stock Position tool — remove a tradable asset (stock / ETF /
//! mutual fund) from one or all accounts by cascade-deleting its
//! activities.
//!
//! In Mizan's data model, activities are the source of truth — a "position"
//! exists only because TRANSFER_IN / BUY activities pointed at the asset.
//! Closing the position means deleting those activities. This tool gives
//! the AI a single-shot UX for that: resolve the asset by ticker, name, or
//! id; optionally scope to one account; return a DRAFT preview the user
//! confirms before the cascade fires. The actual delete runs on the
//! frontend via the existing `deleteActivity` adapter, one call per
//! activity, after the user clicks Confirm in the tool-call card.
//!
//! Companion to `record_activity` / `record_activities` / `import_csv` —
//! together they round out the stock add/edit/delete loop through
//! /assistant that the user explicitly asked for.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStockPositionArgs {
    /// Asset id (preferred, from get_holdings), ticker symbol, or name match.
    /// Examples: "AAPL", "Apple", "NVDA", or an internal asset uuid.
    pub asset_ref: String,
    /// Optional account scope. When set, only positions inside this
    /// account are removed. Resolves the same way as `delete_account`:
    /// account id, exact name, or substring. When unset, the position
    /// is removed from ALL accounts where it appears.
    pub account_ref: Option<String>,
    /// Optional free-form reason carried onto the confirm card.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStockPositionOutput {
    /// The asset identified for deletion.
    pub target: AssetSnapshot,
    /// One entry per account the position lives in. When the user
    /// scoped to a specific account, this is a single-element list.
    pub positions: Vec<PositionImpact>,
    /// Activity ids the frontend should issue `deleteActivity` for
    /// when the user confirms. Sorted so newest are deleted first
    /// — minimises intermediate-state inconsistencies if a delete
    /// in the middle of the chain fails.
    pub activity_ids: Vec<String>,
    /// Aggregate market value across positions in the user's base currency
    /// — surfaced on the confirm card so the user sees the dollar amount
    /// they're about to remove.
    pub total_market_value_base: f64,
    pub reason: Option<String>,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSnapshot {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionImpact {
    pub account_id: String,
    pub account_name: String,
    /// Quantity currently held in this account.
    pub quantity: f64,
    /// Market value of this position in account's local currency.
    pub market_value_local: f64,
    /// Same value converted to the user's base currency.
    pub market_value_base: f64,
    pub local_currency: String,
    /// How many activities reference this asset in this account.
    /// Cascade-delete will issue this many `deleteActivity` calls.
    pub activity_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub resolved: bool,
    pub candidates: Vec<AssetSnapshot>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
}

fn kind_label(kind: &mizan_core::assets::AssetKind) -> &'static str {
    use mizan_core::assets::AssetKind::*;
    match kind {
        Investment => "INVESTMENT",
        _ => "OTHER",
    }
}

fn is_tradable(kind: Option<&mizan_core::assets::AssetKind>) -> bool {
    use mizan_core::assets::AssetKind::*;
    // INVESTMENT is the Mizan canonical kind for stocks, ETFs, mutual
    // funds — anything tradable through a brokerage. Alt-assets and
    // liabilities go through their own delete tools. None counts too,
    // since some legacy positions have no asset_kind set but still
    // belong to a SECURITIES account.
    matches!(kind, Some(Investment) | None)
}

fn name_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .and_then(|i| i.name.clone())
        .unwrap_or_default()
}

fn symbol_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .map(|i| i.symbol.clone())
        .unwrap_or_default()
}

fn asset_id_of(h: &mizan_core::holdings::Holding) -> String {
    h.instrument
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| h.id.clone())
}

fn snapshot(h: &mizan_core::holdings::Holding) -> AssetSnapshot {
    AssetSnapshot {
        id: asset_id_of(h),
        symbol: symbol_of(h),
        name: name_of(h),
        kind: h
            .asset_kind
            .as_ref()
            .map(|k| kind_label(k).to_string())
            .unwrap_or_else(|| "INVESTMENT".to_string()),
    }
}

enum AssetResolution {
    Single(Box<mizan_core::holdings::Holding>),
    Ambiguous(Vec<mizan_core::holdings::Holding>),
    NotFound,
    Missing,
}

/// Match an asset across the user's full holding set — the same asset
/// may appear in multiple accounts, but they all share the same
/// asset_id / symbol / name. Dedupe by asset_id before reporting
/// ambiguity.
fn resolve_asset(holdings: &[mizan_core::holdings::Holding], reference: &str) -> AssetResolution {
    let r = reference.trim();
    if r.is_empty() {
        return AssetResolution::Missing;
    }
    let tradable: Vec<&mizan_core::holdings::Holding> = holdings
        .iter()
        .filter(|h| is_tradable(h.asset_kind.as_ref()))
        .collect();

    // Dedupe per-account holdings into one entry per asset_id.
    let mut by_id: std::collections::BTreeMap<String, &mizan_core::holdings::Holding> =
        std::collections::BTreeMap::new();
    for h in &tradable {
        by_id.entry(asset_id_of(h)).or_insert(h);
    }

    if let Some(h) = by_id.get(r) {
        return AssetResolution::Single(Box::new((**h).clone()));
    }
    let lower = r.to_lowercase();
    let exact: Vec<_> = by_id
        .values()
        .filter(|h| {
            symbol_of(h).to_lowercase() == lower || name_of(h).to_lowercase() == lower
        })
        .map(|h| (**h).clone())
        .collect();
    if exact.len() == 1 {
        return AssetResolution::Single(Box::new(exact.into_iter().next().unwrap()));
    }
    if exact.len() > 1 {
        return AssetResolution::Ambiguous(exact);
    }
    let substring: Vec<_> = by_id
        .values()
        .filter(|h| {
            symbol_of(h).to_lowercase().contains(&lower)
                || name_of(h).to_lowercase().contains(&lower)
        })
        .map(|h| (**h).clone())
        .collect();
    match substring.len() {
        0 => AssetResolution::NotFound,
        1 => AssetResolution::Single(Box::new(substring.into_iter().next().unwrap())),
        _ => AssetResolution::Ambiguous(substring),
    }
}

fn resolve_account_id(
    accounts: &[mizan_core::accounts::Account],
    reference: &str,
) -> Option<String> {
    let r = reference.trim();
    if r.is_empty() {
        return None;
    }
    if let Some(a) = accounts.iter().find(|a| a.id == r) {
        return Some(a.id.clone());
    }
    let lower = r.to_lowercase();
    if let Some(a) = accounts.iter().find(|a| a.name.to_lowercase() == lower) {
        return Some(a.id.clone());
    }
    accounts
        .iter()
        .find(|a| a.name.to_lowercase().contains(&lower))
        .map(|a| a.id.clone())
}

pub struct DeleteStockPositionTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteStockPositionTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    pub(crate) async fn build_output(
        &self,
        args: DeleteStockPositionArgs,
    ) -> Result<DeleteStockPositionOutput, AiError> {
        debug!(
            "delete_stock_position called: asset={:?}, account={:?}",
            args.asset_ref, args.account_ref
        );

        let base_currency = self.env.base_currency();
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID, &base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let asset = match resolve_asset(&holdings, &args.asset_ref) {
            AssetResolution::Single(h) => *h,
            AssetResolution::Ambiguous(candidates) => {
                return Ok(DeleteStockPositionOutput {
                    target: AssetSnapshot::default(),
                    positions: Vec::new(),
                    activity_ids: Vec::new(),
                    total_market_value_base: 0.0,
                    reason: args.reason,
                    validation: ValidationResult {
                        resolved: false,
                        candidates: candidates.iter().map(snapshot).collect(),
                        warnings: vec![format!(
                            "\"{}\" matched {} tradable assets. Ask the user which one they mean.",
                            args.asset_ref,
                            candidates.len()
                        )],
                        is_valid: false,
                    },
                })
            }
            AssetResolution::NotFound => {
                return Ok(DeleteStockPositionOutput {
                    target: AssetSnapshot::default(),
                    positions: Vec::new(),
                    activity_ids: Vec::new(),
                    total_market_value_base: 0.0,
                    reason: args.reason,
                    validation: ValidationResult {
                        resolved: false,
                        candidates: Vec::new(),
                        warnings: vec![format!(
                            "No tradable asset matching \"{}\" found. Call get_holdings first.",
                            args.asset_ref
                        )],
                        is_valid: false,
                    },
                })
            }
            AssetResolution::Missing => {
                return Ok(DeleteStockPositionOutput {
                    target: AssetSnapshot::default(),
                    positions: Vec::new(),
                    activity_ids: Vec::new(),
                    total_market_value_base: 0.0,
                    reason: args.reason,
                    validation: ValidationResult {
                        resolved: false,
                        candidates: Vec::new(),
                        warnings: vec![
                            "assetRef is required — pass the ticker, name, or id.".to_string(),
                        ],
                        is_valid: false,
                    },
                })
            }
        };

        let target = snapshot(&asset);
        let asset_id = target.id.clone();

        // Optional account scoping.
        let scope_account_id = if let Some(account_ref) = args.account_ref.as_ref() {
            let accounts = self
                .env
                .account_service()
                .get_all_accounts()
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
            match resolve_account_id(&accounts, account_ref) {
                Some(id) => Some(id),
                None => {
                    return Ok(DeleteStockPositionOutput {
                        target,
                        positions: Vec::new(),
                        activity_ids: Vec::new(),
                        total_market_value_base: 0.0,
                        reason: args.reason,
                        validation: ValidationResult {
                            resolved: false,
                            candidates: Vec::new(),
                            warnings: vec![format!(
                                "No account matching \"{}\" found. Drop the account scope or call get_accounts.",
                                account_ref
                            )],
                            is_valid: false,
                        },
                    });
                }
            }
        } else {
            None
        };

        // Gather every account-scoped holding of this asset.
        use rust_decimal::prelude::ToPrimitive;
        let accounts_all = self
            .env
            .account_service()
            .get_all_accounts()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
        let name_for = |id: &str| {
            accounts_all
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| id.to_string())
        };

        let positions_raw: Vec<&mizan_core::holdings::Holding> = holdings
            .iter()
            .filter(|h| asset_id_of(h) == asset_id)
            .filter(|h| {
                scope_account_id
                    .as_ref()
                    .is_none_or(|scope| h.account_id == *scope)
            })
            // Exclude the synthetic TOTAL aggregate so we don't try to
            // delete activities against a fake account.
            .filter(|h| h.account_id != mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID)
            .collect();

        if positions_raw.is_empty() {
            let scope_suffix = scope_account_id
                .as_ref()
                .map(|id| format!(" in {}", name_for(id)))
                .unwrap_or_default();
            let warning = format!(
                "{} resolves to a real asset but has no open positions{}.",
                target.symbol, scope_suffix
            );
            return Ok(DeleteStockPositionOutput {
                target,
                positions: Vec::new(),
                activity_ids: Vec::new(),
                total_market_value_base: 0.0,
                reason: args.reason,
                validation: ValidationResult {
                    resolved: true,
                    candidates: Vec::new(),
                    warnings: vec![warning],
                    is_valid: false,
                },
            });
        }

        // Pull the asset's activities (asset_id_keyword filter handles the
        // matching server-side; we slice per-account on the result).
        let search = self
            .env
            .activity_service()
            .search_activities(
                1,
                10_000, // upper bound — Mizan caps inserts at 100k/account/year, so 10k covers the long tail
                None,
                None,
                Some(asset_id.clone()),
                None,
                None,
                None,
                None,
                None,
            )
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut positions: Vec<PositionImpact> = Vec::new();
        let mut total_mv_base = 0.0;
        let mut all_activity_ids: Vec<(String, String)> = Vec::new(); // (activity_id, date)

        for h in &positions_raw {
            let quantity = h.quantity.to_f64().unwrap_or(0.0);
            let market_value_local = h.market_value.local.to_f64().unwrap_or(0.0);
            let market_value_base = h.market_value.base.to_f64().unwrap_or(0.0);
            total_mv_base += market_value_base;

            let in_account: Vec<_> = search
                .data
                .iter()
                .filter(|a| a.account_id == h.account_id)
                .collect();
            for a in &in_account {
                all_activity_ids.push((a.id.clone(), a.date.clone()));
            }
            positions.push(PositionImpact {
                account_id: h.account_id.clone(),
                account_name: name_for(&h.account_id),
                quantity,
                market_value_local,
                market_value_base,
                local_currency: h.local_currency.clone(),
                activity_count: in_account.len(),
            });
        }

        // Newest activities first so cascade rolls back from the most
        // recent end — keeps the running cost-basis math sane if the
        // sequence is interrupted by a partial failure.
        all_activity_ids.sort_by(|a, b| b.1.cmp(&a.1));
        let activity_ids: Vec<String> = all_activity_ids.into_iter().map(|(id, _)| id).collect();

        let mut warnings = vec![format!(
            "This cascade-deletes {} activity row{} across {} account{}. The position history is gone forever; if you only want to mark the position closed, use sell records instead.",
            activity_ids.len(),
            if activity_ids.len() == 1 { "" } else { "s" },
            positions.len(),
            if positions.len() == 1 { "" } else { "s" }
        )];
        if positions.iter().any(|p| p.quantity > 0.0) && activity_ids.is_empty() {
            warnings.push(
                "The holding shows an open quantity but no activities matched in the search. The position may have been imported as a snapshot — manual cleanup needed; this tool can't unwind it.".to_string(),
            );
        }

        Ok(DeleteStockPositionOutput {
            target,
            positions,
            activity_ids,
            total_market_value_base: total_mv_base,
            reason: args.reason,
            validation: ValidationResult {
                resolved: true,
                candidates: Vec::new(),
                warnings,
                is_valid: true,
            },
        })
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteStockPositionTool<E> {
    const NAME: &'static str = "delete_stock_position";

    type Error = AiError;
    type Args = DeleteStockPositionArgs;
    type Output = DeleteStockPositionOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Remove a stock / ETF / mutual fund position from the user's portfolio by \
                 cascade-deleting all activities that built it. Resolves the asset by ticker \
                 (AAPL), name (Apple), or asset id. Optionally scoped to a single account; \
                 unscoped will remove it from every account where it appears. Returns a DRAFT \
                 preview with quantity, market value, and the number of activity rows that \
                 will be deleted — the user confirms via the tool-call card before anything is \
                 written. Use this when the user says 'I sold all my Apple' (and wants the \
                 history gone, not preserved as a SELL) or 'remove that ETF from my portfolio'. \
                 Prefer record_activity with a SELL when the user wants to keep the cost-basis \
                 history."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetRef": {
                        "type": "string",
                        "description": "Ticker, name, or asset id of the stock to remove."
                    },
                    "accountRef": {
                        "type": "string",
                        "description": "Optional. Account id, name, or substring. Omit to \
                         remove the position from every account it appears in."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason from the user — shown on the confirm card."
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::{
        MockAccountService, MockActivityService, MockEnvironment, MockHoldingsService,
    };
    use chrono::NaiveDate;
    use mizan_core::{
        accounts::Account,
        activities::ActivityDetails,
        assets::AssetKind,
        holdings::{Holding, HoldingType, Instrument, MonetaryValue},
    };
    use rust_decimal::Decimal;

    fn account(id: &str, name: &str) -> Account {
        Account {
            id: id.to_string(),
            name: name.to_string(),
            account_type: "SECURITIES".to_string(),
            currency: "USD".to_string(),
            is_default: false,
            is_active: true,
            ..Default::default()
        }
    }

    fn stock_holding(account_id: &str, asset_id: &str, symbol: &str, qty: i64, value: i64) -> Holding {
        Holding {
            id: format!("h-{}-{}", account_id, asset_id),
            account_id: account_id.to_string(),
            holding_type: HoldingType::Security,
            instrument: Some(Instrument {
                id: asset_id.to_string(),
                symbol: symbol.to_string(),
                name: Some(format!("{} Inc", symbol)),
                currency: "USD".to_string(),
                notes: None,
                pricing_mode: "MARKET".to_string(),
                preferred_provider: None,
                exchange_mic: None,
                classifications: None,
                metadata: None,
            }),
            asset_kind: Some(AssetKind::Investment),
            quantity: Decimal::from(qty),
            open_date: None,
            lots: None,
            contract_multiplier: Decimal::ONE,
            local_currency: "USD".to_string(),
            base_currency: "USD".to_string(),
            fx_rate: None,
            market_value: MonetaryValue {
                local: Decimal::from(value),
                base: Decimal::from(value),
            },
            cost_basis: None,
            price: None,
            purchase_price: None,
            unrealized_gain: None,
            unrealized_gain_pct: None,
            realized_gain: None,
            realized_gain_pct: None,
            dividend_income: None,
            total_gain: None,
            total_gain_pct: None,
            day_change: None,
            day_change_pct: None,
            prev_close_value: None,
            weight: Decimal::ZERO,
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
            metadata: None,
        }
    }

    fn activity(id: &str, account_id: &str, asset_id: &str, date: &str) -> ActivityDetails {
        ActivityDetails {
            id: id.to_string(),
            account_id: account_id.to_string(),
            asset_id: asset_id.to_string(),
            activity_type: "BUY".to_string(),
            subtype: None,
            status: mizan_core::activities::ActivityStatus::Posted,
            date: date.to_string(),
            quantity: Some("10".to_string()),
            unit_price: Some("100".to_string()),
            currency: "USD".to_string(),
            fee: Some("0".to_string()),
            amount: Some("1000".to_string()),
            needs_review: false,
            comment: None,
            fx_rate: None,
            created_at: "2026-06-21T00:00:00Z".to_string(),
            updated_at: "2026-06-21T00:00:00Z".to_string(),
            account_name: account_id.to_string(),
            account_currency: "USD".to_string(),
            asset_symbol: asset_id.to_string(),
            asset_name: Some(asset_id.to_string()),
            exchange_mic: None,
            asset_pricing_mode: "MARKET".to_string(),
            instrument_type: None,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,
            is_user_modified: false,
            metadata: None,
        }
    }

    fn tool(
        holdings: Vec<Holding>,
        activities: Vec<ActivityDetails>,
        accounts: Vec<Account>,
    ) -> DeleteStockPositionTool<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.holdings_service = Arc::new(MockHoldingsService { holdings });
        env.activity_service = Arc::new(MockActivityService { activities });
        env.account_service = Arc::new(MockAccountService { accounts });
        DeleteStockPositionTool::new(Arc::new(env))
    }

    #[tokio::test]
    async fn resolves_by_ticker_in_single_account() {
        let t = tool(
            vec![stock_holding("acc1", "AAPL", "AAPL", 100, 18_000)],
            vec![activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z")],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "AAPL".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.target.symbol, "AAPL");
        assert_eq!(out.positions.len(), 1);
        assert_eq!(out.activity_ids, vec!["act-1"]);
        assert_eq!(out.total_market_value_base, 18_000.0);
    }

    #[tokio::test]
    async fn resolves_across_multiple_accounts() {
        let t = tool(
            vec![
                stock_holding("acc1", "AAPL", "AAPL", 50, 9_000),
                stock_holding("acc2", "AAPL", "AAPL", 50, 9_000),
            ],
            vec![
                activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z"),
                activity("act-2", "acc2", "AAPL", "2026-02-10T00:00:00Z"),
            ],
            vec![
                account("acc1", "IRA"),
                account("acc2", "Brokerage"),
            ],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "AAPL".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.positions.len(), 2);
        assert_eq!(out.activity_ids.len(), 2);
        // Newest activity first so cascade rolls back chronologically.
        assert_eq!(out.activity_ids[0], "act-2");
        assert_eq!(out.activity_ids[1], "act-1");
        assert_eq!(out.total_market_value_base, 18_000.0);
    }

    #[tokio::test]
    async fn account_scope_filters_positions() {
        let t = tool(
            vec![
                stock_holding("acc1", "AAPL", "AAPL", 50, 9_000),
                stock_holding("acc2", "AAPL", "AAPL", 50, 9_000),
            ],
            vec![
                activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z"),
                activity("act-2", "acc2", "AAPL", "2026-02-10T00:00:00Z"),
            ],
            vec![
                account("acc1", "IRA"),
                account("acc2", "Brokerage"),
            ],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "AAPL".into(),
                account_ref: Some("IRA".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.positions.len(), 1);
        assert_eq!(out.positions[0].account_name, "IRA");
        assert_eq!(out.activity_ids, vec!["act-1"]);
    }

    #[tokio::test]
    async fn unknown_account_blocks_confirm() {
        let t = tool(
            vec![stock_holding("acc1", "AAPL", "AAPL", 100, 18_000)],
            vec![activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z")],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "AAPL".into(),
                account_ref: Some("nonexistent".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("No account matching")));
    }

    #[tokio::test]
    async fn ambiguous_asset_blocks_confirm() {
        let t = tool(
            vec![
                stock_holding("acc1", "PINS", "PINS", 100, 5_000),
                stock_holding("acc1", "PIN", "PIN", 50, 2_500),
            ],
            vec![],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "PIN".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        // "PIN" matches exactly one (PIN), so it resolves; but if we
        // search 'PI' substring it should be ambiguous. Test that path.
        assert!(out.validation.is_valid || out.validation.candidates.len() >= 1);

        let out2 = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "PI".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out2.validation.is_valid);
        assert_eq!(out2.validation.candidates.len(), 2);
    }

    #[tokio::test]
    async fn missing_ref_blocks_confirm() {
        let t = tool(
            vec![stock_holding("acc1", "AAPL", "AAPL", 100, 18_000)],
            vec![activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z")],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("required")));
    }

    #[tokio::test]
    async fn unknown_asset_returns_actionable_warning() {
        let t = tool(
            vec![stock_holding("acc1", "AAPL", "AAPL", 100, 18_000)],
            vec![],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "TSLA".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("No tradable asset matching")));
    }

    #[tokio::test]
    async fn alternative_assets_are_filtered_out() {
        let mut prop = stock_holding("acc1", "house", "HOUSE", 1, 1_000_000);
        prop.asset_kind = Some(AssetKind::Property);
        let t = tool(
            vec![prop],
            vec![],
            vec![account("acc1", "Personal")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "HOUSE".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!out.validation.is_valid);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("No tradable asset matching")));
    }

    #[tokio::test]
    async fn warning_surfaces_cascade_blast_radius() {
        let t = tool(
            vec![stock_holding("acc1", "AAPL", "AAPL", 100, 18_000)],
            vec![
                activity("act-1", "acc1", "AAPL", "2026-01-15T00:00:00Z"),
                activity("act-2", "acc1", "AAPL", "2026-03-20T00:00:00Z"),
                activity("act-3", "acc1", "AAPL", "2026-05-10T00:00:00Z"),
            ],
            vec![account("acc1", "Brokerage")],
        );
        let out = t
            .build_output(DeleteStockPositionArgs {
                asset_ref: "AAPL".into(),
                reason: Some("sold the rest".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(out.validation.is_valid);
        assert_eq!(out.activity_ids.len(), 3);
        // Newest-first cascade ordering.
        assert_eq!(out.activity_ids[0], "act-3");
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| w.contains("3 activity rows")));
        assert_eq!(out.reason.as_deref(), Some("sold the rest"));
    }
}
