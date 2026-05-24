//! Stripe Price ID lookup, sourced from env.
//!
//! Four product/interval combinations:
//!   - silver / monthly + yearly
//!   - gold / monthly + yearly
//!
//! Each is configured separately so the operator can swap prices without a
//! redeploy. Missing env vars surface at config load time, not at first user
//! checkout.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Subscription plans the client can ask to subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutPlan {
    Silver,
    Gold,
}

impl FromStr for CheckoutPlan {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "silver" | "basic" => Ok(Self::Silver),
            "gold" | "pro" | "enterprise" => Ok(Self::Gold),
            other => Err(format!("unknown plan: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BillingInterval {
    Monthly,
    Yearly,
}

impl FromStr for BillingInterval {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "monthly" | "month" => Ok(Self::Monthly),
            "yearly" | "year" | "annual" => Ok(Self::Yearly),
            other => Err(format!("unknown interval: {other}")),
        }
    }
}

/// Stripe Price ID configuration (two plans × two intervals).
#[derive(Debug, Clone)]
pub struct StripePrices {
    pub silver_monthly: String,
    pub silver_yearly: String,
    pub gold_monthly: String,
    pub gold_yearly: String,
}

impl StripePrices {
    /// Resolve to a single Price ID. Returns `None` only if the operator
    /// shipped an empty string for that slot — treated as "not for sale yet".
    pub fn lookup(&self, plan: CheckoutPlan, interval: BillingInterval) -> Option<&str> {
        let raw = match (plan, interval) {
            (CheckoutPlan::Silver, BillingInterval::Monthly) => &self.silver_monthly,
            (CheckoutPlan::Silver, BillingInterval::Yearly) => &self.silver_yearly,
            (CheckoutPlan::Gold, BillingInterval::Monthly) => &self.gold_monthly,
            (CheckoutPlan::Gold, BillingInterval::Yearly) => &self.gold_yearly,
        };
        if raw.is_empty() {
            None
        } else {
            Some(raw.as_str())
        }
    }

    /// Reverse lookup: a Stripe Price ID seen in a webhook → plan slug.
    /// Used by the subscription upsert to set the `tier` column when the
    /// Price's metadata is missing.
    pub fn plan_for(&self, price_id: &str) -> Option<CheckoutPlan> {
        if price_id == self.silver_monthly || price_id == self.silver_yearly {
            Some(CheckoutPlan::Silver)
        } else if price_id == self.gold_monthly || price_id == self.gold_yearly {
            Some(CheckoutPlan::Gold)
        } else {
            None
        }
    }
}

impl CheckoutPlan {
    /// Stable lowercase slug stored in the `subscriptions.tier` ENUM.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Silver => "silver",
            Self::Gold => "gold",
        }
    }
}
