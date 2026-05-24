//! Service for interacting with Mizan Connect cloud API.
//!
//! This service wraps the ConnectApiClient with keyring token retrieval,
//! providing a simple interface for cloud API operations.

use std::sync::Arc;

use mizan_connect::{
    ensure_valid_access_token, ConnectApiClient, Entitlements, MonthlyReport,
    MonthlyReportsResponse, MyTeamsResponse, TeamMembersResponse, TokenLifecycleConfig,
    TokenLifecycleState, DEFAULT_CLOUD_API_URL,
};
use mizan_core::secrets::SecretStore;

/// Returns true when broker/connect sync was compiled in.
pub fn is_connect_sync_enabled() -> bool {
    cfg!(feature = "connect-sync")
}

/// Returns true when device sync was compiled in.
pub fn is_device_sync_enabled() -> bool {
    cfg!(feature = "device-sync")
}

/// Returns true when any cloud sync feature is compiled in.
pub fn is_cloud_sync_enabled() -> bool {
    is_connect_sync_enabled() || is_device_sync_enabled()
}

/// Returns the cloud API base URL when a sync feature is enabled.
pub fn cloud_api_base_url() -> Option<String> {
    if !is_cloud_sync_enabled() {
        return None;
    }

    option_env!("CONNECT_API_URL")
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| Some(DEFAULT_CLOUD_API_URL.to_string()))
}

fn connect_auth_url() -> Option<String> {
    option_env!("CONNECT_AUTH_URL")
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

fn connect_auth_publishable_key() -> Option<String> {
    option_env!("CONNECT_AUTH_PUBLISHABLE_KEY")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn token_lifecycle_config() -> Option<TokenLifecycleConfig> {
    let auth_url = connect_auth_url()?;
    let publishable_key = connect_auth_publishable_key()?;
    Some(TokenLifecycleConfig::new(auth_url, publishable_key))
}

/// Service for interacting with Mizan Connect cloud API.
///
/// This service handles keyring token retrieval and provides
/// convenient methods for common cloud API operations.
pub struct ConnectService {
    secret_store: Arc<dyn SecretStore>,
    token_lifecycle: Arc<TokenLifecycleState>,
}

impl ConnectService {
    /// Create a new ConnectService instance.
    pub fn new(secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            secret_store,
            token_lifecycle: Arc::new(TokenLifecycleState::new()),
        }
    }

    /// Returns a valid access token, refreshing it through Supabase when needed.
    pub async fn get_valid_access_token(&self) -> Result<String, String> {
        if !is_cloud_sync_enabled() {
            return Err("Cloud sync feature is disabled in this build.".to_string());
        }

        let config = token_lifecycle_config();
        ensure_valid_access_token(
            self.secret_store.as_ref(),
            self.token_lifecycle.as_ref(),
            config.as_ref(),
        )
        .await
        .map_err(|err| err.to_string())
    }

    pub async fn clear_cached_token(&self) {
        self.token_lifecycle.clear_cache().await;
    }

    /// Get an authenticated API client using the stored access token.
    ///
    /// # Returns
    ///
    /// Returns `Ok(ConnectApiClient)` if a valid token is found and the client
    /// can be created, or `Err(String)` if no token is configured or an error occurs.
    pub async fn get_api_client(&self) -> Result<ConnectApiClient, String> {
        if !is_connect_sync_enabled() {
            return Err("Connect sync feature is disabled in this build.".to_string());
        }

        let cloud_api_base_url = cloud_api_base_url().ok_or_else(|| {
            "Cloud API base URL is unavailable. Connect API operations are disabled.".to_string()
        })?;

        let access_token = self.get_valid_access_token().await?;

        ConnectApiClient::new(&cloud_api_base_url, &access_token).map_err(|e| e.to_string())
    }

    /// Check if the current user's plan includes broker sync.
    ///
    /// Returns `Ok(true)` only when the user has an active subscription
    /// on a plan that includes broker sync (i.e. not "basic").
    pub async fn has_broker_sync(&self) -> Result<bool, String> {
        let client = self.get_api_client().await?;
        client.has_broker_sync().await.map_err(|e| e.to_string())
    }

    /// Resolve the current user's entitlements matrix. Callers that want a
    /// safe default for signed-out users should fall back to
    /// [`mizan_connect::Entitlements::default`] on `Err`.
    pub async fn get_entitlements(&self) -> Result<Entitlements, String> {
        let client = self.get_api_client().await?;
        client.get_entitlements().await.map_err(|e| e.to_string())
    }

    /// List the user's stored monthly AI wealth reports (M3.6).
    pub async fn list_monthly_reports(&self, limit: i64) -> Result<MonthlyReportsResponse, String> {
        let client = self.get_api_client().await?;
        client
            .list_monthly_reports(limit)
            .await
            .map_err(|e| e.to_string())
    }

    /// Enqueue an on-demand monthly report regeneration (M3.6).
    pub async fn request_monthly_report(&self) -> Result<MonthlyReport, String> {
        let client = self.get_api_client().await?;
        client
            .request_monthly_report()
            .await
            .map_err(|e| e.to_string())
    }

    /// List every team the caller is a member of (M5.2).
    #[allow(dead_code)] // called via commands::teams in lib.rs invoke_handler
    pub async fn list_my_teams(&self) -> Result<MyTeamsResponse, String> {
        let client = self.get_api_client().await?;
        client.list_my_teams().await.map_err(|e| e.to_string())
    }

    /// Roster of one team (M5.2). Returns 403 when the caller isn't a member.
    #[allow(dead_code)] // called via commands::teams in lib.rs invoke_handler
    pub async fn list_team_members(&self, team_id: &str) -> Result<TeamMembersResponse, String> {
        let client = self.get_api_client().await?;
        client
            .list_team_members(team_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// Fetch a Stripe Checkout URL for the requested plan/interval. The
    /// caller opens this in the user's default browser; on return, focus
    /// listeners invalidate the entitlements query so the new plan unlocks.
    pub async fn create_checkout_url(&self, plan: &str, interval: &str) -> Result<String, String> {
        let client = self.get_api_client().await?;
        client
            .create_checkout_session(plan, interval)
            .await
            .map(|r| r.url)
            .map_err(|e| e.to_string())
    }

    /// Fetch a Stripe Customer Portal URL for self-service plan management.
    pub async fn create_billing_portal_url(&self) -> Result<String, String> {
        let client = self.get_api_client().await?;
        client
            .create_billing_portal_session()
            .await
            .map(|r| r.url)
            .map_err(|e| e.to_string())
    }

    /// Fire-and-forget usage report. Failures are logged but don't abort the
    /// caller — the cloud's authoritative reading of /user/me corrects any
    /// drift on the next refresh.
    pub async fn report_usage(&self, metric: &str, units: i32) {
        let Ok(client) = self.get_api_client().await else {
            return;
        };
        if let Err(e) = client.report_usage(metric, units).await {
            log::warn!("usage report failed (metric={metric}, units={units}): {e}");
        }
    }
}
