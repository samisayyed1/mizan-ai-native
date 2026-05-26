use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PlaidContext {
    pub client: super::client::PlaidClient,
    pub token_cipher: crate::secret_crypto::SecretCipher,
    pub webhook_keys: super::webhook_verifier::WebhookKeyCache,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTokenResponse {
    pub link_token: String,
    pub expiration: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTokenRequest {
    #[serde(default)]
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangePublicTokenRequest {
    pub public_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangePublicTokenResponse {
    pub item_id: String,
    pub accounts_synced: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidConnectionDto {
    pub item_id: String,
    pub institution_id: Option<String>,
    pub institution_name: Option<String>,
    pub status: String,
    pub account_count: i64,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub updated_at: OffsetDateTime,
}

/// Investment transaction row served to the desktop.
///
/// Mirrors the storage schema 1:1 but with camelCase keys for the JSON
/// wire format. Monetary fields are serialised as strings rather than
/// JSON numbers so the desktop can parse them into Decimal without f64
/// round-trip drift at the boundary — same convention the desktop's
/// Decimal-as-string adapter uses for activities.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidInvestmentTransactionDto {
    pub investment_transaction_id: String,
    pub item_id: String,
    pub account_id: String,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
    pub subtype: Option<String>,
    pub name: Option<String>,
    pub security_id: Option<String>,
    /// Decimal-as-string. Empty string would be ambiguous; we emit
    /// `null` for absent values via Option.
    pub amount: Option<String>,
    pub price: Option<String>,
    pub quantity: Option<String>,
    pub fees: Option<String>,
    pub iso_currency_code: Option<String>,
    pub unofficial_currency_code: Option<String>,
    pub transaction_date: String,
    pub cancelled: bool,
    pub raw_json: serde_json::Value,
    pub updated_at: OffsetDateTime,
}

/// Query-string parameters for GET /sync/plaid/investment-transactions.
/// All optional — defaults pull the most recent 500 across all accounts.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListInvestmentTransactionsParams {
    /// ISO YYYY-MM-DD. Only rows with `transaction_date >= since` returned.
    pub since: Option<String>,
    /// Filter to a single Plaid account_id.
    pub account_id: Option<String>,
    /// Max rows. Server-side cap at 1000 to bound memory.
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidAccountDto {
    pub account_id: String,
    pub item_id: String,
    pub name: Option<String>,
    pub official_name: Option<String>,
    pub institution_name: Option<String>,
    pub account_type: Option<String>,
    pub subtype: Option<String>,
    pub mask: Option<String>,
    pub balances: serde_json::Value,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidSyncResponse {
    pub item_id: String,
    pub accounts_synced: usize,
    pub transactions_added: usize,
    pub transactions_modified: usize,
    pub transactions_removed: usize,
    pub liabilities_synced: usize,
    pub holdings_synced: usize,
    /// Count of investment_transactions inserted/upserted in this sync run.
    /// Field name + #[serde(default)] keeps the wire format backward-
    /// compatible with desktops that haven't been updated yet.
    #[serde(default)]
    pub investment_transactions_synced: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidSyncRequest {
    #[serde(default)]
    pub item_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidHealthResponse {
    pub configured: bool,
    pub environment: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidWebhookPayload {
    pub webhook_type: Option<String>,
    pub webhook_code: Option<String>,
    pub item_id: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, FromRow)]
pub struct StoredPlaidItem {
    pub item_id: String,
    pub access_token_encrypted: Vec<u8>,
}

#[derive(Debug)]
pub struct PlaidTokenExchange {
    pub access_token: SecretString,
    pub item_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlaidErrorBody {
    pub error_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub display_message: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidLinkTokenCreateResponse {
    pub link_token: String,
    pub expiration: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidPublicTokenExchangeResponse {
    pub access_token: String,
    pub item_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlaidAccount {
    pub account_id: String,
    pub name: Option<String>,
    pub official_name: Option<String>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    pub subtype: Option<String>,
    pub mask: Option<String>,
    pub balances: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlaidItem {
    pub item_id: String,
    pub institution_id: Option<String>,
    pub institution_name: Option<String>,
    pub webhook: Option<String>,
    pub consent_expiration_time: Option<String>,
    pub update_type: Option<String>,
    pub error: Option<serde_json::Value>,
    pub available_products: Option<Vec<String>>,
    pub billed_products: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidAccountsGetResponse {
    pub accounts: Vec<PlaidAccount>,
    pub item: PlaidItem,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlaidTransaction {
    pub transaction_id: String,
    pub account_id: String,
    pub amount: f64,
    pub date: String,
    pub name: Option<String>,
    pub merchant_name: Option<String>,
    pub currency_code: Option<String>,
    pub iso_currency_code: Option<String>,
    pub category: Option<Vec<String>>,
    pub pending: Option<bool>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemovedTransaction {
    pub transaction_id: String,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionsSyncResponse {
    pub added: Vec<PlaidTransaction>,
    pub modified: Vec<PlaidTransaction>,
    pub removed: Vec<RemovedTransaction>,
    pub next_cursor: String,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct PlaidLiabilitiesResponse {
    pub accounts: Vec<PlaidAccount>,
    pub liabilities: serde_json::Value,
    pub item: PlaidItem,
}

#[derive(Debug, Deserialize)]
pub struct PlaidInvestmentsHoldingsResponse {
    pub accounts: Vec<PlaidAccount>,
    pub holdings: Vec<serde_json::Value>,
    pub securities: Vec<serde_json::Value>,
    pub item: PlaidItem,
}

/// A single investment transaction as returned by /investments/transactions/get.
/// We keep the raw JSON via `#[serde(flatten)]` so the repository can persist
/// the full payload — Plaid sometimes adds fields between schema versions and
/// we don't want to lose those on the cloud side.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlaidInvestmentTransaction {
    pub investment_transaction_id: String,
    pub account_id: String,
    pub security_id: Option<String>,
    pub date: String,
    pub name: Option<String>,
    pub quantity: Option<f64>,
    pub amount: Option<f64>,
    pub price: Option<f64>,
    pub fees: Option<f64>,
    pub iso_currency_code: Option<String>,
    pub unofficial_currency_code: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
    pub subtype: Option<String>,
    /// Plaid uses `cancel_transaction_id` to point at the canceled
    /// transaction; presence-of-field means this row is the cancellation.
    #[serde(default)]
    pub cancel_transaction_id: Option<String>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidInvestmentsTransactionsResponse {
    pub accounts: Vec<PlaidAccount>,
    pub investment_transactions: Vec<PlaidInvestmentTransaction>,
    pub securities: Vec<serde_json::Value>,
    pub total_investment_transactions: u32,
    pub item: PlaidItem,
}

#[derive(Debug)]
pub struct UpsertPlaidItem<'a> {
    pub user_id: Uuid,
    pub item_id: &'a str,
    pub access_token_encrypted: &'a [u8],
    pub institution_id: Option<&'a str>,
    pub institution_name: Option<&'a str>,
}
