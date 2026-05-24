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

#[derive(Debug)]
pub struct UpsertPlaidItem<'a> {
    pub user_id: Uuid,
    pub item_id: &'a str,
    pub access_token_encrypted: &'a [u8],
    pub institution_id: Option<&'a str>,
    pub institution_name: Option<&'a str>,
}
