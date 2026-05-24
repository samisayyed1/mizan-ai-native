//! HTTP client for Mizan Connect cloud API.
//!
//! This module provides a shared HTTP client for communicating with the
//! Mizan Connect cloud service. Both Tauri and server implementations
//! should use this client to ensure consistency.

use async_trait::async_trait;
use log::{debug, info};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::broker::{
    BrokerAccount, BrokerBrokerage, BrokerConnection, BrokerConnectionBrokerage,
    BrokerHoldingsResponse, MonthlyReport, MonthlyReportsResponse, MyTeamsResponse,
    PaginatedUniversalActivity, PlansResponse, TeamMembersResponse, UserInfo, UserTeam,
};
use crate::entitlements::{entitlements_for_plan, Entitlements};
use mizan_core::errors::{Error, Result};

use super::broker::BrokerApiClient;

/// Default timeout for API requests.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default base URL for Mizan Connect cloud service.
pub const DEFAULT_CLOUD_API_URL: &str = "https://api.mizan.app";

// ─────────────────────────────────────────────────────────────────────────────
// API Response Types (internal, for parsing cloud API responses)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiUser {
    id: String,
    email: Option<String>,
    full_name: Option<String>,
    avatar_url: Option<String>,
    locale: Option<String>,
    week_starts_on_monday: Option<bool>,
    timezone: Option<String>,
    timezone_auto_sync: Option<bool>,
    time_format: Option<i32>,
    date_format: Option<String>,
    team_id: Option<String>,
    team_role: Option<String>,
    team: Option<ApiTeam>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiTeam {
    id: String,
    name: Option<String>,
    logo_url: Option<String>,
    plan: Option<String>,
    #[serde(default)]
    subscription_status: Option<String>,
    #[serde(default)]
    subscription_current_period_end: Option<String>,
    #[serde(default)]
    subscription_cancel_at_period_end: Option<bool>,
    #[serde(default)]
    canceled_at: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    /// Explicit entitlements matrix once the cloud returns it (Contract §A).
    /// Absent on current backends — we then derive from `plan`+status.
    #[serde(default)]
    entitlements: Option<crate::entitlements::Entitlements>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct ApiErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Response of `POST /api/v1/sync/plaid/link-token`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidLinkTokenResponse {
    pub link_token: String,
    pub expiration: Option<String>,
    pub request_id: Option<String>,
}

/// Response of `POST /api/v1/sync/plaid/exchange-public-token`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidExchangePublicTokenResponse {
    pub item_id: String,
    pub accounts_synced: usize,
}

/// Stored Plaid connection returned by Mizan Connect.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiPlaidConnection {
    item_id: String,
    institution_id: Option<String>,
    institution_name: Option<String>,
    status: String,
    account_count: i64,
    last_successful_sync_at: Option<String>,
    last_error: Option<String>,
    updated_at: String,
}

/// Stored Plaid account returned by Mizan Connect.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiPlaidAccount {
    account_id: String,
    item_id: String,
    name: Option<String>,
    official_name: Option<String>,
    institution_name: Option<String>,
    account_type: Option<String>,
    subtype: Option<String>,
    mask: Option<String>,
    balances: serde_json::Value,
    updated_at: String,
}

/// Response of `POST /api/v1/billing/checkout-session`. `url` is a Stripe
/// Checkout hosted-session URL the caller opens in the user's default browser.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CheckoutSessionResponse {
    pub url: String,
}

/// Response of `POST /api/v1/billing/portal`. `url` is a Stripe Customer
/// Portal hosted-session URL.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BillingPortalResponse {
    pub url: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Connect API Client
// ─────────────────────────────────────────────────────────────────────────────

/// HTTP client for the Mizan Connect cloud API.
///
/// This client provides methods for:
/// - Fetching broker connections, accounts, and activities
/// - Getting subscription plans
/// - Getting user information
///
/// # Example
///
/// ```ignore
/// let client = ConnectApiClient::new("https://api.mizan.app", "your-token")?;
/// let connections = client.list_connections().await?;
/// ```
#[derive(Debug, Clone)]
pub struct ConnectApiClient {
    client: reqwest::Client,
    base_url: String,
    auth_header: HeaderValue,
}

impl ConnectApiClient {
    /// Create a new Connect API client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the cloud API (e.g., "https://api.mizan.app")
    /// * `access_token` - A valid JWT access token
    ///
    /// # Errors
    ///
    /// Returns an error if the access token format is invalid or the HTTP client
    /// cannot be initialized.
    pub fn new(base_url: &str, access_token: &str) -> Result<Self> {
        let auth_header = HeaderValue::from_str(&format!("Bearer {}", access_token))
            .map_err(|e| Error::Unexpected(format!("Invalid access token format: {}", e)))?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| Error::Unexpected(format!("Failed to initialize HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
        })
    }

    /// Create default headers for API requests.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(AUTHORIZATION, self.auth_header.clone());
        headers
    }

    /// Make a GET request and parse the response.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;

        self.parse_response(response).await
    }

    /// Make a JSON POST request and parse the response.
    async fn post_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;
        self.parse_response(response).await
    }

    /// Fire-and-forget POST that ignores the response body. Used by usage
    /// reporting where the caller doesn't care about the (empty) reply.
    async fn post_no_response<B: serde::Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Unexpected(format!("HTTP {}: {}", status, body)));
        }
        Ok(())
    }

    /// Parse an HTTP response, handling errors appropriately.
    async fn parse_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Unexpected(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            // Try to parse error response for a better message
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                let msg = err
                    .message
                    .or(err.error)
                    .unwrap_or_else(|| format!("HTTP {}", status));
                return Err(Error::Unexpected(format!("API error: {}", msg)));
            }
            return Err(Error::Unexpected(format!(
                "API error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }

        serde_json::from_str(&body)
            .map_err(|e| Error::Unexpected(format!("Failed to parse response: {} - {}", e, body)))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Brokerage Endpoints
    // ─────────────────────────────────────────────────────────────────────────

    /// Fetch account activities with pagination.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The broker account ID (provider's ID)
    /// * `start_date` - Optional start date filter (YYYY-MM-DD)
    /// * `end_date` - Optional end date filter (YYYY-MM-DD)
    /// * `offset` - Pagination offset
    /// * `limit` - Maximum number of results per page
    pub async fn get_account_activities(
        &self,
        _account_id: &str,
        _start_date: Option<&str>,
        _end_date: Option<&str>,
        _offset: Option<i64>,
        _limit: Option<i64>,
    ) -> Result<PaginatedUniversalActivity> {
        Err(Error::Unexpected(
            "Per-account activity fetch is deprecated; use Plaid server-side sync".to_string(),
        ))
    }

    /// Disconnect a Plaid Item from Gold live sync.
    pub async fn delete_connection(&self, connection_id: &str) -> Result<()> {
        let url = format!(
            "{}/api/v1/sync/plaid/connections/{}",
            self.base_url, connection_id
        );

        debug!("[ConnectApi] Disconnecting Plaid Item: {}", url);

        let response = self
            .client
            .delete(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Failed to disconnect broker: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Unexpected(format!(
                "Disconnect failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }
        Ok(())
    }

    /// Fetch current holdings for a broker account.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The broker account ID (provider's ID)
    pub async fn get_account_holdings(&self, _account_id: &str) -> Result<BrokerHoldingsResponse> {
        Err(Error::Unexpected(
            "Per-account holdings fetch is deprecated; use Plaid server-side sync".to_string(),
        ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // User & Subscription Endpoints
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the current user's information.
    pub async fn get_user_info(&self) -> Result<UserInfo> {
        let api_user: Option<ApiUser> = self.get("/api/v1/user/me").await?;

        let user =
            api_user.ok_or_else(|| Error::Unexpected("No user info returned".to_string()))?;

        Ok(UserInfo {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            avatar_url: user.avatar_url,
            locale: user.locale,
            week_starts_on_monday: user.week_starts_on_monday,
            timezone: user.timezone,
            timezone_auto_sync: user.timezone_auto_sync,
            time_format: user.time_format,
            date_format: user.date_format,
            team_id: user.team_id,
            team_role: user.team_role,
            team: user.team.map(|t| UserTeam {
                id: t.id,
                name: t.name.unwrap_or_default(),
                logo_url: t.logo_url,
                plan: t.plan,
                subscription_status: t.subscription_status,
                subscription_current_period_end: t.subscription_current_period_end,
                subscription_cancel_at_period_end: t.subscription_cancel_at_period_end,
                canceled_at: t.canceled_at,
                country_code: t.country_code,
                created_at: t.created_at,
                entitlements: t.entitlements,
            }),
        })
    }

    /// Open Stripe Checkout for a subscription. Returns the hosted-session URL
    /// the caller should open in the user's default browser.
    pub async fn create_checkout_session(
        &self,
        plan: &str,
        interval: &str,
    ) -> Result<CheckoutSessionResponse> {
        let body = serde_json::json!({ "plan": plan, "interval": interval });
        self.post_json("/api/v1/billing/checkout-session", &body)
            .await
    }

    /// Open the Stripe Customer Portal for self-service plan management.
    /// Returns `Err` (404) when the user has no Stripe customer yet — caller
    /// should fall back to the upgrade modal in that case.
    pub async fn create_billing_portal_session(&self) -> Result<BillingPortalResponse> {
        let body = serde_json::json!({});
        self.post_json("/api/v1/billing/portal", &body).await
    }

    /// Fire-and-forget usage report. The cloud increments AI-credit balances
    /// and ledger rows; the desktop calls this after metered local actions
    /// (broker poll trigger, CSV import, market refresh) so the cloud has the
    /// authoritative count.
    pub async fn report_usage(&self, metric: &str, units: i32) -> Result<()> {
        let body = serde_json::json!({ "metric": metric, "units": units });
        self.post_no_response("/api/v1/usage", &body).await
    }

    /// Resolve the current user's [`Entitlements`].
    ///
    /// Prefers an explicit `entitlements` object from the cloud (Contract §A)
    /// when present; otherwise derives the matrix from the team's `plan` slug +
    /// `subscription_status` via [`entitlements_for_plan`].
    pub async fn get_entitlements(&self) -> Result<Entitlements> {
        let user_info = self.get_user_info().await?;
        let team = match user_info.team {
            Some(t) => t,
            None => return Ok(Entitlements::default()),
        };

        if let Some(explicit) = team.entitlements {
            return Ok(explicit);
        }
        Ok(entitlements_for_plan(
            team.plan.as_deref(),
            team.subscription_status.as_deref(),
        ))
    }

    /// List the current user's stored monthly wealth reports (M3.6).
    /// Returns up to `limit` rows newest-first; the cloud clamps to [1, 24].
    pub async fn list_monthly_reports(&self, limit: i64) -> Result<MonthlyReportsResponse> {
        let path = format!("/api/v1/reports/monthly?limit={}", limit.clamp(1, 24));
        self.get(&path).await
    }

    /// Enqueue an on-demand monthly report regeneration for the current
    /// period. Idempotent: a second call within the same calendar month
    /// returns the existing row (HTTP 200 instead of 202 — same shape).
    pub async fn request_monthly_report(&self) -> Result<MonthlyReport> {
        self.post_json("/api/v1/reports/monthly", &serde_json::json!({}))
            .await
    }

    /// List every team the caller is a member of (M5.2).
    pub async fn list_my_teams(&self) -> Result<MyTeamsResponse> {
        self.get("/api/v1/me/teams").await
    }

    /// List members of a specific team (M5.2). Returns 403 if the caller
    /// isn't a member of the team.
    pub async fn list_team_members(&self, team_id: &str) -> Result<TeamMembersResponse> {
        let path = format!("/api/v1/teams/{}/members", team_id);
        self.get(&path).await
    }

    /// Get available subscription plans (authenticated).
    pub async fn get_subscription_plans(&self) -> Result<PlansResponse> {
        self.get("/api/v1/subscription/plans").await
    }

    /// Create a short-lived Plaid Link token for Gold live sync.
    pub async fn create_plaid_link_token(
        &self,
        redirect_uri: Option<&str>,
    ) -> Result<PlaidLinkTokenResponse> {
        let body = serde_json::json!({ "redirectUri": redirect_uri });
        self.post_json("/api/v1/sync/plaid/link-token", &body).await
    }

    /// Exchange a Plaid public token. The access token is stored only by
    /// Mizan Connect in its encrypted server-side boundary.
    pub async fn exchange_plaid_public_token(
        &self,
        public_token: &str,
    ) -> Result<PlaidExchangePublicTokenResponse> {
        let body = serde_json::json!({ "publicToken": public_token });
        self.post_json("/api/v1/sync/plaid/exchange-public-token", &body)
            .await
    }

    /// Trigger Plaid cursor sync for all connected Items.
    pub async fn sync_plaid_data(&self) -> Result<()> {
        let body = serde_json::json!({});
        let _: serde_json::Value = self.post_json("/api/v1/sync/plaid/sync", &body).await?;
        Ok(())
    }

    /// Check if the current user's plan includes broker sync.
    ///
    /// Returns true when the user has an active subscription AND their plan
    /// includes Gold Plaid live sync.
    pub async fn has_broker_sync(&self) -> Result<bool> {
        let user_info = self.get_user_info().await?;

        let team = match &user_info.team {
            Some(t) => t,
            None => {
                debug!("[ConnectApi] No team info, broker sync not available");
                return Ok(false);
            }
        };

        let is_active = matches!(
            team.subscription_status.as_deref(),
            Some("active") | Some("trialing")
        );
        if !is_active {
            debug!("[ConnectApi] No active subscription, broker sync not available");
            return Ok(false);
        }

        let entitlements =
            entitlements_for_plan(team.plan.as_deref(), team.subscription_status.as_deref());
        debug!(
            "[ConnectApi] Plaid sync entitlement for plan {:?}: {}",
            team.plan, entitlements.broker_sync
        );
        Ok(entitlements.broker_sync)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BrokerApiClient Trait Implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl BrokerApiClient for ConnectApiClient {
    /// Fetch all Plaid connections for the user.
    async fn list_connections(&self) -> Result<Vec<BrokerConnection>> {
        let url = format!("{}/api/v1/sync/plaid/connections", self.base_url);
        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Unexpected(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(Error::Unexpected(format!(
                "API error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }

        let raw: Vec<ApiPlaidConnection> = serde_json::from_str(&body).map_err(|e| {
            Error::Unexpected(format!("Failed to parse connections: {} - {}", e, body))
        })?;

        let connections: Vec<BrokerConnection> = raw
            .into_iter()
            .map(|c| {
                BrokerConnection {
                    id: c.item_id.clone(),
                    brokerage: Some(BrokerConnectionBrokerage {
                        id: c.institution_id,
                        slug: None,
                        name: c.institution_name.clone(),
                        display_name: c.institution_name,
                        aws_s3_logo_url: None,
                        aws_s3_square_logo_url: None,
                    }),
                    connection_type: Some("plaid".to_string()),
                    status: Some(if c.last_error.is_some() {
                        "needs_attention".to_string()
                    } else {
                        c.status
                    }),
                    disabled: false,
                    disabled_date: None,
                    updated_at: c.last_successful_sync_at.or(Some(c.updated_at)),
                    name: Some(format!("{} accounts", c.account_count)),
                }
            })
            .collect();

        let count = connections.len();
        info!("[ConnectApi] Fetched {} broker connections", count);
        Ok(connections)
    }

    /// Fetch all Plaid accounts for the user.
    async fn list_accounts(
        &self,
        _authorization_ids: Option<Vec<String>>,
    ) -> Result<Vec<BrokerAccount>> {
        let url = format!("{}/api/v1/sync/plaid/accounts", self.base_url);
        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Unexpected(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(Error::Unexpected(format!(
                "API error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }

        let accounts: Vec<ApiPlaidAccount> = serde_json::from_str(&body).map_err(|e| {
            Error::Unexpected(format!("Failed to parse accounts: {} - {}", e, body))
        })?;

        Ok(accounts
            .into_iter()
            .map(|a| BrokerAccount {
                id: Some(a.account_id),
                name: a.name.or(a.official_name),
                account_number: a.mask,
                account_type: a.account_type.clone(),
                currency: a
                    .balances
                    .get("iso_currency_code")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                institution_name: a.institution_name,
                balance: None,
                meta: Some(serde_json::json!({
                    "provider": "plaid",
                    "itemId": a.item_id,
                    "type": a.account_type,
                    "subtype": a.subtype,
                    "balances": a.balances,
                    "updatedAt": a.updated_at,
                })),
                owner: None,
                brokerage_authorization: None,
                created_date: None,
                sync_status: None,
                status: Some("connected".to_string()),
                raw_type: a.account_type,
                is_paper: false,
                sync_enabled: true,
                shared_with_household: false,
            })
            .collect())
    }

    /// Fetch all available brokerages (not implemented in REST API).
    async fn list_brokerages(&self) -> Result<Vec<BrokerBrokerage>> {
        // The REST API doesn't have a separate brokerages endpoint
        // Brokerages are embedded in connections
        Ok(vec![])
    }

    /// Fetch account activities with pagination.
    async fn get_account_activities(
        &self,
        account_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<PaginatedUniversalActivity> {
        // Delegate to the inherent method
        ConnectApiClient::get_account_activities(
            self, account_id, start_date, end_date, offset, limit,
        )
        .await
    }

    /// Fetch current holdings for a broker account.
    async fn get_account_holdings(&self, account_id: &str) -> Result<BrokerHoldingsResponse> {
        // Delegate to the inherent method
        ConnectApiClient::get_account_holdings(self, account_id).await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public (Unauthenticated) API Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Fetch subscription plans without authentication.
///
/// This function is used to display pricing information before the user logs in.
/// The plans endpoint is public and does not require authentication.
///
/// # Arguments
///
/// * `base_url` - The base URL of the cloud API (e.g., "https://api.mizan.app")
///
/// # Example
///
/// ```ignore
/// let plans = fetch_subscription_plans_public("https://api.mizan.app").await?;
/// ```
pub async fn fetch_subscription_plans_public(base_url: &str) -> Result<PlansResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .build()
        .map_err(|e| Error::Unexpected(format!("Failed to initialize HTTP client: {}", e)))?;

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{}/api/v1/subscription/plans", base_url);

    let response = client
        .get(&url)
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await
        .map_err(|e| Error::Unexpected(format!("Request failed: {}", e)))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| Error::Unexpected(format!("Failed to read response: {}", e)))?;

    if !status.is_success() {
        return Err(Error::Unexpected(format!(
            "API error {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        )));
    }

    serde_json::from_str(&body)
        .map_err(|e| Error::Unexpected(format!("Failed to parse plans response: {} - {}", e, body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_client_creation() {
        let client = ConnectApiClient::new("https://api.mizan.app", "test-token");
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_url_normalization() {
        let client = ConnectApiClient::new("https://api.mizan.app/", "test-token").unwrap();
        assert_eq!(client.base_url, "https://api.mizan.app");
    }

    /// Verifies Plaid Link token creation uses the Plaid-only Mizan Connect
    /// endpoint and never returns or handles access tokens in the desktop app.
    #[tokio::test]
    async fn create_plaid_link_token_signs_request_and_parses_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v1/sync/plaid/link-token"))
            .and(header("authorization", "Bearer mizan-test-jwt"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "linkToken": "link-sandbox-abc",
                "expiration": "2026-05-05T12:34:56Z",
                "requestId": "req_123",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ConnectApiClient::new(&server.uri(), "mizan-test-jwt").unwrap();
        let resp = client
            .create_plaid_link_token(None)
            .await
            .expect("Plaid link-token call");
        assert_eq!(resp.link_token, "link-sandbox-abc");
        assert_eq!(resp.expiration.as_deref(), Some("2026-05-05T12:34:56Z"));
        assert_eq!(resp.request_id.as_deref(), Some("req_123"));
    }

    #[tokio::test]
    async fn exchange_plaid_public_token_parses_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v1/sync/plaid/exchange-public-token"))
            .and(header("authorization", "Bearer mizan-test-jwt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "itemId": "item-abc",
                "accountsSynced": 3,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ConnectApiClient::new(&server.uri(), "mizan-test-jwt").unwrap();
        let response = client
            .exchange_plaid_public_token("public-sandbox-abc")
            .await
            .expect("public-token exchange");
        assert_eq!(response.item_id, "item-abc");
        assert_eq!(response.accounts_synced, 3);
    }

    #[tokio::test]
    async fn create_plaid_link_token_propagates_rate_limit_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v1/sync/plaid/link-token"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "error": {
                    "code": "too_many_requests",
                    "message": "too many Plaid Link token requests; try again later",
                    "request_id": "abc",
                }
            })))
            .mount(&server)
            .await;

        let client = ConnectApiClient::new(&server.uri(), "mizan-test-jwt").unwrap();
        let err = client.create_plaid_link_token(None).await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("API error"), "got: {msg}");
    }

    #[tokio::test]
    async fn list_connections_parses_plaid_array_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/sync/plaid/connections"))
            .and(header("authorization", "Bearer mizan-test-jwt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "itemId": "item-plaid-1",
                    "institutionId": "ins_109508",
                    "institutionName": "Plaid Bank",
                    "status": "connected",
                    "accountCount": 2,
                    "lastSuccessfulSyncAt": "2026-05-05T10:00:01Z",
                    "lastError": null,
                    "updatedAt": "2026-05-05T10:00:02Z"
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let client = ConnectApiClient::new(&server.uri(), "mizan-test-jwt").unwrap();
        let connections = client
            .list_connections()
            .await
            .expect("list_connections should accept Plaid response");

        assert_eq!(connections.len(), 1);
        let only = &connections[0];
        assert_eq!(only.id, "item-plaid-1");
        assert_eq!(only.status.as_deref(), Some("connected"));
        assert!(!only.disabled);
        let brk = only.brokerage.as_ref().expect("brokerage present");
        assert_eq!(brk.id.as_deref(), Some("ins_109508"));
        assert_eq!(brk.display_name.as_deref(), Some("Plaid Bank"));
    }

    #[tokio::test]
    async fn list_accounts_parses_plaid_array_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/sync/plaid/accounts"))
            .and(header("authorization", "Bearer mizan-test-jwt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "accountId": "acct-1",
                    "itemId": "item-plaid-1",
                    "name": "Checking",
                    "officialName": "Plaid Gold Checking",
                    "institutionName": "Plaid Bank",
                    "accountType": "depository",
                    "subtype": "checking",
                    "mask": "1234",
                    "balances": { "current": 1000, "iso_currency_code": "USD" },
                    "updatedAt": "2026-05-05T10:00:02Z"
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let client = ConnectApiClient::new(&server.uri(), "mizan-test-jwt").unwrap();
        let accounts = client
            .list_accounts(None)
            .await
            .expect("list_accounts should accept Plaid response");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].account_number.as_deref(), Some("1234"));
        assert_eq!(accounts[0].institution_name.as_deref(), Some("Plaid Bank"));
    }

}
