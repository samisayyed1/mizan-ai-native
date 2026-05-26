//! Silver/Gold capability matrix.
//!
//! Single source of truth on the cloud side; **must stay in lock-step with the
//! desktop's `crates/connect/src/entitlements.rs` table**. The /v1/me response
//! ships this matrix verbatim so the desktop's `Entitlements` struct deserializes
//! it directly.

use serde::{Deserialize, Serialize};

/// Sentinel meaning "no limit" for any numeric quota.
pub const UNLIMITED: i32 = -1;

/// Capabilities + quotas a subscription unlocks.
///
/// Field names + JSON shape mirror the desktop's `Entitlements` struct
/// at `mizan-4/crates/connect/src/entitlements.rs`. The desktop ships
/// the canonical truth table; this cloud copy MUST stay in lock-step
/// (lint-checked via the cross-repo entitlement-parity test in
/// `crates/connect/tests/cloud_contract.rs` once that exists).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlements {
    /// Resolved plan slug this matrix is for (`free`/`silver`/`gold`).
    pub plan: String,
    pub max_portfolios: i32,
    pub max_holdings: i32,
    pub max_asset_classes: i32,
    pub broker_sync: bool,
    pub max_broker_connections: i32,
    pub device_sync: bool,
    pub cloud_backup: bool,
    pub managed_ai: bool,
    pub ai_credits_monthly: i32,
    pub news_daily_limit: i32,
    pub market_refresh_daily_limit: i32,
    pub csv_imports_monthly: i32,
    pub advanced_reports: bool,
    /// Zakat & purification assessment engine. Gold-only (per Feroz 25
    /// May 2026). Added in lock-step with the desktop's Entitlements —
    /// previous absence here caused the desktop to fail parsing /v1/me
    /// with `missing field zakatEngine` for every user.
    pub zakat_engine: bool,
    pub advisor_mode: bool,
}

impl Default for Entitlements {
    /// **TRUE FREE** — what every signed-in user without an active paid
    /// subscription gets. Local-first, BYO-AI key. 1 portfolio cap, no
    /// CSV import, no Plaid, no Zakat engine, no managed AI.
    ///
    /// (The pre-2026-05 cloud default was actually the Silver matrix
    /// with `plan: "free"` — a copy-paste regression. Synced now to
    /// the desktop's canonical Free tier.)
    fn default() -> Self {
        Self {
            plan: "free".to_string(),
            max_portfolios: 1,
            max_holdings: 20,
            max_asset_classes: UNLIMITED,
            broker_sync: false,
            max_broker_connections: 0,
            device_sync: false,
            cloud_backup: false,
            managed_ai: false,
            ai_credits_monthly: 0,
            news_daily_limit: 3,
            market_refresh_daily_limit: 0,
            csv_imports_monthly: 0,
            advanced_reports: false,
            zakat_engine: false,
            advisor_mode: false,
        }
    }
}

/// Derive entitlements from a stored subscription's `tier` slug + `status`.
///
/// Anything other than `active`/`trialing` collapses to Silver-capability UX.
/// Legacy `free`/`basic` rows map to Silver; legacy `pro`/`plus`/`enterprise`
/// rows map to Gold so existing paid users retain live sync.
pub fn entitlements_for(tier: Option<&str>, status: Option<&str>) -> Entitlements {
    let active = matches!(status, Some("active") | Some("trialing"));
    if !active {
        return Entitlements::default();
    }
    match tier.map(|t| t.to_ascii_lowercase()).as_deref() {
        None | Some("free") => Entitlements::default(),

        Some("silver") | Some("basic") => Entitlements {
            plan: "silver".to_string(),
            max_portfolios: 25,
            max_holdings: UNLIMITED,
            max_asset_classes: UNLIMITED,
            broker_sync: false,
            max_broker_connections: 0,
            device_sync: false,
            cloud_backup: false,
            managed_ai: true,
            ai_credits_monthly: 300,
            news_daily_limit: UNLIMITED,
            market_refresh_daily_limit: 0,
            csv_imports_monthly: UNLIMITED,
            advanced_reports: false,
            zakat_engine: false,
            advisor_mode: false,
        },

        Some("gold") | Some("enterprise") | Some("advisor") => Entitlements {
            plan: "gold".to_string(),
            max_portfolios: UNLIMITED,
            max_holdings: UNLIMITED,
            max_asset_classes: UNLIMITED,
            broker_sync: true,
            max_broker_connections: UNLIMITED,
            device_sync: true,
            cloud_backup: true,
            managed_ai: true,
            ai_credits_monthly: UNLIMITED,
            news_daily_limit: UNLIMITED,
            market_refresh_daily_limit: UNLIMITED,
            csv_imports_monthly: UNLIMITED,
            advanced_reports: true,
            zakat_engine: true,
            advisor_mode: matches!(tier, Some("enterprise") | Some("advisor")),
        },

        // pro / essentials / duo / plus / any other active paid slug → Gold matrix.
        Some(_) => Entitlements {
            plan: "gold".to_string(),
            max_portfolios: 25,
            max_holdings: UNLIMITED,
            max_asset_classes: UNLIMITED,
            broker_sync: true,
            max_broker_connections: 5,
            device_sync: true,
            cloud_backup: true,
            managed_ai: true,
            ai_credits_monthly: 1500,
            news_daily_limit: UNLIMITED,
            market_refresh_daily_limit: UNLIMITED,
            csv_imports_monthly: UNLIMITED,
            advanced_reports: true,
            zakat_engine: true,
            advisor_mode: false,
        },
    }
}

/// AI credit balance + reset window surfaced to clients on /v1/me.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCredits {
    pub monthly: i32,
    pub used: i32,
    pub resets_at: Option<time::OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_falls_back_to_free_capabilities() {
        // Previously asserted "silver-capability UX" but the function
        // actually returns Default — which is now true-Free (1
        // portfolio, no managed AI). The fallback is the unpaid
        // experience, not Silver.
        assert_eq!(
            entitlements_for(Some("pro"), Some("canceled")),
            Entitlements::default()
        );
        assert_eq!(entitlements_for(Some("pro"), None), Entitlements::default());
        assert_eq!(entitlements_for(None, None), Entitlements::default());
    }

    #[test]
    fn default_is_true_free_not_silver() {
        // Regression: pre-2026-05 the default was silently the Silver
        // matrix with plan: "free", so signed-out users got 25
        // portfolios + 300 AI credits + unlimited CSV import. Lock the
        // canonical Free matrix in place.
        let e = Entitlements::default();
        assert_eq!(e.plan, "free");
        assert_eq!(e.max_portfolios, 1);
        assert_eq!(e.max_holdings, 20);
        assert!(!e.managed_ai);
        assert_eq!(e.ai_credits_monthly, 0);
        assert_eq!(e.csv_imports_monthly, 0);
        assert!(!e.zakat_engine);
        assert!(!e.broker_sync);
        assert_eq!(e.news_daily_limit, 3);
    }

    #[test]
    fn zakat_engine_field_serialises_camel_case() {
        // Regression: missing zakatEngine on the wire was breaking
        // every desktop's /v1/me parse with `missing field zakatEngine`.
        // Lock the JSON shape in place.
        let gold = entitlements_for(Some("gold"), Some("active"));
        let v = serde_json::to_value(&gold).unwrap();
        assert_eq!(v.get("zakatEngine").and_then(|v| v.as_bool()), Some(true));

        let silver = entitlements_for(Some("silver"), Some("active"));
        let v = serde_json::to_value(&silver).unwrap();
        assert_eq!(v.get("zakatEngine").and_then(|v| v.as_bool()), Some(false));

        let free = Entitlements::default();
        let v = serde_json::to_value(&free).unwrap();
        assert_eq!(v.get("zakatEngine").and_then(|v| v.as_bool()), Some(false));
    }

    #[test]
    fn basic_legacy_maps_to_silver() {
        let e = entitlements_for(Some("basic"), Some("active"));
        assert!(!e.broker_sync);
        assert_eq!(e.plan, "silver");
        assert!(e.managed_ai);
        assert_eq!(e.csv_imports_monthly, UNLIMITED);
    }

    #[test]
    fn gold_and_pro_legacy_slugs_unlock_plaid_sync() {
        for slug in ["pro", "plus", "duo", "essentials"] {
            let e = entitlements_for(Some(slug), Some("active"));
            assert!(e.broker_sync, "{slug} should include broker sync");
            assert_eq!(e.plan, "gold");
            assert_eq!(e.ai_credits_monthly, 1500);
        }
        assert!(entitlements_for(Some("gold"), Some("active")).broker_sync);
    }

    #[test]
    fn enterprise_unlocks_advisor() {
        let e = entitlements_for(Some("enterprise"), Some("active"));
        assert!(e.advisor_mode);
        assert_eq!(e.max_portfolios, UNLIMITED);
    }

    #[test]
    fn trialing_counts_as_active() {
        assert!(entitlements_for(Some("pro"), Some("trialing")).broker_sync);
    }
}
