//! Typed SnapTrade HTTP client.
//!
//! Auth model:
//!   - `clientId` + `timestamp` go on every request as query params.
//!   - `Signature` header is an HMAC-SHA256(consumer_key, canonical_json)
//!     where canonical_json is built by `signing::canonical_string`.
//!   - Per-user endpoints additionally take `userId` + `userSecret`
//!     query params.
//!
//! Errors are mapped to `AppError` so the handler layer can decide
//! how loudly to surface them — connect-portal failures bubble up as
//! 502 / 503, signature failures are 500 (logic error in our auth).

use reqwest::{Method, StatusCode};
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use serde_json::Value;
use time::OffsetDateTime;

use crate::config::SnapTradeConfig;
use crate::error::{AppError, ErrorCode};

use super::signing;
use super::types::{
    BrokerageAuthorization, LoginPortalRequest, LoginPortalResponse, RegisterUserRequest,
    RegisterUserResponse, SnapTradeAccount, SnapTradeActivity, SnapTradeErrorBody,
    SnapTradePosition,
};

#[derive(Debug, Clone)]
pub struct SnapTradeClient {
    http: reqwest::Client,
    client_id: String,
    consumer_key: SecretString,
    api_base: String,
}

impl SnapTradeClient {
    pub fn new(config: &SnapTradeConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id: config.client_id.clone(),
            consumer_key: config.consumer_key.clone(),
            api_base: config.api_base.clone(),
        }
    }

    // ─── User registration ──────────────────────────────────────────

    /// Register a new SnapTrade user. The returned `user_secret` MUST be
    /// stored encrypted forever — losing it means the user has to
    /// re-link every brokerage from scratch.
    pub async fn register_user(
        &self,
        mizan_user_id: &str,
    ) -> Result<RegisterUserResponse, AppError> {
        let body = serde_json::to_value(RegisterUserRequest {
            user_id: mizan_user_id,
        })
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("serialize register: {e}")))?;
        self.post_signed("/api/v1/snapTrade/registerUser", None, &body)
            .await
    }

    // ─── Login portal ───────────────────────────────────────────────

    /// Generate a one-time URL that opens SnapTrade's connection portal
    /// in a browser. The user picks a brokerage there, completes OAuth,
    /// then SnapTrade redirects to `custom_redirect` (our deep link).
    pub async fn login_portal(
        &self,
        user_id: &str,
        user_secret: &str,
        custom_redirect: Option<&str>,
        broker: Option<&str>,
    ) -> Result<LoginPortalResponse, AppError> {
        let body = serde_json::to_value(LoginPortalRequest {
            immediate_redirect: true,
            custom_redirect,
            broker,
        })
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("serialize portal: {e}")))?;
        self.post_signed(
            "/api/v1/snapTrade/login",
            Some((user_id, user_secret)),
            &body,
        )
        .await
    }

    // ─── Authorizations / connections ───────────────────────────────

    pub async fn list_authorizations(
        &self,
        user_id: &str,
        user_secret: &str,
    ) -> Result<Vec<BrokerageAuthorization>, AppError> {
        self.get_signed("/api/v1/authorizations", Some((user_id, user_secret)))
            .await
    }

    pub async fn disconnect_authorization(
        &self,
        user_id: &str,
        user_secret: &str,
        authorization_id: &str,
    ) -> Result<(), AppError> {
        let path = format!("/api/v1/authorizations/{authorization_id}");
        self.delete_signed(&path, Some((user_id, user_secret)))
            .await
    }

    // ─── Accounts / holdings / activities ───────────────────────────

    pub async fn list_accounts(
        &self,
        user_id: &str,
        user_secret: &str,
    ) -> Result<Vec<SnapTradeAccount>, AppError> {
        self.get_signed("/api/v1/accounts", Some((user_id, user_secret)))
            .await
    }

    pub async fn list_positions(
        &self,
        user_id: &str,
        user_secret: &str,
        account_id: &str,
    ) -> Result<Vec<SnapTradePosition>, AppError> {
        let path = format!("/api/v1/accounts/{account_id}/positions");
        self.get_signed(&path, Some((user_id, user_secret))).await
    }

    pub async fn list_activities(
        &self,
        user_id: &str,
        user_secret: &str,
        account_id: &str,
    ) -> Result<Vec<SnapTradeActivity>, AppError> {
        // SnapTrade actually exposes activities via `/activities` with a
        // `?accounts=<id>` filter, not nested. We use the canonical
        // endpoint path here.
        let path = "/api/v1/activities";
        let extra_query = format!("accounts={account_id}");
        self.get_signed_with_extra_query(path, Some((user_id, user_secret)), &extra_query)
            .await
    }

    // ─── Internal: signed request plumbing ──────────────────────────

    async fn post_signed<T: DeserializeOwned>(
        &self,
        path: &str,
        user_creds: Option<(&str, &str)>,
        body: &Value,
    ) -> Result<T, AppError> {
        self.signed_request(Method::POST, path, user_creds, "", Some(body))
            .await
    }

    async fn get_signed<T: DeserializeOwned>(
        &self,
        path: &str,
        user_creds: Option<(&str, &str)>,
    ) -> Result<T, AppError> {
        self.signed_request(Method::GET, path, user_creds, "", None)
            .await
    }

    async fn get_signed_with_extra_query<T: DeserializeOwned>(
        &self,
        path: &str,
        user_creds: Option<(&str, &str)>,
        extra: &str,
    ) -> Result<T, AppError> {
        self.signed_request(Method::GET, path, user_creds, extra, None)
            .await
    }

    async fn delete_signed(
        &self,
        path: &str,
        user_creds: Option<(&str, &str)>,
    ) -> Result<(), AppError> {
        // 204 is the success path; some endpoints return an empty body.
        let _: serde_json::Value = self
            .signed_request(Method::DELETE, path, user_creds, "", None)
            .await?;
        Ok(())
    }

    async fn signed_request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        user_creds: Option<(&str, &str)>,
        extra_query: &str,
        body: Option<&Value>,
    ) -> Result<T, AppError> {
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        // Assemble query string. SnapTrade requires `clientId` +
        // `timestamp` on every request; per-user calls append
        // `userId` + `userSecret`.
        let mut q = format!("clientId={}&timestamp={timestamp}", self.client_id);
        if let Some((uid, secret)) = user_creds {
            q.push_str(&format!(
                "&userId={}&userSecret={}",
                urlencoding::encode(uid),
                urlencoding::encode(secret)
            ));
        }
        if !extra_query.is_empty() {
            q.push('&');
            q.push_str(extra_query);
        }

        let canonical = signing::canonical_string(path, &q, body);
        let signature = signing::sign(&self.consumer_key, &canonical);

        let url = format!("{}{path}?{q}", self.api_base.trim_end_matches('/'));
        let mut req = self
            .http
            .request(method.clone(), &url)
            .header("Signature", &signature)
            .header("Accept", "application/json");
        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await.map_err(|e| {
            AppError::new(
                ErrorCode::ServiceUnavailable,
                format!("snaptrade network: {e}"),
            )
        })?;

        let status = resp.status();
        let bytes = resp.bytes().await.map_err(|e| {
            AppError::new(
                ErrorCode::ServiceUnavailable,
                format!("snaptrade read body: {e}"),
            )
        })?;

        if !status.is_success() {
            let err: SnapTradeErrorBody =
                serde_json::from_slice(&bytes).unwrap_or(SnapTradeErrorBody {
                    code: None,
                    detail: None,
                    message: None,
                });
            let detail = err
                .detail
                .or(err.message)
                .or(err.code.clone())
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
            // 401 with code 1076 = signature problem on our side; surface
            // it as Internal so it shows up in error budgets.
            let code = match status {
                StatusCode::UNAUTHORIZED => ErrorCode::Unauthorized,
                StatusCode::FORBIDDEN => ErrorCode::Forbidden,
                StatusCode::NOT_FOUND => ErrorCode::NotFound,
                StatusCode::TOO_MANY_REQUESTS => ErrorCode::TooManyRequests,
                StatusCode::BAD_REQUEST => ErrorCode::BadRequest,
                s if s.is_server_error() => ErrorCode::BadGateway,
                _ => ErrorCode::Internal,
            };
            return Err(AppError::new(code, format!("snaptrade {status}: {detail}")));
        }

        // 204 No Content paths return an empty body; deserialize as null
        // and let the caller's type cope (we use serde_json::Value for
        // DELETE).
        if bytes.is_empty() {
            return serde_json::from_str("null")
                .map_err(|e| AppError::new(ErrorCode::Internal, format!("snaptrade decode: {e}")));
        }

        serde_json::from_slice(&bytes)
            .map_err(|e| AppError::new(ErrorCode::Internal, format!("snaptrade decode: {e}")))
    }
}
