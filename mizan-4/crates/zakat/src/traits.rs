//! Trait surface for the Zakat service.

use async_trait::async_trait;

use super::model::{School, ZakatInputs, ZakatReport};
use mizan_core::errors::Result;

/// What the Tauri command + the assistant tool call into.
///
/// Lives behind a trait so the call sites can use a mock in tests without
/// reaching into the live `holdings_service` / `net_worth_service`.
#[async_trait]
pub trait ZakatServiceTrait: Send + Sync {
    /// Run the assessment with caller-supplied totals. Pure math — no DB
    /// access. Use this when the caller has already aggregated the inputs
    /// (e.g. a unit test or an external caller passing exact figures).
    fn assess(&self, inputs: ZakatInputs) -> ZakatReport;

    /// Aggregate the user's portfolio into [`ZakatInputs`] and assess.
    ///
    /// Inspects every account → routes each holding's market value into one
    /// of `liquid_cash` / `precious_metals` / `tradable_assets`, converts
    /// to base currency, then assesses. Short-term debts come from
    /// liabilities maturing within 12 months (per the existing
    /// alt-asset-metadata `loanDurationYears` + `originationDate` fields).
    ///
    /// Equivalent to `assess_portfolio_for_school(School::Hanafi, ...)` —
    /// preserved for backward compat with callers that haven't been
    /// updated to pass a school selector yet.
    async fn assess_portfolio(
        &self,
        base_currency: &str,
        nisab: rust_decimal::Decimal,
    ) -> Result<ZakatReport>;

    /// School-aware portfolio assessment — PR-F2.b.1 / Goal v3 §V Phase 8.
    ///
    /// Identical to [`assess_portfolio`] except the routing of each
    /// holding into the Zakat buckets honours the user's school of
    /// jurisprudence:
    ///
    /// - **Maliki** — Property holdings are routed via `route_property`
    ///   using `metadata.property.intent` (`primary-residence` /
    ///   `for-rent` / `for-sale`). `for-sale` flows to `tradable_assets`
    ///   at market value; everything else stays exempt today (rental
    ///   income tracking lands in PR-F2.b.2). Long-term mortgage
    ///   principal + locked retirement remain Hanafi-shaped under
    ///   Maliki at this layer; PR-F2.c.1 wires those for Hanbali.
    /// - **Hanafi / Shafi'i / Hanbali** — existing consumer-use
    ///   exclusion for all property. PR-F2.c.1 adds Hanbali debt
    ///   deduction + locked-retirement apportionment.
    ///
    /// Default impl delegates to `assess_portfolio` so existing trait
    /// implementors don't break. ZakatService overrides this to apply
    /// the school-specific routing.
    async fn assess_portfolio_for_school(
        &self,
        school: School,
        base_currency: &str,
        nisab: rust_decimal::Decimal,
    ) -> Result<ZakatReport> {
        // Default: ignore school selector — the consumer-use exclusion
        // baseline applies for any school except Maliki, and the
        // default impl here is what most mock implementations want.
        let _ = school;
        self.assess_portfolio(base_currency, nisab).await
    }
}
