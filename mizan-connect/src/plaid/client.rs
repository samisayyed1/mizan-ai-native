use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::config::{PlaidConfig, PlaidEnv};
use crate::error::AppError;

use super::types::{
    LinkTokenResponse, PlaidAccountsGetResponse, PlaidErrorBody, PlaidInvestmentsHoldingsResponse,
    PlaidLiabilitiesResponse, PlaidLinkTokenCreateResponse, PlaidPublicTokenExchangeResponse,
    PlaidTokenExchange, TransactionsSyncResponse,
};
use super::webhook_verifier::WebhookKeyResponse;

#[derive(Debug, Clone)]
pub struct PlaidClient {
    http: reqwest::Client,
    client_id: String,
    secret: SecretString,
    environment: PlaidEnv,
    base_url: String,
    webhook_url: Option<String>,
    redirect_uri: Option<String>,
}

impl PlaidClient {
    pub fn new(config: &PlaidConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id: config.client_id.clone(),
            secret: config.secret.clone(),
            environment: config.environment,
            base_url: config.api_base.clone(),
            webhook_url: config.webhook_url.clone(),
            redirect_uri: Some(config.redirect_uri.clone()),
        }
    }

    pub fn environment(&self) -> PlaidEnv {
        self.environment
    }

    pub async fn create_link_token(
        &self,
        user_id: &str,
        redirect_uri: Option<&str>,
    ) -> Result<LinkTokenResponse, AppError> {
        let mut body = json!({
            "client_id": self.client_id,
            "secret": self.secret.expose_secret(),
            "client_name": "Mizan",
            "country_codes": ["US", "CA"],
            "language": "en",
            "products": ["transactions", "liabilities", "investments"],
            "user": { "client_user_id": user_id },
        });
        if let Some(webhook) = self.webhook_url.as_deref() {
            body["webhook"] = json!(webhook);
        }
        if let Some(uri) = redirect_uri.or(self.redirect_uri.as_deref()) {
            body["redirect_uri"] = json!(uri);
        }

        let response: PlaidLinkTokenCreateResponse = self.post("/link/token/create", &body).await?;
        Ok(LinkTokenResponse {
            link_token: response.link_token,
            expiration: response.expiration,
            request_id: response.request_id,
        })
    }

    pub async fn exchange_public_token(
        &self,
        public_token: &str,
    ) -> Result<PlaidTokenExchange, AppError> {
        let body = json!({
            "client_id": self.client_id,
            "secret": self.secret.expose_secret(),
            "public_token": public_token,
        });
        let response: PlaidPublicTokenExchangeResponse =
            self.post("/item/public_token/exchange", &body).await?;
        Ok(PlaidTokenExchange {
            access_token: SecretString::from(response.access_token),
            item_id: response.item_id,
        })
    }

    pub async fn accounts_get(
        &self,
        access_token: &SecretString,
    ) -> Result<PlaidAccountsGetResponse, AppError> {
        let body = self.access_token_body(access_token);
        self.post("/accounts/get", &body).await
    }

    pub async fn transactions_sync(
        &self,
        access_token: &SecretString,
        cursor: Option<&str>,
    ) -> Result<TransactionsSyncResponse, AppError> {
        let mut body = self.access_token_body(access_token);
        body["count"] = json!(500);
        if let Some(cursor) = cursor {
            body["cursor"] = json!(cursor);
        }
        self.post("/transactions/sync", &body).await
    }

    pub async fn liabilities_get(
        &self,
        access_token: &SecretString,
    ) -> Result<PlaidLiabilitiesResponse, AppError> {
        let body = self.access_token_body(access_token);
        self.post("/liabilities/get", &body).await
    }

    pub async fn investments_holdings_get(
        &self,
        access_token: &SecretString,
    ) -> Result<PlaidInvestmentsHoldingsResponse, AppError> {
        let body = self.access_token_body(access_token);
        self.post("/investments/holdings/get", &body).await
    }

    pub async fn webhook_verification_key_get(
        &self,
        key_id: &str,
    ) -> Result<WebhookKeyResponse, AppError> {
        let body = json!({
            "client_id": self.client_id,
            "secret": self.secret.expose_secret(),
            "key_id": key_id,
        });
        self.post("/webhook_verification_key/get", &body).await
    }

    fn access_token_body(&self, access_token: &SecretString) -> serde_json::Value {
        json!({
            "client_id": self.client_id,
            "secret": self.secret.expose_secret(),
            "access_token": access_token.expose_secret(),
        })
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, AppError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.http.post(url).json(body).send().await.map_err(|err| {
            tracing::warn!(error = %err, plaid.path = path, "Plaid request failed");
            AppError::service_unavailable("Plaid is temporarily unavailable")
        })?;

        let status = response.status();
        let text = response.text().await.map_err(|err| {
            tracing::warn!(error = %err, plaid.path = path, "Plaid response read failed");
            AppError::service_unavailable("Plaid response could not be read")
        })?;

        if !status.is_success() {
            return Err(map_plaid_error(status, &text));
        }

        serde_json::from_str(&text).map_err(|err| {
            tracing::error!(
                error = %err,
                plaid.path = path,
                body_len = text.len(),
                "Plaid response decode failed"
            );
            AppError::internal("Plaid response could not be decoded")
        })
    }
}

fn map_plaid_error(status: StatusCode, body: &str) -> AppError {
    let parsed = serde_json::from_str::<PlaidErrorBody>(body).ok();
    let request_id = parsed.as_ref().and_then(|e| e.request_id.as_deref());
    let code = parsed
        .as_ref()
        .and_then(|e| e.error_code.as_deref())
        .unwrap_or("unknown");
    let error_type = parsed
        .as_ref()
        .and_then(|e| e.error_type.as_deref())
        .unwrap_or("unknown");
    let message = parsed
        .as_ref()
        .and_then(|e| e.display_message.as_deref().or(e.error_message.as_deref()))
        .unwrap_or("Plaid request failed");

    tracing::warn!(
        plaid.status = status.as_u16(),
        plaid.error_type = error_type,
        plaid.error_code = code,
        plaid.request_id = request_id,
        "Plaid API error"
    );

    match status {
        StatusCode::BAD_REQUEST => AppError::bad_request(message.to_string()),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            AppError::forbidden("Plaid access was denied or expired")
        }
        StatusCode::NOT_FOUND => AppError::not_found("Plaid item or account not found"),
        StatusCode::TOO_MANY_REQUESTS => {
            AppError::service_unavailable("Plaid rate limit reached; try again later")
        }
        status if status.is_server_error() => {
            AppError::service_unavailable("Plaid is temporarily unavailable")
        }
        _ => AppError::service_unavailable("Plaid request failed"),
    }
}
