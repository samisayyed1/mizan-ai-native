//! Trait surface for the Zakat service.

use async_trait::async_trait;

use super::zakat_model::{ZakatInputs, ZakatReport};
use crate::errors::Result;

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
    async fn assess_portfolio(
        &self,
        base_currency: &str,
        nisab: rust_decimal::Decimal,
    ) -> Result<ZakatReport>;
}
