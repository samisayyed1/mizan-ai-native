//! SnapTrade request/response DTOs.
//!
//! Only the shapes we actually use are typed — SnapTrade returns a lot
//! of metadata we currently ignore. Unknown fields are silently
//! dropped via `#[serde(deny_unknown_fields)]` being deliberately
//! omitted, so future schema additions don't break us.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// ────────────────────────────────────────────────────────────────────
// Registration

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserRequest<'a> {
    pub user_id: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserResponse {
    pub user_id: String,
    pub user_secret: String,
}

// ────────────────────────────────────────────────────────────────────
// Login portal — opens the SnapTrade connection portal in a browser.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPortalRequest<'a> {
    /// Where SnapTrade should send the user after they finish linking.
    pub immediate_redirect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_redirect: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPortalResponse {
    pub redirect_uri: String,
    /// Session id SnapTrade attaches to the callback URL — useful for
    /// correlating return events to the right user.
    #[serde(default)]
    pub session_id: Option<String>,
}

// ────────────────────────────────────────────────────────────────────
// Brokerage authorizations (= connections)

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerageAuthorization {
    pub id: String,
    #[serde(default)]
    pub created_date: Option<String>,
    #[serde(default)]
    pub updated_date: Option<String>,
    pub brokerage: BrokerageRef,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub disabled_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerageRef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

// ────────────────────────────────────────────────────────────────────
// Accounts

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeAccount {
    pub id: String,
    pub brokerage_authorization: String,
    pub name: String,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub institution_name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    pub balance: AccountBalance,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    #[serde(default)]
    pub total: Option<MoneyAmount>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MoneyAmount {
    pub amount: f64,
    pub currency: String,
}

// ────────────────────────────────────────────────────────────────────
// Positions / holdings

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradePosition {
    pub symbol: PositionSymbol,
    pub units: f64,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub average_purchase_price: Option<f64>,
    #[serde(default)]
    pub open_pnl: Option<f64>,
    #[serde(default)]
    pub fractional_units: Option<f64>,
    #[serde(default)]
    pub currency: Option<MoneyAmount>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionSymbol {
    pub symbol: SymbolInner,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInner {
    pub id: String,
    pub symbol: String,
    #[serde(default)]
    pub description: Option<String>,
    pub currency: Currency,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
}

// ────────────────────────────────────────────────────────────────────
// Activities / transactions

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeActivity {
    pub id: String,
    pub account: ActivityAccountRef,
    #[serde(default)]
    pub amount: Option<f64>,
    pub currency: Currency,
    /// `BUY`, `SELL`, `DIVIDEND`, `CONTRIBUTION`, `WITHDRAWAL`,
    /// `TAX`, `FEE`, `INTEREST`, `STOCK_DIVIDEND`, …
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub symbol: Option<SymbolInner>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub units: Option<f64>,
    pub trade_date: String,
    #[serde(default)]
    pub settlement_date: Option<String>,
    #[serde(default)]
    pub fee: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityAccountRef {
    pub id: String,
}

// ────────────────────────────────────────────────────────────────────
// API client response envelopes (for clean error mapping)

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeErrorBody {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

// ────────────────────────────────────────────────────────────────────
// Mizan-side typed responses surfaced via the HTTP handlers

/// Slim envelope returned by `POST /v1/sync/snaptrade/login-portal`.
/// Frontend opens `redirect_uri` in the system browser; SnapTrade
/// posts back to our deep-link.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPortalEnvelope {
    pub redirect_uri: String,
    pub session_id: Option<String>,
}

/// One row in `GET /v1/sync/snaptrade/connections` — the per-user
/// view of brokerage authorizations the desktop renders in the
/// "Connected brokerages" card.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEnvelope {
    pub authorization_id: String,
    pub brokerage_name: String,
    pub display_name: Option<String>,
    pub connected_at_ms: Option<i64>,
    pub disabled: bool,
    pub disabled_at_ms: Option<i64>,
}

impl ConnectionEnvelope {
    pub fn from_authorization(auth: BrokerageAuthorization) -> Self {
        Self {
            authorization_id: auth.id,
            brokerage_name: auth.brokerage.name,
            display_name: auth.brokerage.display_name,
            connected_at_ms: auth.created_date.as_deref().and_then(parse_rfc3339_ms),
            disabled: auth.disabled.unwrap_or(false),
            disabled_at_ms: auth.disabled_date.as_deref().and_then(parse_rfc3339_ms),
        }
    }
}

fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|d| (d.unix_timestamp_nanos() / 1_000_000) as i64)
}

/// Summary returned by `POST /v1/sync/snaptrade/sync`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub accounts_synced: u32,
    pub positions_synced: u32,
    pub activities_synced: u32,
}
