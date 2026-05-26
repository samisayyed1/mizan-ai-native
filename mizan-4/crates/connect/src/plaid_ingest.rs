//! Plaid investment-transaction → local-activity mapping.
//!
//! Pure mapping layer for the audit's "valued but untraceable" gap.
//! The cloud (mizan-connect) syncs Plaid `/investments/transactions/get`
//! into Postgres on every broker sync (see migrations 0009/0010);
//! this module turns each row into a Mizan `NewActivity` that the
//! desktop's existing `ActivitiesService::prepare_activities_for_sync`
//! + `upsert_activities_bulk` pipeline already knows how to ingest.
//!
//! Why a mapping layer and not a full pipeline:
//!   - `prepare_activities_for_sync` is account-scoped (takes an
//!     `&Account`) and asset resolution lives behind a trait. Wiring
//!     this end-to-end is best done at the Tauri command layer where
//!     the services + accounts are already in hand.
//!   - This module is a pure function: `Vec<json> →
//!     Vec<(local_account_id, NewActivity)>` so it's trivially
//!     testable and the orchestration layer stays thin.
//!
//! Plaid `type` → Mizan `activity_type` mapping:
//!
//!   - buy           → BUY      (qty, price; debits cash)
//!   - sell          → SELL     (qty, price; credits cash)
//!   - dividend      → DIVIDEND (cash credit; asset_id when symbol present)
//!   - fee           → FEE      (cash debit, no asset binding)
//!   - cash          → DEPOSIT  if amount<0 else WITHDRAWAL
//!     (Plaid sign convention is broker-POV: positive = funds leaving)
//!   - transfer      → TRANSFER_IN  if amount<0 else TRANSFER_OUT
//!   - cancelled rows → status = Void on the same activity_type
//!   - anything else → UNKNOWN + needs_review = true
//!
//! Source identity for idempotent upsert (set by caller from the
//! mapping output):
//!   - `source_system`    = "PLAID"
//!   - `source_record_id` = investment_transaction_id (Plaid stable id)
//!   - `source_group_id`  = item_id (Plaid Item / broker connection)

use std::collections::HashMap;
use std::sync::Arc;

use log::{debug, info, warn};
use mizan_core::accounts::{Account, AccountServiceTrait};
use mizan_core::activities::{
    ActivityServiceTrait, ActivityStatus, ActivityUpsert, AssetResolutionInput, NewActivity,
    ACTIVITY_TYPE_BUY, ACTIVITY_TYPE_DEPOSIT, ACTIVITY_TYPE_DIVIDEND, ACTIVITY_TYPE_FEE,
    ACTIVITY_TYPE_SELL, ACTIVITY_TYPE_TRANSFER_IN, ACTIVITY_TYPE_TRANSFER_OUT,
    ACTIVITY_TYPE_UNKNOWN, ACTIVITY_TYPE_WITHDRAWAL,
};
use mizan_core::errors::Result;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::client::ConnectApiClient;

/// One mapped Plaid investment transaction, keyed to a resolved local
/// Mizan `account_id`. The caller groups by account_id, calls
/// `prepare_activities_for_sync` per account, then upserts.
#[derive(Debug, Clone)]
pub struct MappedPlaidActivity {
    pub local_account_id: String,
    pub activity: NewActivity,
}

/// End-to-end ingest summary returned by `ingest_plaid_investment_transactions`.
/// All counts are post-upsert truth — the activity made it into local
/// SQLite (and therefore participates in TWR + cost basis recalc).
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidIngestSummary {
    pub mapping: PlaidMappingSummary,
    /// Activities the upsert wrote anew.
    pub created: usize,
    /// Activities matched on idempotency / source identity and updated.
    pub updated: usize,
    /// Activities skipped (e.g. user-modified rows the bulk-upsert
    /// guard refuses to overwrite — preserves manual edits).
    pub skipped: usize,
    /// Accounts touched (used by the caller for portfolio-recalc
    /// invalidation).
    pub accounts_touched: Vec<String>,
    /// Asset rows ensure_assets had to create on the way in.
    pub assets_created: u32,
    /// Per-account error messages, keyed by local account_id. Non-fatal
    /// — other accounts still complete.
    pub per_account_errors: std::collections::HashMap<String, String>,
}

/// Outcome of mapping a batch of Plaid investment transactions.
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidMappingSummary {
    pub fetched: usize,
    pub mapped: usize,
    /// Activities whose Plaid `account_id` didn't resolve to a local
    /// Mizan account — logged at DEBUG, counted here for the caller.
    pub skipped_unknown_account: usize,
    /// Activities flagged needs_review (unmapped type, no symbol for a
    /// security-bearing row, zero-amount cash/transfer).
    pub needs_review: usize,
}

/// Map a batch of Plaid investment transactions (as returned by
/// `ConnectApiClient::list_plaid_investment_transactions`) into
/// Mizan `NewActivity` rows ready for the existing sync pipeline.
///
/// `accounts` is a snapshot of the user's local accounts; we use it
/// to translate Plaid's `accountId` into our internal `account_id`
/// via the `provider_account_id` column (set when a Plaid sync first
/// creates the account row).
pub fn map_plaid_investment_transactions(
    txns: &[serde_json::Value],
    accounts: &[Account],
) -> (Vec<MappedPlaidActivity>, PlaidMappingSummary) {
    let mut provider_to_local: HashMap<String, String> = HashMap::new();
    for acct in accounts {
        if let Some(pid) = acct.provider_account_id.as_ref() {
            provider_to_local.insert(pid.clone(), acct.id.clone());
        }
    }

    let mut summary = PlaidMappingSummary {
        fetched: txns.len(),
        ..Default::default()
    };
    let mut out: Vec<MappedPlaidActivity> = Vec::with_capacity(txns.len());

    for raw in txns {
        let provider_account_id = match raw
            .get("accountId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        {
            Some(s) => s,
            None => {
                warn!("Plaid map: skipping row with no accountId");
                continue;
            }
        };
        let local_account_id = match provider_to_local.get(&provider_account_id) {
            Some(id) => id.clone(),
            None => {
                debug!(
                    "Plaid map: no local account for provider id {provider_account_id}; skipping"
                );
                summary.skipped_unknown_account += 1;
                continue;
            }
        };

        if let Some(activity) = map_single(raw, &local_account_id, &mut summary) {
            out.push(MappedPlaidActivity {
                local_account_id,
                activity,
            });
            summary.mapped += 1;
        }
    }

    (out, summary)
}

/// End-to-end ingest: fetch from cloud → map → prepare → upsert.
///
/// This is the public entry point Tauri commands and schedulers
/// invoke. It owns the full pipeline so callers don't have to know
/// about per-account batching, asset resolution, or the upsert
/// idempotency contract.
///
/// Failure mode is per-account: one account that fails preparation
/// (e.g. an unrecoverable asset-resolution error) won't stop the
/// other accounts from completing. The `per_account_errors` map
/// captures what failed; the caller can surface that in a toast.
///
/// The bulk upsert emits a single aggregated `ActivitiesChanged`
/// domain event covering all touched accounts, which drives the
/// downstream portfolio recalc + FX-pair registration. No additional
/// fan-out is needed at the call site.
pub async fn ingest_plaid_investment_transactions(
    api_client: &ConnectApiClient,
    activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
    account_service: Arc<dyn AccountServiceTrait + Send + Sync>,
    since: Option<&str>,
    limit: Option<u32>,
) -> Result<PlaidIngestSummary> {
    let txns = api_client
        .list_plaid_investment_transactions(since, None, limit)
        .await?;
    info!(
        "Plaid ingest: fetched {} investment transactions from cloud",
        txns.len()
    );

    if txns.is_empty() {
        return Ok(PlaidIngestSummary::default());
    }

    let accounts = account_service.get_all_accounts()?;
    let (mapped, mapping_summary) = map_plaid_investment_transactions(&txns, &accounts);

    let mut summary = PlaidIngestSummary {
        mapping: mapping_summary,
        ..Default::default()
    };

    if mapped.is_empty() {
        info!("Plaid ingest: no rows mapped to local accounts");
        return Ok(summary);
    }

    // Group by local_account_id so we can satisfy
    // prepare_activities_for_sync's per-account contract.
    let mut by_account: HashMap<String, Vec<NewActivity>> = HashMap::new();
    for m in mapped {
        by_account
            .entry(m.local_account_id)
            .or_default()
            .push(m.activity);
    }

    let account_index: HashMap<&str, &Account> =
        accounts.iter().map(|a| (a.id.as_str(), a)).collect();

    // Aggregate ActivityUpsert payloads across all accounts so we can
    // single-shot the bulk upsert at the end. The bulk path emits one
    // aggregated ActivitiesChanged event regardless of how many
    // accounts were touched, which is what we want.
    let mut all_upserts: Vec<ActivityUpsert> = Vec::new();

    for (account_id, account_activities) in by_account {
        let Some(account) = account_index.get(account_id.as_str()) else {
            // Account vanished between the map step and now; record + skip.
            summary
                .per_account_errors
                .insert(account_id.clone(), "Account not found".to_string());
            continue;
        };

        let prepared = match activity_service
            .prepare_activities_for_sync(account_activities, account)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Plaid ingest: prepare_activities_for_sync failed for account {}: {}",
                    account_id, e
                );
                summary.per_account_errors.insert(account_id.clone(), e.to_string());
                continue;
            }
        };

        summary.assets_created += prepared.assets_created;
        summary.accounts_touched.push(account_id.clone());

        for p in prepared.prepared {
            let act = p.activity;
            let asset_id = p.resolved_asset_id;
            let activity_id = act
                .id
                .clone()
                .unwrap_or_else(|| format!("plaid-{}-{}", account_id, all_upserts.len()));

            all_upserts.push(ActivityUpsert {
                id: activity_id,
                account_id: act.account_id,
                asset_id,
                activity_type: act.activity_type,
                subtype: act.subtype,
                activity_date: act.activity_date,
                quantity: act.quantity,
                unit_price: act.unit_price,
                currency: act.currency,
                fee: act.fee,
                amount: act.amount,
                status: act.status,
                notes: act.notes,
                fx_rate: act.fx_rate,
                metadata: act.metadata,
                needs_review: act.needs_review,
                source_system: act.source_system,
                source_record_id: act.source_record_id,
                source_group_id: act.source_group_id,
                idempotency_key: act.idempotency_key,
                import_run_id: None,
            });
        }
    }

    if all_upserts.is_empty() {
        info!("Plaid ingest: nothing to upsert after preparation");
        return Ok(summary);
    }

    info!(
        "Plaid ingest: upserting {} activities across {} accounts",
        all_upserts.len(),
        summary.accounts_touched.len()
    );

    let bulk = activity_service
        .upsert_activities_bulk(all_upserts)
        .await?;
    summary.created = bulk.created;
    summary.updated = bulk.updated;
    summary.skipped = bulk.skipped;

    info!(
        "Plaid ingest complete: created={} updated={} skipped={} assets_created={}",
        summary.created, summary.updated, summary.skipped, summary.assets_created
    );

    Ok(summary)
}

fn map_single(
    raw: &serde_json::Value,
    local_account_id: &str,
    summary: &mut PlaidMappingSummary,
) -> Option<NewActivity> {
    let txn_id = raw.get("investmentTransactionId")?.as_str()?.to_string();
    let item_id = raw
        .get("itemId")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let plaid_type = raw
        .get("type")
        .and_then(|v| v.as_str())
        .map(str::to_lowercase)
        .unwrap_or_else(|| "unknown".to_string());

    let amount_signed = decimal_from_str_field(raw, "amount");
    let quantity = decimal_from_str_field(raw, "quantity").map(|q| q.abs());
    let price = decimal_from_str_field(raw, "price");
    let fees = decimal_from_str_field(raw, "fees");
    let currency = raw
        .get("isoCurrencyCode")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("unofficialCurrencyCode").and_then(|v| v.as_str()))
        .unwrap_or("USD")
        .to_string();

    // Treat bare YYYY-MM-DD as noon-UTC to dodge timezone DST drift —
    // same convention QA Pass 18 baked into the activity write path
    // for Mizan-native rows.
    let date_str = raw.get("transactionDate").and_then(|v| v.as_str())?;
    let activity_date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(12, 0, 0)?
        .and_utc()
        .to_rfc3339();

    let cancelled = raw
        .get("cancelled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let symbol = raw
        .get("securityTickerSymbol")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let security_name = raw
        .get("securityName")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let plaid_security_type = raw
        .get("securityType")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut needs_review = false;
    let mut needs_review_reason: Option<&str> = None;
    let mut asset_input: Option<AssetResolutionInput> = None;

    // Plaid `amount` sign convention is from the broker's POV:
    //   - Buys + outgoing flows: positive (cash leaving the account)
    //   - Sells + incoming flows: negative (cash entering the account)
    // Mizan's BUY/SELL store unsigned `amount` (cost/proceeds); we take
    // .abs() at the boundary. Cash-direction types (DEPOSIT/WITHDRAWAL,
    // TRANSFER_IN/TRANSFER_OUT) encode direction via the type itself.
    let activity_type = match plaid_type.as_str() {
        "buy" => {
            if symbol.is_none() {
                needs_review = true;
                needs_review_reason = Some("BUY with no security symbol");
            } else {
                asset_input = Some(build_asset_input(
                    symbol,
                    security_name.clone(),
                    plaid_security_type.clone(),
                ));
            }
            ACTIVITY_TYPE_BUY
        }
        "sell" => {
            if symbol.is_none() {
                needs_review = true;
                needs_review_reason = Some("SELL with no security symbol");
            } else {
                asset_input = Some(build_asset_input(
                    symbol,
                    security_name.clone(),
                    plaid_security_type.clone(),
                ));
            }
            ACTIVITY_TYPE_SELL
        }
        "dividend" => {
            // DIVIDEND can carry an asset (asset-backed) or be cash-only.
            if symbol.is_some() {
                asset_input = Some(build_asset_input(
                    symbol,
                    security_name.clone(),
                    plaid_security_type.clone(),
                ));
            }
            ACTIVITY_TYPE_DIVIDEND
        }
        "fee" => ACTIVITY_TYPE_FEE,
        "cash" => match amount_signed {
            Some(a) if a > Decimal::ZERO => ACTIVITY_TYPE_WITHDRAWAL,
            Some(a) if a < Decimal::ZERO => ACTIVITY_TYPE_DEPOSIT,
            _ => {
                needs_review = true;
                needs_review_reason = Some("Cash row with zero amount");
                ACTIVITY_TYPE_UNKNOWN
            }
        },
        "transfer" => match amount_signed {
            Some(a) if a > Decimal::ZERO => ACTIVITY_TYPE_TRANSFER_OUT,
            Some(a) if a < Decimal::ZERO => ACTIVITY_TYPE_TRANSFER_IN,
            _ => {
                needs_review = true;
                needs_review_reason = Some("Transfer with zero amount");
                ACTIVITY_TYPE_UNKNOWN
            }
        },
        other => {
            needs_review = true;
            needs_review_reason = Some("Unmapped Plaid investment-transaction type");
            debug!("Plaid map: unmapped type='{other}' txn={txn_id}");
            ACTIVITY_TYPE_UNKNOWN
        }
    };

    if needs_review {
        summary.needs_review += 1;
    }

    let amount = amount_signed.map(|a| a.abs());
    let status = if cancelled {
        Some(ActivityStatus::Void)
    } else {
        Some(ActivityStatus::Posted)
    };

    let notes = match (needs_review_reason, raw.get("name").and_then(|v| v.as_str())) {
        (Some(reason), Some(name)) => Some(format!("{reason}: {name}")),
        (Some(reason), None) => Some(reason.to_string()),
        (None, Some(name)) => Some(name.to_string()),
        (None, None) => None,
    };

    Some(NewActivity {
        id: Some(format!("plaid-{txn_id}")),
        account_id: local_account_id.to_string(),
        asset: asset_input,
        activity_type: activity_type.to_string(),
        subtype: None,
        activity_date,
        quantity,
        unit_price: price,
        currency,
        fee: fees,
        amount,
        status,
        notes,
        fx_rate: None,
        metadata: None,
        needs_review: Some(needs_review),
        source_system: Some("PLAID".to_string()),
        source_record_id: Some(txn_id),
        source_group_id: item_id,
        idempotency_key: None,
    })
}

fn decimal_from_str_field(raw: &serde_json::Value, key: &str) -> Option<Decimal> {
    raw.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Decimal::from_str(s).ok())
}

fn build_asset_input(
    symbol: Option<&str>,
    name: Option<String>,
    plaid_security_type: Option<String>,
) -> AssetResolutionInput {
    AssetResolutionInput {
        id: None,
        symbol: symbol.map(str::to_string),
        exchange_mic: None,
        kind: None, // let AssetService infer; Plaid's type ("equity", "etf") gets passed via instrument_type
        name,
        quote_mode: None,
        quote_ccy: None,
        instrument_type: plaid_security_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mizan_core::accounts::{Account, TrackingMode};
    use serde_json::json;

    fn fake_account(id: &str, provider_account_id: Option<&str>) -> Account {
        Account {
            id: id.to_string(),
            name: format!("Test {id}"),
            account_type: "INVESTMENT".to_string(),
            group: None,
            currency: "USD".to_string(),
            is_default: false,
            is_active: true,
            platform_id: None,
            account_number: None,
            meta: None,
            provider: provider_account_id.map(|_| "PLAID".to_string()),
            provider_account_id: provider_account_id.map(str::to_string),
            is_archived: false,
            tracking_mode: TrackingMode::Holdings,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
        }
    }

    fn fake_txn(
        plaid_type: &str,
        plaid_account_id: &str,
        symbol: Option<&str>,
        amount: &str,
        quantity: Option<&str>,
        price: Option<&str>,
    ) -> serde_json::Value {
        json!({
            "investmentTransactionId": format!("tx-{plaid_type}-{plaid_account_id}"),
            "itemId": "item-1",
            "accountId": plaid_account_id,
            "type": plaid_type,
            "subtype": serde_json::Value::Null,
            "name": format!("test {plaid_type}"),
            "securityId": symbol.map(|s| format!("sec-{s}")),
            "securityTickerSymbol": symbol,
            "securityName": symbol.map(|s| format!("{s} Inc")),
            "securityType": "equity",
            "amount": amount,
            "quantity": quantity,
            "price": price,
            "fees": serde_json::Value::Null,
            "isoCurrencyCode": "USD",
            "unofficialCurrencyCode": serde_json::Value::Null,
            "transactionDate": "2026-05-15",
            "cancelled": false,
            "rawJson": {},
            "updatedAt": "2026-05-15T00:00:00Z"
        })
    }

    #[test]
    fn maps_buy_with_symbol_to_buy_with_asset_input() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let txn = fake_txn("buy", "plaid-acct-1", Some("AAPL"), "1500.00", Some("10"), Some("150"));
        let (mapped, summary) = map_plaid_investment_transactions(&[txn], &accounts);
        assert_eq!(mapped.len(), 1);
        assert_eq!(summary.mapped, 1);
        assert_eq!(summary.needs_review, 0);
        let m = &mapped[0];
        assert_eq!(m.local_account_id, "local-1");
        assert_eq!(m.activity.activity_type, "BUY");
        assert_eq!(m.activity.asset.as_ref().and_then(|a| a.symbol.clone()), Some("AAPL".to_string()));
        assert_eq!(m.activity.quantity, Some(Decimal::from(10)));
        assert_eq!(m.activity.unit_price, Some(Decimal::from(150)));
        assert_eq!(m.activity.amount, Some(Decimal::from(1500))); // unsigned at the boundary
        assert_eq!(m.activity.source_system.as_deref(), Some("PLAID"));
    }

    #[test]
    fn maps_sell_with_negative_plaid_amount_to_unsigned_sell_amount() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        // Plaid emits sells as negative from broker POV (funds entering)
        let txn = fake_txn("sell", "plaid-acct-1", Some("AAPL"), "-2000.00", Some("10"), Some("200"));
        let (mapped, _) = map_plaid_investment_transactions(&[txn], &accounts);
        assert_eq!(mapped[0].activity.activity_type, "SELL");
        assert_eq!(mapped[0].activity.amount, Some(Decimal::from(2000)));
    }

    #[test]
    fn cash_positive_amount_maps_to_withdrawal_negative_to_deposit() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let withdraw = fake_txn("cash", "plaid-acct-1", None, "500.00", None, None);
        let deposit = fake_txn("cash", "plaid-acct-1", None, "-500.00", None, None);
        let (mapped, _) = map_plaid_investment_transactions(&[withdraw, deposit], &accounts);
        assert_eq!(mapped[0].activity.activity_type, "WITHDRAWAL");
        assert_eq!(mapped[1].activity.activity_type, "DEPOSIT");
    }

    #[test]
    fn dividend_with_symbol_binds_asset_dividend_without_symbol_does_not() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let with = fake_txn("dividend", "plaid-acct-1", Some("AAPL"), "-25", None, None);
        let without = fake_txn("dividend", "plaid-acct-1", None, "-10", None, None);
        let (mapped, _) = map_plaid_investment_transactions(&[with, without], &accounts);
        assert_eq!(mapped[0].activity.activity_type, "DIVIDEND");
        assert!(mapped[0].activity.asset.is_some());
        assert_eq!(mapped[1].activity.activity_type, "DIVIDEND");
        assert!(mapped[1].activity.asset.is_none());
        // Neither flagged needs_review — symbol-less dividend is legitimate cash-only.
    }

    #[test]
    fn buy_without_symbol_is_flagged_needs_review() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let txn = fake_txn("buy", "plaid-acct-1", None, "1500.00", Some("10"), Some("150"));
        let (mapped, summary) = map_plaid_investment_transactions(&[txn], &accounts);
        assert_eq!(summary.needs_review, 1);
        assert_eq!(mapped[0].activity.needs_review, Some(true));
        assert!(mapped[0].activity.notes.as_deref().unwrap().contains("no security symbol"));
    }

    #[test]
    fn unknown_plaid_account_is_skipped_with_summary_increment() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-other"))];
        let txn = fake_txn("buy", "plaid-acct-1", Some("AAPL"), "1500.00", Some("10"), Some("150"));
        let (mapped, summary) = map_plaid_investment_transactions(&[txn], &accounts);
        assert!(mapped.is_empty());
        assert_eq!(summary.skipped_unknown_account, 1);
    }

    #[test]
    fn cancelled_flag_propagates_to_void_status() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let mut txn = fake_txn("buy", "plaid-acct-1", Some("AAPL"), "1500.00", Some("10"), Some("150"));
        txn["cancelled"] = json!(true);
        let (mapped, _) = map_plaid_investment_transactions(&[txn], &accounts);
        assert!(matches!(mapped[0].activity.status, Some(ActivityStatus::Void)));
    }

    #[test]
    fn unmapped_type_lands_as_unknown_with_needs_review() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let mut txn = fake_txn("buy", "plaid-acct-1", None, "100", None, None);
        txn["type"] = json!("some_future_type");
        let (mapped, summary) = map_plaid_investment_transactions(&[txn], &accounts);
        assert_eq!(mapped[0].activity.activity_type, "UNKNOWN");
        assert_eq!(summary.needs_review, 1);
    }

    #[test]
    fn noon_utc_anchoring_on_bare_date() {
        let accounts = vec![fake_account("local-1", Some("plaid-acct-1"))];
        let txn = fake_txn("buy", "plaid-acct-1", Some("AAPL"), "100", Some("1"), Some("100"));
        let (mapped, _) = map_plaid_investment_transactions(&[txn], &accounts);
        // 2026-05-15T12:00:00+00:00 — noon UTC dodges DST drift for Western timezones
        assert!(mapped[0].activity.activity_date.starts_with("2026-05-15T12:00:00"));
    }
}
