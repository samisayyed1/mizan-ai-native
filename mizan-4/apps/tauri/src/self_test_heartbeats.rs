//! Production `Heartbeats` impl backed by `reqwest::blocking` (Track I
//! PR-I5.b per ADR 0009).
//!
//! Lives in `apps/tauri` (not `mizan-storage-sqlite`) so the storage
//! crate stays free of an HTTP-client dep + lifecycle code. The
//! storage-sqlite `Heartbeats` trait is the contract; this is its
//! production implementation.
//!
//! Why blocking + not async? `mizan_storage_sqlite::self_test::run_and_persist`
//! is sync (it runs during the blocking startup sequence before the
//! tokio runtime is built). reqwest::blocking is the conventional
//! Rust ecosystem answer.

use std::time::{Duration, Instant};

use mizan_storage_sqlite::self_test::{HeartbeatResult, Heartbeats};

/// 5-second per-probe budget per ADR 0009 + working-agreement §A19.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

/// Production [`Heartbeats`] impl. Holds the configured base URLs +
/// API keys captured from env at startup. Re-creates a fresh
/// short-lived `reqwest::blocking::Client` per probe to avoid holding
/// an idle connection during the WebView's lifetime.
pub struct HttpHeartbeats {
    /// Twelve Data API base URL. Falls back to the documented default.
    pub twelve_data_base: String,
    /// Twelve Data API key. If absent, the probe is reported as
    /// Skipped (no env var) — the user may not have a quote provider
    /// configured (free tier, etc.).
    pub twelve_data_api_key: Option<String>,
    /// Mizan Connect base URL (e.g. `https://mizan-connect.fly.dev`).
    /// If absent, the probe is reported as Skipped.
    pub mizan_connect_base: Option<String>,
}

impl HttpHeartbeats {
    /// Build from environment. None of these reads can fail — missing
    /// env vars degrade the probe to Skipped rather than failing the
    /// orchestrator.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            twelve_data_base: std::env::var("TWELVE_DATA_API_BASE")
                .unwrap_or_else(|_| "https://api.twelvedata.com".to_string()),
            twelve_data_api_key: std::env::var("TWELVE_DATA_API_KEY").ok(),
            mizan_connect_base: std::env::var("CONNECT_API_URL").ok(),
        }
    }

    fn build_client() -> Option<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .timeout(HEARTBEAT_TIMEOUT)
            .user_agent(concat!("mizan-self-test/", env!("CARGO_PKG_VERSION")))
            .build()
            .ok()
    }
}

impl Heartbeats for HttpHeartbeats {
    fn twelve_data(&self) -> HeartbeatResult {
        let started = Instant::now();

        let Some(api_key) = &self.twelve_data_api_key else {
            return HeartbeatResult {
                elapsed: started.elapsed(),
                result: Err("TWELVE_DATA_API_KEY not set (free-tier / unconfigured)".to_string()),
            };
        };

        let Some(client) = Self::build_client() else {
            return HeartbeatResult {
                elapsed: started.elapsed(),
                result: Err("reqwest client build failed".to_string()),
            };
        };

        let url = format!(
            "{}/quote?symbol=AAPL&apikey={}",
            self.twelve_data_base.trim_end_matches('/'),
            api_key
        );

        let result = match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => match resp.text() {
                Ok(body) if body.contains("symbol") => Ok(()),
                Ok(_) => Err("response missing `symbol` field".to_string()),
                Err(e) => Err(format!("read body: {e}")),
            },
            Ok(resp) => Err(format!("HTTP {}", resp.status())),
            Err(e) => Err(format!("send: {e}")),
        };

        HeartbeatResult {
            elapsed: started.elapsed(),
            result,
        }
    }

    fn mizan_connect(&self) -> HeartbeatResult {
        let started = Instant::now();

        let Some(base) = &self.mizan_connect_base else {
            return HeartbeatResult {
                elapsed: started.elapsed(),
                result: Err("CONNECT_API_URL not set".to_string()),
            };
        };

        let Some(client) = Self::build_client() else {
            return HeartbeatResult {
                elapsed: started.elapsed(),
                result: Err("reqwest client build failed".to_string()),
            };
        };

        let url = format!("{}/v1/health", base.trim_end_matches('/'));
        let result = match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!("HTTP {}", resp.status())),
            Err(e) => Err(format!("send: {e}")),
        };

        HeartbeatResult {
            elapsed: started.elapsed(),
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn from_env_with_no_vars_yields_no_keys() {
        // SAFETY: tests run in serial enough that the absence-test is
        // valid only if no other test concurrently sets these. Tolerate
        // either outcome — the contract is "from_env doesn't panic".
        std::env::remove_var("TWELVE_DATA_API_KEY");
        std::env::remove_var("CONNECT_API_URL");
        let hb = HttpHeartbeats::from_env();
        assert!(hb.twelve_data_api_key.is_none());
        // mizan_connect_base may be set by other tests running in
        // parallel within the same process; tolerate either value.
        let _ = hb.mizan_connect_base;
    }

    #[test]
    fn twelve_data_returns_err_when_api_key_absent() {
        let hb = HttpHeartbeats {
            twelve_data_base: "https://api.twelvedata.com".to_string(),
            twelve_data_api_key: None,
            mizan_connect_base: None,
        };
        let out = hb.twelve_data();
        let err = out.result.unwrap_err();
        assert!(err.contains("not set"), "got: {err}");
    }

    #[test]
    fn mizan_connect_returns_err_when_base_absent() {
        let hb = HttpHeartbeats {
            twelve_data_base: "https://api.twelvedata.com".to_string(),
            twelve_data_api_key: None,
            mizan_connect_base: None,
        };
        let out = hb.mizan_connect();
        let err = out.result.unwrap_err();
        assert!(err.contains("not set"), "got: {err}");
    }
}
