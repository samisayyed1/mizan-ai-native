//! Application configuration loaded from environment / `.env`.
//!
//! Validated at startup via [`Config::load`]. Any validation failure aborts
//! the process before binding a port — fail fast.

use std::str::FromStr;

use figment::providers::Env;
use figment::Figment;
use secrecy::SecretString;
use serde::Deserialize;

/// Operating environment for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppEnv {
    Development,
    Staging,
    Production,
    Test,
}

impl AppEnv {
    pub fn is_production(self) -> bool {
        matches!(self, AppEnv::Production)
    }
    pub fn is_test(self) -> bool {
        matches!(self, AppEnv::Test)
    }
}

impl FromStr for AppEnv {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(AppEnv::Development),
            "staging" | "stage" => Ok(AppEnv::Staging),
            "production" | "prod" => Ok(AppEnv::Production),
            "test" => Ok(AppEnv::Test),
            other => Err(format!("unknown APP_ENV value: {other}")),
        }
    }
}

/// Logging output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

/// Plaid API environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaidEnv {
    Sandbox,
    Development,
    Production,
}

impl PlaidEnv {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Development => "development",
            Self::Production => "production",
        }
    }

    pub fn api_base(self) -> &'static str {
        match self {
            Self::Sandbox => "https://sandbox.plaid.com",
            Self::Development => "https://development.plaid.com",
            Self::Production => "https://production.plaid.com",
        }
    }
}

impl FromStr for PlaidEnv {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sandbox" => Ok(Self::Sandbox),
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            other => Err(format!("unknown PLAID_ENV value: {other}")),
        }
    }
}

impl FromStr for LogFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pretty" => Ok(LogFormat::Pretty),
            "json" => Ok(LogFormat::Json),
            other => Err(format!("unknown LOG_FORMAT value: {other}")),
        }
    }
}

/// Sentry-related options.
#[derive(Debug, Clone)]
pub struct SentryConfig {
    pub dsn: Option<String>,
    pub environment: String,
    pub traces_sample_rate: f32,
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub app_host: String,
    pub app_port: u16,
    pub app_env: AppEnv,

    pub log_level: String,
    pub log_format: LogFormat,

    pub database_url: SecretString,
    pub database_max_connections: u32,

    pub supabase_url: String,
    pub supabase_jwt_audience: String,
    pub supabase_service_role_key: Option<SecretString>,
    /// Supabase publishable (anon) key. PUBLIC by design — Supabase
    /// explicitly markets this as the client-embeddable key. Exposed
    /// to the desktop via GET /v1/config/public so a single Fly
    /// secret update propagates to every installed client without
    /// rebuilding the desktop.
    pub supabase_publishable_key: Option<String>,

    pub cors_allowed_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
    /// Per-authenticated-user request cap. `None` falls back to a
    /// sensible default (60/min, burst 20) — see `AppState::new`.
    pub user_rate_limit_per_minute: Option<u32>,
    /// Burst capacity for the per-user limiter. `None` → default.
    pub user_rate_limit_burst: Option<u32>,

    pub sentry: SentryConfig,

    /// HS256 secret accepted by the auth layer in non-production builds.
    /// Always `None` when `app_env == Production`, regardless of env value.
    pub test_jwt_secret: Option<SecretString>,

    /// Plaid integration for Gold live sync. Required in production; optional
    /// in local/test so development builds can boot without real credentials.
    pub plaid: Option<PlaidConfig>,

    /// SnapTrade brokerage integration — independent of Plaid. Either,
    /// neither, or both providers can be configured per environment.
    /// `None` collapses `/v1/sync/snaptrade/*` to a `service_unavailable`
    /// response.
    pub snaptrade: Option<SnapTradeConfig>,

    /// Stripe + AI billing (Chunk 4). `None` collapses billing endpoints to a
    /// clean `not_implemented` response — useful for local dev without Stripe
    /// keys configured.
    pub billing: Option<BillingConfig>,
}

/// Billing configuration. All-or-nothing: every field must be present or the
/// whole block is treated as absent and billing endpoints disable themselves.
#[derive(Debug, Clone)]
pub struct BillingConfig {
    pub stripe_secret_key: SecretString,
    pub stripe_webhook_secret: SecretString,
    pub stripe_prices: BillingPrices,
    /// Where Stripe redirects the browser after Checkout/Portal. The desktop's
    /// auth-callback page handles this URL.
    pub return_url: String,
    /// Optional OpenAI key for the managed-AI proxy. When absent, the AI proxy
    /// endpoint rejects with `service_unavailable` even for paid users.
    pub openai_api_key: Option<SecretString>,
}

/// Stripe Price IDs sourced from env. Empty strings → "not for sale yet"
/// (handler returns `bad_request("plan/interval not available")`).
#[derive(Debug, Clone, Default)]
pub struct BillingPrices {
    pub silver_monthly: String,
    pub silver_yearly: String,
    pub gold_monthly: String,
    pub gold_yearly: String,
}

/// Plaid configuration. Access tokens are encrypted with
/// `MIZAN_PLAID_TOKEN_ENCRYPTION_KEY` before storage.
///
/// `redirect_uri` is `Option<String>` because Plaid only requires
/// it for **OAuth-required institutions**. Non-OAuth sandbox banks
/// (First Platypus, Tartan etc.) accept `/link/token/create` without
/// a `redirect_uri` field. Forcing the env var at boot crashed the
/// cloud whenever the operator deployed without setting it; the
/// optional shape lets sandbox-only postures ship without any
/// redirect URL while real OAuth banks still get one when configured.
#[derive(Debug, Clone)]
pub struct PlaidConfig {
    pub client_id: String,
    pub secret: SecretString,
    pub environment: PlaidEnv,
    pub api_base: String,
    pub redirect_uri: Option<String>,
    pub webhook_url: Option<String>,
    pub token_encryption_key: Vec<u8>,
}

/// SnapTrade API environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapTradeEnv {
    Sandbox,
    Production,
}

impl SnapTradeEnv {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Production => "production",
        }
    }
    /// Public API base. SnapTrade uses the same host for sandbox + prod;
    /// the environment is selected by the `clientId` (each client id is
    /// scoped to one env). We still record the variant for telemetry
    /// + health reporting + the desktop "you're on sandbox" badge.
    pub fn api_base(self) -> &'static str {
        "https://api.snaptrade.com"
    }
}

impl FromStr for SnapTradeEnv {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sandbox" => Ok(Self::Sandbox),
            "production" | "prod" => Ok(Self::Production),
            other => Err(format!("invalid SNAPTRADE_ENV: {other}")),
        }
    }
}

/// SnapTrade configuration. The per-user `userSecret` returned by
/// `/snapTrade/registerUser` is encrypted with the AES-256 key in
/// `MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY` before storage.
#[derive(Debug, Clone)]
pub struct SnapTradeConfig {
    pub client_id: String,
    pub consumer_key: SecretString,
    pub environment: SnapTradeEnv,
    pub api_base: String,
    /// Where SnapTrade should redirect the user after they complete the
    /// connection portal flow. Typically a deep link back into the
    /// desktop app (e.g. `mizan://snaptrade/return`).
    pub custom_redirect: Option<String>,
    pub token_encryption_key: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required env var: {0}")]
    Missing(&'static str),
    #[error("invalid value for {var}: {reason}")]
    Invalid { var: &'static str, reason: String },
    #[error("CORS allowlist may not contain '*' when auth is enabled")]
    CorsWildcardWithAuth,
    #[error("MIZAN_PLAID_TOKEN_ENCRYPTION_KEY must decode to exactly 32 bytes (got {0})")]
    BadPlaidTokenKeyLength(usize),
    #[error("MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY must decode to exactly 32 bytes (got {0})")]
    BadSnapTradeTokenKeyLength(usize),
    #[error("figment: {0}")]
    Figment(#[from] figment::Error),
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    app_host: Option<String>,
    app_port: Option<u16>,
    app_env: Option<String>,

    log_level: Option<String>,
    log_format: Option<String>,

    database_url: String,
    database_max_connections: Option<u32>,

    supabase_url: String,
    supabase_jwt_audience: Option<String>,
    supabase_service_role_key: Option<String>,
    /// Supabase anon/publishable key — PUBLIC. Optional so dev
    /// builds that don't enable Mizan Connect on desktop can omit it.
    supabase_publishable_key: Option<String>,

    mizan_cors_allowed_origins: Option<String>,
    rate_limit_per_minute: Option<u32>,
    user_rate_limit_per_minute: Option<u32>,
    user_rate_limit_burst: Option<u32>,

    sentry_dsn: Option<String>,
    sentry_environment: Option<String>,
    sentry_traces_sample_rate: Option<f32>,

    mizan_test_jwt_secret: Option<String>,

    // Plaid Gold sync
    plaid_client_id: Option<String>,
    plaid_secret: Option<String>,
    plaid_env: Option<String>,
    plaid_api_base: Option<String>,
    plaid_redirect_uri: Option<String>,
    plaid_webhook_url: Option<String>,
    mizan_plaid_token_encryption_key: Option<String>,

    // SnapTrade — Gold brokerage sync (second live boundary after Plaid)
    snaptrade_client_id: Option<String>,
    snaptrade_consumer_key: Option<String>,
    snaptrade_env: Option<String>,
    snaptrade_api_base: Option<String>,
    /// Where SnapTrade should redirect the user after the connection
    /// portal flow completes. Conventionally a deep link the desktop
    /// app handles (e.g. `mizan://snaptrade/return`). The env var is
    /// `SNAPTRADE_REDIRECT_URI` to match the SnapTrade docs naming.
    snaptrade_redirect_uri: Option<String>,
    mizan_snaptrade_token_encryption_key: Option<String>,
    /// HS256 secret used to sign the state nonce SnapTrade echoes back
    /// on the OAuth-style return so we can defend against replay. Not
    /// strictly required for the read-only flow but required when the
    /// callback handler is wired (future slice). Tracked here so the
    /// existing `MIZAN_SNAPTRADE_STATE_SECRET` Fly secret doesn't
    /// surface as "unused-but-set" in the audit log.
    #[serde(default)]
    #[allow(dead_code)]
    mizan_snaptrade_state_secret: Option<String>,

    // Stripe billing (Chunk 4)
    stripe_secret_key: Option<String>,
    stripe_webhook_secret: Option<String>,
    stripe_price_silver_monthly: Option<String>,
    stripe_price_silver_yearly: Option<String>,
    stripe_price_gold_monthly: Option<String>,
    stripe_price_gold_yearly: Option<String>,
    stripe_price_basic_monthly: Option<String>,
    stripe_price_basic_yearly: Option<String>,
    stripe_price_pro_monthly: Option<String>,
    stripe_price_pro_yearly: Option<String>,
    stripe_price_enterprise_monthly: Option<String>,
    stripe_price_enterprise_yearly: Option<String>,
    mizan_billing_return_url: Option<String>,

    // Managed AI proxy (Chunk 4)
    openai_api_key: Option<String>,
}

impl Config {
    /// Load and validate configuration from environment variables.
    ///
    /// Reads `.env` first (no override of already-set env), then merges
    /// process environment. Returns [`ConfigError`] on any validation failure
    /// so the caller can surface a clean startup error.
    pub fn load() -> Result<Self, ConfigError> {
        // Best-effort .env load; missing file is fine.
        let _ = dotenvy::dotenv();

        let raw: RawConfig = Figment::new().merge(Env::raw().lowercase(true)).extract()?;

        let app_env = parse_required("APP_ENV", raw.app_env.as_deref(), Some("development"))?;

        let log_format = parse_optional::<LogFormat>(
            "LOG_FORMAT",
            raw.log_format.as_deref(),
            match app_env {
                AppEnv::Production | AppEnv::Staging => LogFormat::Json,
                _ => LogFormat::Pretty,
            },
        )?;

        let database_url = trim_required(&raw.database_url, "DATABASE_URL")?;
        let supabase_url = trim_required(&raw.supabase_url, "SUPABASE_URL")?;

        if !supabase_url.starts_with("https://") && !app_env.is_test() {
            return Err(ConfigError::Invalid {
                var: "SUPABASE_URL",
                reason: "must use https:// scheme outside of test env".to_string(),
            });
        }

        let cors_allowed_origins = parse_cors(
            raw.mizan_cors_allowed_origins.as_deref(),
            // In Chunk 1 every endpoint behind /v1 requires auth. Wildcard
            // CORS is therefore always disallowed in non-test envs.
            !app_env.is_test(),
        )?;

        let plaid = build_plaid_config(&raw, app_env)?;
        let snaptrade = build_snaptrade_config(&raw)?;
        let billing = build_billing_config(&raw);

        let test_jwt_secret = if app_env.is_production() {
            None
        } else {
            raw.mizan_test_jwt_secret
                .filter(|s| !s.trim().is_empty())
                .map(SecretString::from)
        };

        let service_role_key = raw
            .supabase_service_role_key
            .filter(|s| !s.trim().is_empty())
            .map(SecretString::from);

        let sentry = SentryConfig {
            dsn: raw.sentry_dsn.filter(|s| !s.trim().is_empty()),
            environment: raw.sentry_environment.unwrap_or_else(|| match app_env {
                AppEnv::Production => "production".into(),
                AppEnv::Staging => "staging".into(),
                AppEnv::Development => "development".into(),
                AppEnv::Test => "test".into(),
            }),
            traces_sample_rate: raw.sentry_traces_sample_rate.unwrap_or(0.1).clamp(0.0, 1.0),
        };

        Ok(Self {
            app_host: raw.app_host.unwrap_or_else(|| "0.0.0.0".into()),
            app_port: raw.app_port.unwrap_or(8080),
            app_env,
            log_level: raw.log_level.unwrap_or_else(|| "info".into()),
            log_format,
            database_url: SecretString::from(database_url),
            database_max_connections: raw.database_max_connections.unwrap_or(10).max(1),
            supabase_url,
            supabase_jwt_audience: raw
                .supabase_jwt_audience
                .unwrap_or_else(|| "authenticated".into()),
            supabase_service_role_key: service_role_key,
            // PUBLIC anon key, optionally set so /v1/config/public can
            // hand it back to desktop clients. None → desktop falls back
            // to its build-time CONNECT_AUTH_PUBLISHABLE_KEY (which may
            // also be unset → "offline build" message).
            supabase_publishable_key: raw
                .supabase_publishable_key
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            cors_allowed_origins,
            rate_limit_per_minute: raw.rate_limit_per_minute.unwrap_or(100).max(1),
            user_rate_limit_per_minute: raw.user_rate_limit_per_minute,
            user_rate_limit_burst: raw.user_rate_limit_burst,
            sentry,
            test_jwt_secret,
            plaid,
            snaptrade,
            billing,
        })
    }

    /// JWKS URL derived from `SUPABASE_URL`.
    pub fn jwks_url(&self) -> String {
        format!(
            "{}/auth/v1/.well-known/jwks.json",
            self.supabase_url.trim_end_matches('/')
        )
    }

    /// Expected JWT issuer (`iss`) claim.
    pub fn supabase_jwt_issuer(&self) -> String {
        format!("{}/auth/v1", self.supabase_url.trim_end_matches('/'))
    }
}

fn build_plaid_config(
    raw: &RawConfig,
    app_env: AppEnv,
) -> Result<Option<PlaidConfig>, ConfigError> {
    use base64::Engine;

    let any_configured = [
        raw.plaid_client_id.as_deref(),
        raw.plaid_secret.as_deref(),
        raw.plaid_env.as_deref(),
        raw.plaid_redirect_uri.as_deref(),
        raw.plaid_webhook_url.as_deref(),
        raw.mizan_plaid_token_encryption_key.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|s| !s.trim().is_empty());

    // Plaid is required when:
    //   - PLAID_ENV is explicitly set to `production`, OR
    //   - any Plaid var is set (operator opted in — finish the wiring).
    // Otherwise the app boots without Plaid and /sync/plaid/* returns 503.
    // This lets us ship to Fly in sandbox-only or pre-Plaid postures without
    // forcing real credentials at boot.
    let plaid_env_is_production = raw
        .plaid_env
        .as_deref()
        .map(str::trim)
        .map(|s| s.eq_ignore_ascii_case("production"))
        .unwrap_or(false);
    let required = plaid_env_is_production || any_configured;
    if !required {
        return Ok(None);
    }
    let _ = app_env;

    let environment =
        parse_required::<PlaidEnv>("PLAID_ENV", raw.plaid_env.as_deref(), Some("sandbox"))?;
    let api_base = raw
        .plaid_api_base
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| environment.api_base().to_string());

    let client_id = pick_required("PLAID_CLIENT_ID", raw.plaid_client_id.as_deref(), required)?
        .unwrap_or_default();
    let secret =
        pick_required("PLAID_SECRET", raw.plaid_secret.as_deref(), required)?.unwrap_or_default();
    // PLAID_REDIRECT_URI is OPTIONAL even when Plaid as a whole is
    // required — see PlaidConfig docs above. Same pattern as
    // `webhook_url`: empty / unset → None, anything trimmed-non-empty
    // → Some(uri). When None the client omits the `redirect_uri`
    // field from /link/token/create, which sandbox non-OAuth banks
    // accept.
    let redirect_uri = raw
        .plaid_redirect_uri
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let enc_key_b64 = pick_required(
        "MIZAN_PLAID_TOKEN_ENCRYPTION_KEY",
        raw.mizan_plaid_token_encryption_key.as_deref(),
        required,
    )?;
    let token_encryption_key = match enc_key_b64 {
        Some(s) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .map_err(|e| ConfigError::Invalid {
                    var: "MIZAN_PLAID_TOKEN_ENCRYPTION_KEY",
                    reason: format!("base64 decode: {e}"),
                })?;
            if bytes.len() != 32 {
                return Err(ConfigError::BadPlaidTokenKeyLength(bytes.len()));
            }
            bytes
        }
        None => Vec::new(),
    };

    Ok(Some(PlaidConfig {
        client_id,
        secret: SecretString::from(secret),
        environment,
        api_base,
        redirect_uri,
        webhook_url: raw
            .plaid_webhook_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        token_encryption_key,
    }))
}

/// Build the SnapTrade block. Mirrors `build_plaid_config` exactly: any
/// non-empty SnapTrade env var opts the operator in and we then require
/// the full set. Empty across the board → `Ok(None)` and the SnapTrade
/// router returns 503 from the health endpoint, every other endpoint
/// returns `service_unavailable`.
fn build_snaptrade_config(raw: &RawConfig) -> Result<Option<SnapTradeConfig>, ConfigError> {
    use base64::Engine;

    let any_configured = [
        raw.snaptrade_client_id.as_deref(),
        raw.snaptrade_consumer_key.as_deref(),
        raw.snaptrade_env.as_deref(),
        raw.snaptrade_api_base.as_deref(),
        raw.snaptrade_redirect_uri.as_deref(),
        raw.mizan_snaptrade_token_encryption_key.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|s| !s.trim().is_empty());

    let production_explicit = raw
        .snaptrade_env
        .as_deref()
        .map(str::trim)
        .map(|s| s.eq_ignore_ascii_case("production"))
        .unwrap_or(false);

    let required = production_explicit || any_configured;
    if !required {
        return Ok(None);
    }

    let environment = parse_required::<SnapTradeEnv>(
        "SNAPTRADE_ENV",
        raw.snaptrade_env.as_deref(),
        Some("sandbox"),
    )?;

    let api_base = raw
        .snaptrade_api_base
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| environment.api_base().to_string());

    let client_id =
        pick_required("SNAPTRADE_CLIENT_ID", raw.snaptrade_client_id.as_deref(), true)?
            .unwrap_or_default();
    let consumer_key = pick_required(
        "SNAPTRADE_CONSUMER_KEY",
        raw.snaptrade_consumer_key.as_deref(),
        true,
    )?
    .unwrap_or_default();

    let enc_key_b64 = pick_required(
        "MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY",
        raw.mizan_snaptrade_token_encryption_key.as_deref(),
        true,
    )?;
    let token_encryption_key = match enc_key_b64 {
        Some(s) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .map_err(|e| ConfigError::Invalid {
                    var: "MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY",
                    reason: format!("base64 decode: {e}"),
                })?;
            if bytes.len() != 32 {
                return Err(ConfigError::BadSnapTradeTokenKeyLength(bytes.len()));
            }
            bytes
        }
        None => Vec::new(),
    };

    Ok(Some(SnapTradeConfig {
        client_id,
        consumer_key: SecretString::from(consumer_key),
        environment,
        api_base,
        custom_redirect: raw
            .snaptrade_redirect_uri
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        token_encryption_key,
    }))
}

/// `pick_required(var, raw, required)`:
/// - if `raw` is non-empty → `Ok(Some(value))`
/// - if `raw` is empty AND `required` → `Err(Missing)`
/// - otherwise → `Ok(None)`
fn pick_required(
    var: &'static str,
    raw: Option<&str>,
    required: bool,
) -> Result<Option<String>, ConfigError> {
    let value = raw.map(str::trim).filter(|s| !s.is_empty());
    match value {
        Some(v) => Ok(Some(v.to_string())),
        None if required => Err(ConfigError::Missing(var)),
        None => Ok(None),
    }
}

fn trim_required(value: &str, var: &'static str) -> Result<String, ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::Missing(var));
    }
    Ok(trimmed.to_string())
}

fn parse_required<T: FromStr<Err = String>>(
    var: &'static str,
    raw: Option<&str>,
    default: Option<&str>,
) -> Result<T, ConfigError> {
    let value = match raw.filter(|s| !s.trim().is_empty()) {
        Some(v) => v,
        None => default.ok_or(ConfigError::Missing(var))?,
    };
    T::from_str(value).map_err(|reason| ConfigError::Invalid { var, reason })
}

fn parse_optional<T: FromStr<Err = String>>(
    var: &'static str,
    raw: Option<&str>,
    default: T,
) -> Result<T, ConfigError> {
    match raw.filter(|s| !s.trim().is_empty()) {
        Some(v) => T::from_str(v).map_err(|reason| ConfigError::Invalid { var, reason }),
        None => Ok(default),
    }
}

/// Build the [`BillingConfig`] from env, or `None` when the operator hasn't
/// configured Stripe yet. All-or-nothing: if either `STRIPE_SECRET_KEY` or
/// `STRIPE_WEBHOOK_SECRET` is missing, billing is disabled entirely so a
/// half-configured server can't serve "200 OK" on broken checkout flows.
fn build_billing_config(raw: &RawConfig) -> Option<BillingConfig> {
    let secret = raw
        .stripe_secret_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let webhook = raw
        .stripe_webhook_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let return_url = raw
        .mizan_billing_return_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("https://mizan.app/billing/return")
        .to_string();

    let prices = BillingPrices {
        silver_monthly: raw
            .stripe_price_silver_monthly
            .clone()
            .or_else(|| raw.stripe_price_basic_monthly.clone())
            .unwrap_or_default(),
        silver_yearly: raw
            .stripe_price_silver_yearly
            .clone()
            .or_else(|| raw.stripe_price_basic_yearly.clone())
            .unwrap_or_default(),
        gold_monthly: raw
            .stripe_price_gold_monthly
            .clone()
            .or_else(|| raw.stripe_price_pro_monthly.clone())
            .or_else(|| raw.stripe_price_enterprise_monthly.clone())
            .unwrap_or_default(),
        gold_yearly: raw
            .stripe_price_gold_yearly
            .clone()
            .or_else(|| raw.stripe_price_pro_yearly.clone())
            .or_else(|| raw.stripe_price_enterprise_yearly.clone())
            .unwrap_or_default(),
    };

    let openai_api_key = raw
        .openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| SecretString::from(s.to_string()));

    Some(BillingConfig {
        stripe_secret_key: SecretString::from(secret.to_string()),
        stripe_webhook_secret: SecretString::from(webhook.to_string()),
        stripe_prices: prices,
        return_url,
        openai_api_key,
    })
}

fn parse_cors(raw: Option<&str>, reject_wildcard: bool) -> Result<Vec<String>, ConfigError> {
    let raw = raw.unwrap_or("");
    let origins: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if reject_wildcard && origins.iter().any(|o| o == "*") {
        return Err(ConfigError::CorsWildcardWithAuth);
    }

    Ok(origins)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn cors_rejects_wildcard_when_strict() {
        let err = parse_cors(Some("*,https://example.com"), true).unwrap_err();
        assert!(matches!(err, ConfigError::CorsWildcardWithAuth));
    }

    #[test]
    fn cors_allows_wildcard_in_test_env() {
        let origins = parse_cors(Some("*"), false).expect("wildcard allowed in test");
        assert_eq!(origins, vec!["*".to_string()]);
    }

    #[test]
    fn cors_parses_csv() {
        let origins =
            parse_cors(Some("https://a.test, https://b.test , "), true).expect("valid origins");
        assert_eq!(
            origins,
            vec!["https://a.test".to_string(), "https://b.test".to_string()]
        );
    }

    #[test]
    fn app_env_parses_known() {
        assert_eq!(AppEnv::from_str("production"), Ok(AppEnv::Production));
        assert_eq!(AppEnv::from_str("DEV"), Ok(AppEnv::Development));
        assert!(AppEnv::from_str("nope").is_err());
    }
}
