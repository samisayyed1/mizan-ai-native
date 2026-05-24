//! Subscription entitlements — the single source of truth for what a user's
//! plan unlocks.
//!
//! This is the **client-side stopgap** half of the entitlements system. The
//! Mizan Connect cloud (`/api/v1/user/me`) will eventually return an explicit
//! `entitlements` object; until then we derive the matrix from the team's
//! `plan` slug + `subscription_status` via [`entitlements_for_plan`]. When the
//! cloud starts returning entitlements directly, the consuming service prefers
//! that and this mapping becomes the fallback for older backends.
//!
//! `-1` is the sentinel for "unlimited" on every numeric quota.

use serde::{Deserialize, Serialize};

/// Sentinel meaning "no limit" for any numeric quota field.
pub const UNLIMITED: i32 = -1;

/// What a user's subscription unlocks. Mirrors the product tier table; gated
/// both in the Rust IPC layer (tamper-resistant) and the frontend (UX).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlements {
    /// Resolved plan slug this matrix was derived from (`free`/`basic`/…),
    /// surfaced to the UI as the "current plan" in upgrade prompts.
    pub plan: String,
    /// Max portfolios (accounts). `UNLIMITED` for no cap.
    pub max_portfolios: i32,
    /// Max holdings per portfolio. `UNLIMITED` for no cap.
    pub max_holdings: i32,
    /// Max distinct asset classes a portfolio may use. `UNLIMITED` for all.
    pub max_asset_classes: i32,
    /// Plaid live sync allowed at all.
    pub broker_sync: bool,
    /// Max simultaneous broker connections.
    pub max_broker_connections: i32,
    /// Encrypted multi-device sync.
    pub device_sync: bool,
    /// Encrypted cloud backup.
    pub cloud_backup: bool,
    /// Managed Mizan AI (vs. bring-your-own-key, which is always allowed).
    pub managed_ai: bool,
    /// Monthly managed-AI credit allowance (0 = none / BYO-key only).
    pub ai_credits_monthly: i32,
    /// News headlines per day. `UNLIMITED` for the full feed.
    pub news_daily_limit: i32,
    /// Market-data refreshes per day. `UNLIMITED` for no cap.
    pub market_refresh_daily_limit: i32,
    /// CSV imports per month. `UNLIMITED` for no cap.
    pub csv_imports_monthly: i32,
    /// Advanced/deep reports (income, rental, payoff, health score).
    pub advanced_reports: bool,
    /// Advisor/enterprise client dashboards & white-label.
    pub advisor_mode: bool,
}

impl Default for Entitlements {
    /// Silver-capability fallback. Anyone without an active paid subscription
    /// still gets the private local-first product.
    fn default() -> Self {
        Self {
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
            news_daily_limit: 3,
            market_refresh_daily_limit: 0,
            csv_imports_monthly: UNLIMITED,
            advanced_reports: false,
            advisor_mode: false,
        }
    }
}

impl Entitlements {
    /// True when `n` is below the quota `limit` (`UNLIMITED` always passes).
    /// Use for "may I add one more?" checks: `entitlements.within(count, limit)`.
    pub fn within(current: i32, limit: i32) -> bool {
        limit == UNLIMITED || current < limit
    }
}

/// Derive entitlements from a team's `plan` slug + `subscription_status`.
///
/// Inactive/absent subscriptions collapse to Silver. Legacy `free`/`basic`
/// slugs map to Silver; legacy paid slugs map to Gold.
pub fn entitlements_for_plan(plan: Option<&str>, status: Option<&str>) -> Entitlements {
    let is_active = matches!(status, Some("active") | Some("trialing"));
    if !is_active {
        return Entitlements::default();
    }

    match plan.map(str::to_ascii_lowercase).as_deref() {
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
            advisor_mode: matches!(plan, Some("enterprise") | Some("advisor")),
        },

        // Any other active paid slug (pro / essentials / duo / plus / …) maps to Gold.
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
            advisor_mode: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_subscription_falls_back_to_silver() {
        assert_eq!(
            entitlements_for_plan(Some("pro"), Some("canceled")),
            Entitlements::default()
        );
        assert_eq!(
            entitlements_for_plan(Some("pro"), None),
            Entitlements::default()
        );
        assert_eq!(entitlements_for_plan(None, None), Entitlements::default());
    }

    #[test]
    fn silver_defaults() {
        let e = Entitlements::default();
        assert_eq!(e.plan, "silver");
        assert_eq!(e.max_portfolios, 25);
        assert_eq!(e.max_asset_classes, UNLIMITED);
        assert!(!e.broker_sync);
        assert!(e.managed_ai);
        assert_eq!(e.ai_credits_monthly, 300);
    }

    #[test]
    fn basic_legacy_maps_to_silver_no_plaid() {
        let e = entitlements_for_plan(Some("basic"), Some("active"));
        assert_eq!(e.plan, "silver");
        assert!(!e.broker_sync, "Silver excludes Plaid live sync");
        assert!(e.managed_ai);
        assert_eq!(e.max_portfolios, 25);
        assert_eq!(e.ai_credits_monthly, 300);
    }

    #[test]
    fn pro_like_plans_unlock_broker_sync() {
        for slug in ["pro", "plus", "duo", "essentials", "anything-paid"] {
            let e = entitlements_for_plan(Some(slug), Some("active"));
            assert!(e.broker_sync, "{slug} should include Plaid sync");
            assert_eq!(e.plan, "gold");
            assert!(e.max_broker_connections >= 1);
            assert_eq!(e.max_holdings, UNLIMITED);
        }
    }

    #[test]
    fn trialing_counts_as_active() {
        let e = entitlements_for_plan(Some("pro"), Some("trialing"));
        assert!(e.broker_sync);
    }

    #[test]
    fn enterprise_unlocks_advisor_mode() {
        let e = entitlements_for_plan(Some("enterprise"), Some("active"));
        assert!(e.advisor_mode);
        assert_eq!(e.max_portfolios, UNLIMITED);
        assert_eq!(e.ai_credits_monthly, UNLIMITED);
    }

    #[test]
    fn within_respects_unlimited_and_caps() {
        assert!(Entitlements::within(0, 1)); // first one allowed
        assert!(!Entitlements::within(1, 1)); // at cap, blocked
        assert!(Entitlements::within(9999, UNLIMITED)); // unlimited always ok
    }
}
