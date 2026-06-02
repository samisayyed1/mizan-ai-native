//! Zakat assessment service.
//!
//! The math (`assess`) is pure and table-tested. Portfolio aggregation
//! (`assess_portfolio`) wires the existing `holdings_service` into the math —
//! the caller picks Nisab in base currency and we route every holding to one
//! of three Zakatable buckets per [`AssetKind`].

use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;

use super::zakat_model::{ZakatInputs, ZakatReport};
use super::zakat_traits::ZakatServiceTrait;
use crate::assets::AssetKind;
use crate::errors::Result;
use crate::portfolio::holdings::{HoldingType, HoldingsServiceTrait};

/// Standard Zakat rate: 2.5% per lunar year.
const ZAKAT_RATE: Decimal = dec!(0.025);

/// Default disclaimer the UI always renders below the number.
const DEFAULT_NOTE: &str =
    "This is an arithmetic estimate, not religious guidance. Confirm the inputs and standard \
     (gold-Nisab vs. silver-Nisab, school of jurisprudence) with your imam before paying.";

pub struct ZakatService {
    holdings: Arc<dyn HoldingsServiceTrait>,
}

impl ZakatService {
    pub fn new(holdings: Arc<dyn HoldingsServiceTrait>) -> Self {
        Self { holdings }
    }
}

/// Stateless math entry — exposed for unit tests and the assistant tool.
/// Negative `net_zakat_base` (user is net-indebted) collapses to zero Zakat
/// regardless of whether assets cross Nisab.
pub fn assess(inputs: ZakatInputs) -> ZakatReport {
    let total_assets = inputs.liquid_cash + inputs.precious_metals + inputs.tradable_assets;
    let net_base = total_assets - inputs.short_term_debts;
    let above = net_base >= inputs.nisab && net_base > Decimal::ZERO;
    let due = if above {
        net_base * ZAKAT_RATE
    } else {
        Decimal::ZERO
    };

    ZakatReport {
        total_assessable_assets: total_assets,
        deductible_debts: inputs.short_term_debts,
        net_zakat_base: net_base,
        nisab_threshold: inputs.nisab,
        is_above_nisab: above,
        zakat_due: due,
        currency: inputs.currency,
        notes: vec![DEFAULT_NOTE.to_string()],
    }
}

#[async_trait]
impl ZakatServiceTrait for ZakatService {
    fn assess(&self, inputs: ZakatInputs) -> ZakatReport {
        assess(inputs)
    }

    async fn assess_portfolio(&self, base_currency: &str, nisab: Decimal) -> Result<ZakatReport> {
        // Aggregate the consolidated portfolio. The "TOTAL" sentinel account
        // holds every alt-asset; per-real-account holdings cover the
        // securities side. We sum once across both via the holdings service's
        // canonical `get_holdings(account, base)` API.
        let holdings = self
            .holdings
            .get_holdings(crate::constants::PORTFOLIO_TOTAL_ACCOUNT_ID, base_currency)
            .await?;

        let mut liquid = Decimal::ZERO;
        let mut metals = Decimal::ZERO;
        let mut tradable = Decimal::ZERO;
        let mut short_term_debts = Decimal::ZERO;
        // Track "Other"-kind holdings we routed into tradable so the
        // report can flag them for the user to review with their imam.
        let mut other_kind_count: usize = 0;
        let mut other_kind_value = Decimal::ZERO;

        for h in &holdings {
            // Each holding carries a base-currency value in `market_value.base`.
            // Route by holding type / asset kind into the four buckets the
            // Zakat math expects. Liabilities feed the debt bucket; consumer-
            // use assets (property, collectibles, vehicles) are excluded per
            // the majority jurisprudence view.
            let value = h.market_value.base;
            match (&h.holding_type, &h.asset_kind) {
                (_, Some(AssetKind::Liability)) => short_term_debts += value.abs(),
                (HoldingType::Cash, _) => liquid += value,
                (_, Some(AssetKind::PreciousMetal)) => metals += value,
                (_, Some(AssetKind::Investment)) => tradable += value,
                // Private equity is generally zakatable: the underlying
                // business holds zakatable assets and the share is held for
                // appreciation, not consumption. Treat as tradable.
                (_, Some(AssetKind::PrivateEquity)) => tradable += value,
                // Consumer-use exclusions (majority view: not zakatable).
                (_, Some(AssetKind::Property))
                | (_, Some(AssetKind::Vehicle))
                | (_, Some(AssetKind::Collectible)) => {}
                // FX is infrastructure (not directly holdable per the enum).
                (_, Some(AssetKind::Fx)) => {}
                // Unknown/unclassified: include conservatively in tradable
                // AND flag in the report so the user reviews with their
                // imam. Silently skipping was a zakat under-statement risk.
                (_, Some(AssetKind::Other)) | (_, None) => {
                    tradable += value;
                    other_kind_count += 1;
                    other_kind_value += value;
                }
            }
        }

        let mut report = assess(ZakatInputs {
            liquid_cash: liquid,
            precious_metals: metals,
            tradable_assets: tradable,
            short_term_debts,
            nisab,
            currency: Some(base_currency.to_string()),
        });
        // Add a portfolio-specific note clarifying what was excluded.
        report.notes.push(
            "Property, collectibles, and vehicles were excluded (consumer-use, not held \
             for resale). Long-term-held stocks/ETFs, crypto, sukuk, treasuries and private \
             equity are included as `tradable assets` per the most common modern \
             interpretation."
                .to_string(),
        );
        if other_kind_count > 0 {
            report.notes.push(format!(
                "{} unclassified holding(s) worth {:.2} {} were included as tradable assets \
                 (conservative inclusion). Set each asset's `kind` correctly under Settings \
                 → Assets so this is no longer ambiguous.",
                other_kind_count, other_kind_value, base_currency
            ));
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::holdings::holdings_model::{Holding, MonetaryValue};
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use std::sync::Arc;

    /// Builds a Holding carrying only the fields the zakat aggregator
    /// reads (holding_type, asset_kind, market_value.base). Everything
    /// else is defaulted — this is intentionally a "zakat-shaped"
    /// fixture, not a fully populated holding.
    fn zakat_holding(holding_type: HoldingType, kind: Option<AssetKind>, base: Decimal) -> Holding {
        Holding {
            id: "test".to_string(),
            account_id: crate::constants::PORTFOLIO_TOTAL_ACCOUNT_ID.to_string(),
            holding_type,
            instrument: None,
            asset_kind: kind,
            quantity: Decimal::ZERO,
            open_date: None,
            lots: None,
            contract_multiplier: Decimal::ONE,
            local_currency: "USD".to_string(),
            base_currency: "USD".to_string(),
            fx_rate: None,
            market_value: MonetaryValue { local: base, base },
            cost_basis: None,
            price: None,
            purchase_price: None,
            unrealized_gain: None,
            unrealized_gain_pct: None,
            realized_gain: None,
            realized_gain_pct: None,
            dividend_income: None,
            total_gain: None,
            total_gain_pct: None,
            day_change: None,
            day_change_pct: None,
            prev_close_value: None,
            weight: Decimal::ZERO,
            as_of_date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            metadata: None,
        }
    }

    struct StubHoldingsService {
        holdings: Vec<Holding>,
    }

    #[async_trait]
    impl HoldingsServiceTrait for StubHoldingsService {
        async fn get_holdings(
            &self,
            _account_id: &str,
            _base_currency: &str,
        ) -> Result<Vec<Holding>> {
            Ok(self.holdings.clone())
        }

        async fn get_holding(
            &self,
            _account_id: &str,
            _asset_id: &str,
            _base_currency: &str,
        ) -> Result<Option<Holding>> {
            Ok(None)
        }

        async fn holdings_from_snapshot(
            &self,
            _snapshot: &crate::portfolio::snapshot::AccountStateSnapshot,
            _base_currency: &str,
        ) -> Result<Vec<Holding>> {
            Ok(self.holdings.clone())
        }
    }

    fn inputs(cash: i64, metals: i64, tradable: i64, debts: i64, nisab: i64) -> ZakatInputs {
        ZakatInputs {
            liquid_cash: Decimal::from(cash),
            precious_metals: Decimal::from(metals),
            tradable_assets: Decimal::from(tradable),
            short_term_debts: Decimal::from(debts),
            nisab: Decimal::from(nisab),
            currency: Some("USD".to_string()),
        }
    }

    #[test]
    fn below_nisab_owes_nothing() {
        // Total $4,000; Nisab $5,000 → no Zakat.
        let r = assess(inputs(4_000, 0, 0, 0, 5_000));
        assert!(!r.is_above_nisab);
        assert_eq!(r.zakat_due, Decimal::ZERO);
    }

    #[test]
    fn exactly_at_nisab_owes_zakat() {
        // The boundary is inclusive — at Nisab you owe.
        let r = assess(inputs(5_000, 0, 0, 0, 5_000));
        assert!(r.is_above_nisab);
        assert_eq!(r.zakat_due, dec!(125)); // 2.5% of 5000
    }

    #[test]
    fn above_nisab_charges_two_point_five_percent() {
        // Cash $50k + metals $20k + tradable $10k − debts $5k = $75k.
        // Nisab $5k → 2.5% of 75k = $1,875.
        let r = assess(inputs(50_000, 20_000, 10_000, 5_000, 5_000));
        assert_eq!(r.total_assessable_assets, dec!(80_000));
        assert_eq!(r.net_zakat_base, dec!(75_000));
        assert!(r.is_above_nisab);
        assert_eq!(r.zakat_due, dec!(1_875));
    }

    #[test]
    fn net_indebted_owes_nothing() {
        // Assets $10k, debts $15k → net -$5k → 0.
        let r = assess(inputs(10_000, 0, 0, 15_000, 5_000));
        assert!(!r.is_above_nisab);
        assert_eq!(r.zakat_due, Decimal::ZERO);
        assert_eq!(r.net_zakat_base, dec!(-5_000));
    }

    #[test]
    fn debts_reduce_zakatable_base() {
        // Cash $10k, no metals/tradable, debts $4k → net $6k > Nisab $5k.
        // 2.5% of 6000 = $150.
        let r = assess(inputs(10_000, 0, 0, 4_000, 5_000));
        assert_eq!(r.zakat_due, dec!(150));
    }

    #[test]
    fn always_emits_default_disclaimer() {
        let r = assess(inputs(10_000, 0, 0, 0, 5_000));
        assert!(!r.notes.is_empty());
        assert!(r.notes[0].contains("imam"));
    }

    #[test]
    fn fractional_amounts_round_to_decimal_precision() {
        // 2.5% of $7,777.77 = $194.44425 → expect full precision, no rounding loss.
        let r = assess(ZakatInputs {
            liquid_cash: dec!(7_777.77),
            precious_metals: Decimal::ZERO,
            tradable_assets: Decimal::ZERO,
            short_term_debts: Decimal::ZERO,
            nisab: dec!(5_000),
            currency: Some("USD".to_string()),
        });
        assert_eq!(r.zakat_due, dec!(194.444250));
    }

    #[test]
    fn currency_round_trips_into_report() {
        let r = assess(ZakatInputs {
            currency: Some("SAR".to_string()),
            ..inputs(50_000, 0, 0, 0, 5_000)
        });
        assert_eq!(r.currency.as_deref(), Some("SAR"));
    }

    // QA Pass 9 — assess_portfolio routing regressions.
    //
    // Before the fix, `AssetKind::PrivateEquity` and `AssetKind::Other`
    // fell through the match arm and were silently excluded from
    // tradable_assets, under-stating zakat due. These tests pin the new
    // conservative-include contract.

    #[tokio::test]
    async fn assess_portfolio_routes_private_equity_to_tradable() {
        let holdings = vec![
            // $80k startup equity stake.
            zakat_holding(
                HoldingType::AlternativeAsset,
                Some(AssetKind::PrivateEquity),
                dec!(80_000),
            ),
            // $5k cash to lift us cleanly above Nisab.
            zakat_holding(HoldingType::Cash, None, dec!(5_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc.assess_portfolio("USD", dec!(5_000)).await.unwrap();

        // Tradable bucket must include the $80k PE plus the $5k liquid.
        assert_eq!(report.total_assessable_assets, dec!(85_000));
        assert_eq!(report.net_zakat_base, dec!(85_000));
        assert!(report.is_above_nisab);
        // 2.5% × $85,000 = $2,125 — the figure the user would have
        // historically under-paid by ~$2,000 when PE silently fell out.
        assert_eq!(report.zakat_due, dec!(2_125));
    }

    #[tokio::test]
    async fn assess_portfolio_includes_unclassified_other_holdings_and_flags_them() {
        let holdings = vec![
            // $20k unclassified — user hasn't set a kind yet.
            zakat_holding(HoldingType::Security, Some(AssetKind::Other), dec!(20_000)),
            // $5k cash to lift above Nisab.
            zakat_holding(HoldingType::Cash, None, dec!(5_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc.assess_portfolio("USD", dec!(5_000)).await.unwrap();

        assert_eq!(report.total_assessable_assets, dec!(25_000));
        assert_eq!(report.zakat_due, dec!(625)); // 2.5% × 25k
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("unclassified holding")),
            "Other-kind inclusion must be flagged in report.notes so the \
             user reviews classification with their imam. Got: {:?}",
            report.notes
        );
    }

    #[tokio::test]
    async fn assess_portfolio_excludes_consumer_use_assets() {
        let holdings = vec![
            // Cash above Nisab.
            zakat_holding(HoldingType::Cash, None, dec!(10_000)),
            // Consumer-use items (correctly excluded per majority view).
            zakat_holding(
                HoldingType::AlternativeAsset,
                Some(AssetKind::Property),
                dec!(500_000),
            ),
            zakat_holding(
                HoldingType::AlternativeAsset,
                Some(AssetKind::Vehicle),
                dec!(45_000),
            ),
            zakat_holding(
                HoldingType::AlternativeAsset,
                Some(AssetKind::Collectible),
                dec!(25_000),
            ),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc.assess_portfolio("USD", dec!(5_000)).await.unwrap();

        // Only the $10k cash is assessable — the $570k of consumer-use
        // assets is excluded.
        assert_eq!(report.total_assessable_assets, dec!(10_000));
        assert_eq!(report.zakat_due, dec!(250)); // 2.5% × 10k
    }

    #[tokio::test]
    async fn assess_portfolio_subtracts_liabilities_correctly() {
        // Liability values come in negative from the holdings service
        // (it's a debt, not an asset). assess_portfolio must take .abs()
        // before feeding short_term_debts.
        let holdings = vec![
            zakat_holding(HoldingType::Cash, None, dec!(20_000)),
            zakat_holding(
                HoldingType::AlternativeAsset,
                Some(AssetKind::Liability),
                dec!(-7_000),
            ),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc.assess_portfolio("USD", dec!(5_000)).await.unwrap();

        assert_eq!(report.total_assessable_assets, dec!(20_000));
        assert_eq!(report.deductible_debts, dec!(7_000));
        assert_eq!(report.net_zakat_base, dec!(13_000));
        assert_eq!(report.zakat_due, dec!(325)); // 2.5% × 13k
    }
}
