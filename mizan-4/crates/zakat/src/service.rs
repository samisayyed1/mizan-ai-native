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

use super::model::{School, ZakatInputs, ZakatReport};
use super::traits::ZakatServiceTrait;
// Track H PR-H3.b note: the engine currently consumes mizan-core's AssetKind
// and HoldingType. mizan-domain-types declares duplicates per ADR 0003 but
// the unification (single canonical AssetKind/HoldingType) lands in a
// follow-up sweep PR that touches every consumer at once. Until then,
// mizan-zakat reads from mizan-core to match what HoldingsServiceTrait
// returns. Tracked in docs/plans/track-h-extractions/02-zakat.md.
use mizan_core::assets::AssetKind;
use mizan_core::errors::Result;
use mizan_core::portfolio::holdings::{HoldingType, HoldingsServiceTrait};

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
///
/// # School branching (PR-F2)
///
/// `inputs.school` selects the school of jurisprudence. The Hanafi
/// math today is shared by all four schools at the arithmetic level;
/// what differs is the `notes` array on the result so the audit trail
/// records which school produced the number. School-specific MATH
/// (Maliki real-estate intent + Hanbali debt deduction) lands in
/// PR-F2.b/c.
pub fn assess(inputs: ZakatInputs) -> ZakatReport {
    let school = inputs.school;
    let total_assets = inputs.liquid_cash + inputs.precious_metals + inputs.tradable_assets;
    let net_base = total_assets - inputs.short_term_debts;
    let above = net_base >= inputs.nisab && net_base > Decimal::ZERO;
    let due = if above {
        net_base * ZAKAT_RATE
    } else {
        Decimal::ZERO
    };

    let notes = vec![DEFAULT_NOTE.to_string(), school.school_note().to_string()];

    ZakatReport {
        total_assessable_assets: total_assets,
        deductible_debts: inputs.short_term_debts,
        net_zakat_base: net_base,
        nisab_threshold: inputs.nisab,
        is_above_nisab: above,
        zakat_due: due,
        currency: inputs.currency,
        notes,
        school,
    }
}

/// Assess Zakat against an explicit school selector. Convenience wrapper
/// around `assess` that constructs the inputs with the school filled in.
/// Used by the desktop's `compute_zakat` Tauri command + the AI agent's
/// `compute_zakat` tool (PR-C12.b).
pub fn assess_for_school(mut inputs: ZakatInputs, school: School) -> ZakatReport {
    inputs.school = school;
    assess(inputs)
}

/// Read `metadata.property.intent` from a Holding's optional metadata
/// JSON. Returns [`PropertyIntent::Unknown`] for any holding without
/// a declared intent — under Maliki's conservative routing this means
/// the property flows into `tradable_assets` (see `route_property`).
///
/// Looks for both `metadata.property.intent` (the v2 shape per the
/// autonomous directive) and the flat `metadata.intent` key (the v1
/// shape that some legacy fixtures still use).
fn extract_property_intent(
    h: &mizan_core::portfolio::holdings::holdings_model::Holding,
) -> super::property_intent::PropertyIntent {
    use super::property_intent::PropertyIntent;
    let Some(meta) = h.metadata.as_ref() else {
        return PropertyIntent::Unknown;
    };
    // v2 shape: metadata.property.intent
    if let Some(intent) = meta
        .get("property")
        .and_then(|p| p.get("intent"))
        .and_then(|i| i.as_str())
    {
        return PropertyIntent::parse(intent);
    }
    // v1 shape: metadata.intent (some legacy fixtures)
    if let Some(intent) = meta.get("intent").and_then(|i| i.as_str()) {
        return PropertyIntent::parse(intent);
    }
    PropertyIntent::Unknown
}

#[async_trait]
impl ZakatServiceTrait for ZakatService {
    fn assess(&self, inputs: ZakatInputs) -> ZakatReport {
        assess(inputs)
    }

    async fn assess_portfolio(&self, base_currency: &str, nisab: Decimal) -> Result<ZakatReport> {
        // Backward-compat shim: delegate to the school-aware path with
        // Hanafi (the default school). This preserves the legacy
        // behaviour for any caller still on the old API.
        self.assess_portfolio_for_school(School::Hanafi, base_currency, nisab)
            .await
    }

    async fn assess_portfolio_for_school(
        &self,
        school: School,
        base_currency: &str,
        nisab: Decimal,
    ) -> Result<ZakatReport> {
        use super::property_intent::{route_property, PropertyBucket};

        // Aggregate the consolidated portfolio. The "TOTAL" sentinel account
        // holds every alt-asset; per-real-account holdings cover the
        // securities side. We sum once across both via the holdings service's
        // canonical `get_holdings(account, base)` API.
        let holdings = self
            .holdings
            .get_holdings(
                mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID,
                base_currency,
            )
            .await?;

        let mut liquid = Decimal::ZERO;
        let mut metals = Decimal::ZERO;
        let mut tradable = Decimal::ZERO;
        let mut short_term_debts = Decimal::ZERO;
        // Track "Other"-kind holdings we routed into tradable so the
        // report can flag them for the user to review with their imam.
        let mut other_kind_count: usize = 0;
        let mut other_kind_value = Decimal::ZERO;
        // Track per-school property routing so the audit trail can
        // surface a "$X of for-sale property routed via Maliki" note.
        let mut maliki_property_tradable = Decimal::ZERO;
        let mut maliki_property_unknown_intent_count: usize = 0;

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
                // Property — PR-F2.b.1: school-aware routing via the
                // route_property table (ADR 0015 for Maliki). Maliki
                // routes for-sale property into tradable; other schools
                // keep the consumer-use exclusion baseline.
                (_, Some(AssetKind::Property)) => {
                    let intent = extract_property_intent(h);
                    match route_property(school, intent) {
                        PropertyBucket::Tradable => {
                            tradable += value;
                            if matches!(intent, super::property_intent::PropertyIntent::Unknown) {
                                maliki_property_unknown_intent_count += 1;
                            }
                            maliki_property_tradable += value;
                        }
                        PropertyBucket::Exempt => {}
                    }
                }
                // Vehicles + collectibles: always consumer-use exempt.
                (_, Some(AssetKind::Vehicle)) | (_, Some(AssetKind::Collectible)) => {}
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
            school,
        });
        // Add a portfolio-specific note clarifying what was excluded.
        // Wording branches by school so the user's imam sees exactly
        // what was routed where.
        if matches!(school, School::Maliki) {
            report.notes.push(format!(
                "Maliki routing applied (ADR 0015): {} {} of for-sale / unknown-intent property \
                 routed into tradable assets at market value. Primary-residence and rental \
                 property remain exempt from market-value Zakat (rental income routes separately \
                 via cash flows). Vehicles and collectibles are excluded.",
                maliki_property_tradable, base_currency,
            ));
            if maliki_property_unknown_intent_count > 0 {
                report.notes.push(format!(
                    "{} property holding(s) had no `metadata.property.intent` declared — \
                     conservatively routed into tradable assets under Maliki. Set intent under \
                     Settings → Assets so future calculations are precise.",
                    maliki_property_unknown_intent_count,
                ));
            }
        } else {
            report.notes.push(
                "Property, collectibles, and vehicles were excluded (consumer-use, not held \
                 for resale). Long-term-held stocks/ETFs, crypto, sukuk, treasuries and private \
                 equity are included as `tradable assets` per the most common modern \
                 interpretation."
                    .to_string(),
            );
        }
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
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use mizan_core::portfolio::holdings::holdings_model::{Holding, MonetaryValue};
    use std::sync::Arc;

    /// Builds a Holding carrying only the fields the zakat aggregator
    /// reads (holding_type, asset_kind, market_value.base). Everything
    /// else is defaulted — this is intentionally a "zakat-shaped"
    /// fixture, not a fully populated holding.
    fn zakat_holding(holding_type: HoldingType, kind: Option<AssetKind>, base: Decimal) -> Holding {
        Holding {
            id: "test".to_string(),
            account_id: mizan_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID.to_string(),
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
            _snapshot: &mizan_core::portfolio::snapshot::AccountStateSnapshot,
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
            school: School::default(),
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
            school: School::default(),
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

    // ─── PR-F2: School enum + per-school branching ────────────────

    #[test]
    fn school_default_is_hanafi() {
        assert_eq!(School::default(), School::Hanafi);
    }

    #[test]
    fn school_parse_canonical_names() {
        assert_eq!(School::parse("hanafi"), Some(School::Hanafi));
        assert_eq!(School::parse("Hanafi"), Some(School::Hanafi));
        assert_eq!(School::parse("  HANAFI  "), Some(School::Hanafi));
        assert_eq!(School::parse("shafii"), Some(School::Shafii));
        assert_eq!(School::parse("shafi'i"), Some(School::Shafii));
        assert_eq!(School::parse("shafi-i"), Some(School::Shafii));
        assert_eq!(School::parse("shafi"), Some(School::Shafii));
        assert_eq!(School::parse("maliki"), Some(School::Maliki));
        assert_eq!(School::parse("hanbali"), Some(School::Hanbali));
    }

    #[test]
    fn school_parse_unknown_returns_none() {
        assert_eq!(School::parse("zaydi"), None);
        assert_eq!(School::parse(""), None);
        assert_eq!(School::parse("ibadi"), None);
    }

    #[test]
    fn school_labels_match_canonical_spelling() {
        assert_eq!(School::Hanafi.label(), "Hanafi");
        assert_eq!(School::Shafii.label(), "Shafi'i");
        assert_eq!(School::Maliki.label(), "Maliki");
        assert_eq!(School::Hanbali.label(), "Hanbali");
    }

    #[test]
    fn school_notes_reference_relevant_adr() {
        assert!(
            School::Maliki.school_note().contains("ADR 0015"),
            "Maliki note must cite ADR 0015 so the audit trail surfaces the source"
        );
        assert!(
            School::Hanbali.school_note().contains("ADR 0016"),
            "Hanbali note must cite ADR 0016"
        );
        assert!(School::Hanafi.school_note().contains("Hanafi"));
        assert!(School::Shafii.school_note().contains("Shafi"));
    }

    #[test]
    fn assess_includes_school_note_in_report() {
        let r = assess(ZakatInputs {
            school: School::Maliki,
            ..inputs(100_000, 0, 0, 0, 5_000)
        });
        assert_eq!(r.school, School::Maliki);
        // Notes must include both the universal disclaimer AND the school-specific note
        assert!(r.notes.len() >= 2);
        assert!(r.notes[0].contains("imam"));
        assert!(
            r.notes.iter().any(|n| n.contains("Maliki")),
            "Expected a Maliki-specific note in {:?}",
            r.notes
        );
    }

    #[test]
    fn assess_for_school_overrides_inputs_school() {
        let inputs_hanafi = ZakatInputs {
            school: School::Hanafi,
            ..inputs(100_000, 0, 0, 0, 5_000)
        };
        let r = assess_for_school(inputs_hanafi, School::Hanbali);
        assert_eq!(r.school, School::Hanbali);
        assert!(r.notes.iter().any(|n| n.contains("Hanbali")));
    }

    #[test]
    fn all_four_schools_produce_same_arithmetic_today() {
        // PR-F2 ships the enum + branching plumbing; PR-F2.b/c will
        // diverge the math. Until then all four schools must produce
        // the same number for the same inputs — this test pins that
        // invariant so PR-F2.b/c will deliberately break it.
        let base = inputs(100_000, 0, 0, 0, 5_000);
        let r_hanafi = assess_for_school(base.clone(), School::Hanafi);
        let r_shafii = assess_for_school(base.clone(), School::Shafii);
        let r_maliki = assess_for_school(base.clone(), School::Maliki);
        let r_hanbali = assess_for_school(base, School::Hanbali);
        assert_eq!(r_hanafi.zakat_due, r_shafii.zakat_due);
        assert_eq!(r_hanafi.zakat_due, r_maliki.zakat_due);
        assert_eq!(r_hanafi.zakat_due, r_hanbali.zakat_due);
        // But the school field + notes differ — audit trail discriminator.
        assert_ne!(r_hanafi.school, r_maliki.school);
        assert_ne!(r_hanafi.notes, r_maliki.notes);
    }

    #[test]
    fn school_serializes_lowercase() {
        let json = serde_json::to_string(&School::Maliki).expect("ok");
        assert_eq!(json, "\"maliki\"");
        let parsed: School = serde_json::from_str("\"hanbali\"").expect("ok");
        assert_eq!(parsed, School::Hanbali);
    }

    #[test]
    fn school_deserialize_accepts_shafii_aliases() {
        // Per #[serde(alias)] on the variant
        let parsed: School = serde_json::from_str("\"shafi-i\"").expect("ok");
        assert_eq!(parsed, School::Shafii);
        let parsed: School = serde_json::from_str("\"shafi'i\"").expect("ok");
        assert_eq!(parsed, School::Shafii);
    }

    // ─── PR-F2.b.1: route_property wired into assess_portfolio ────

    /// Build a Property-kind Holding with `metadata.property.intent` set.
    fn property_holding(base: Decimal, intent: &str) -> Holding {
        let mut h = zakat_holding(HoldingType::Security, Some(AssetKind::Property), base);
        h.metadata = Some(serde_json::json!({
            "property": { "intent": intent }
        }));
        h
    }

    #[tokio::test]
    async fn s23_singapore_fixture_maliki_routes_for_sale_into_tradable() {
        // §23 reference user:
        //   - Bukit Batok primary-residence ($800K) → exempt under Maliki
        //   - 3 Hyderabad rentals ($250K + $225K + $225K) → exempt
        //     (rental income routes via cash flows, PR-F2.b.2)
        //   - 1 Hyderabad for-sale unit ($300K) → tradable under Maliki
        // Plus $10K liquid cash so the report is above Nisab.
        let holdings = vec![
            property_holding(dec!(800_000), "primary-residence"),
            property_holding(dec!(250_000), "for-rent"),
            property_holding(dec!(225_000), "for-rent"),
            property_holding(dec!(225_000), "for-rent"),
            property_holding(dec!(300_000), "for-sale"),
            zakat_holding(HoldingType::Cash, None, dec!(10_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc
            .assess_portfolio_for_school(School::Maliki, "USD", dec!(5_000))
            .await
            .unwrap();

        // Under Maliki: cash $10K + for-sale property $300K = $310K
        assert_eq!(report.total_assessable_assets, dec!(310_000));
        assert_eq!(report.school, School::Maliki);
        assert!(report.is_above_nisab);
        assert_eq!(report.zakat_due, dec!(7_750)); // 2.5% × 310k
                                                   // Report notes must mention the Maliki routing
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("Maliki routing applied")),
            "Maliki note missing; notes: {:?}",
            report.notes
        );
    }

    #[tokio::test]
    async fn s23_singapore_fixture_hanafi_excludes_all_property() {
        // Same fixture as above, but under Hanafi (default) — ALL
        // property is consumer-use exempt. Only cash $10K counts.
        let holdings = vec![
            property_holding(dec!(800_000), "primary-residence"),
            property_holding(dec!(250_000), "for-rent"),
            property_holding(dec!(225_000), "for-rent"),
            property_holding(dec!(225_000), "for-rent"),
            property_holding(dec!(300_000), "for-sale"),
            zakat_holding(HoldingType::Cash, None, dec!(10_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc
            .assess_portfolio_for_school(School::Hanafi, "USD", dec!(5_000))
            .await
            .unwrap();

        assert_eq!(report.total_assessable_assets, dec!(10_000));
        assert_eq!(report.school, School::Hanafi);
        assert_eq!(report.zakat_due, dec!(250)); // 2.5% × 10k
    }

    #[tokio::test]
    async fn assess_portfolio_backward_compat_default_is_hanafi() {
        // Existing assess_portfolio(&self, base, nisab) callers must
        // continue to get the Hanafi consumer-use exclusion baseline.
        let holdings = vec![
            property_holding(dec!(500_000), "for-sale"),
            zakat_holding(HoldingType::Cash, None, dec!(10_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc.assess_portfolio("USD", dec!(5_000)).await.unwrap();
        // Backward-compat: school = Hanafi → for-sale property exempt
        assert_eq!(report.total_assessable_assets, dec!(10_000));
        assert_eq!(report.school, School::Hanafi);
    }

    #[tokio::test]
    async fn maliki_unknown_intent_property_flagged_in_notes() {
        let holdings = vec![
            property_holding(dec!(400_000), "investment-condo"), // unrecognised → Unknown
            zakat_holding(HoldingType::Cash, None, dec!(10_000)),
        ];
        let svc = ZakatService::new(Arc::new(StubHoldingsService { holdings }));
        let report = svc
            .assess_portfolio_for_school(School::Maliki, "USD", dec!(5_000))
            .await
            .unwrap();
        // Conservative inclusion: unknown intent → tradable
        assert_eq!(report.total_assessable_assets, dec!(410_000));
        // And the report flags it
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("no `metadata.property.intent` declared")),
            "Unknown-intent note missing; notes: {:?}",
            report.notes
        );
    }

    #[tokio::test]
    async fn extract_property_intent_handles_missing_metadata() {
        // No metadata at all → PropertyIntent::Unknown → under Maliki,
        // conservative inclusion. Pin via assess_portfolio_for_school.
        let h = zakat_holding(
            HoldingType::Security,
            Some(AssetKind::Property),
            dec!(200_000),
        );
        let svc = ZakatService::new(Arc::new(StubHoldingsService {
            holdings: vec![h, zakat_holding(HoldingType::Cash, None, dec!(10_000))],
        }));
        let report = svc
            .assess_portfolio_for_school(School::Maliki, "USD", dec!(5_000))
            .await
            .unwrap();
        assert_eq!(report.total_assessable_assets, dec!(210_000));
    }

    #[tokio::test]
    async fn extract_property_intent_v1_flat_metadata_shape() {
        // Legacy fixtures use `metadata.intent` directly (no nested
        // `property` key). The reader must accept both shapes.
        let mut h = zakat_holding(
            HoldingType::Security,
            Some(AssetKind::Property),
            dec!(200_000),
        );
        h.metadata = Some(serde_json::json!({ "intent": "for-sale" }));
        let svc = ZakatService::new(Arc::new(StubHoldingsService {
            holdings: vec![h, zakat_holding(HoldingType::Cash, None, dec!(10_000))],
        }));
        let report = svc
            .assess_portfolio_for_school(School::Maliki, "USD", dec!(5_000))
            .await
            .unwrap();
        // for-sale routes into tradable under Maliki
        assert_eq!(report.total_assessable_assets, dec!(210_000));
    }
}
