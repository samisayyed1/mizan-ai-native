//! Recurring stock-purchase plan (RSP) schedule generator.
//!
//! A user-facing RSP — also known as a SIP (systematic investment plan) or
//! dollar-cost-averaging schedule — is shaped by a small set of parameters:
//!
//! * symbol (e.g. "VTI", "AAPL")
//! * cash amount per buy (e.g. 500.00)
//! * reference unit price (e.g. 245.30 — the per-share price the user
//!   expects each buy to clear at; for past dates this is the historical
//!   close they're modelling, for future dates this is their estimate)
//! * frequency (weekly / biweekly / monthly / quarterly / semi-annual /
//!   annual)
//! * start date
//! * number of installments (e.g. 24 monthly installments = 2 years)
//! * currency
//!
//! From that we deterministically emit a series of `BUY` activities at
//! the correct purchase dates. Each BUY carries `quantity = amount /
//! unit_price` (rounded to 6 d.p. — the precision most brokers use for
//! fractional shares) and `unit_price = unit_price`, so the engine treats
//! them exactly like any manually-entered or broker-synced BUY.
//!
//! The generator is a **pure function** so it's trivially testable; the
//! caller (ActivityService::create_recurring_buy_plan) is responsible
//! for actually inserting the resulting `NewActivity` rows.

use crate::activities::activities_constants::ACTIVITY_TYPE_BUY;
use crate::activities::activities_model::{ActivityStatus, AssetResolutionInput, NewActivity};
use chrono::{Days, Months, NaiveDate};
use rust_decimal::prelude::*;

/// Cadence of recurring purchases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RspFrequency {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

impl RspFrequency {
    /// Step in days for week-based cadences, `None` for month-based.
    fn days_step(self) -> Option<u64> {
        match self {
            Self::Weekly => Some(7),
            Self::Biweekly => Some(14),
            _ => None,
        }
    }

    /// Step in months for month-based cadences, `None` for week-based.
    fn months_step(self) -> Option<u32> {
        match self {
            Self::Monthly => Some(1),
            Self::Quarterly => Some(3),
            Self::SemiAnnual => Some(6),
            Self::Annual => Some(12),
            _ => None,
        }
    }
}

/// Caller-provided RSP parameters.
#[derive(Debug, Clone)]
pub struct RspParams {
    pub account_id: String,
    /// Ticker symbol (e.g. "VTI", "AAPL", "BTC-USD"). Required — RSP
    /// always targets a specific instrument.
    pub symbol: String,
    /// Optional exchange MIC code. Forwarded to the asset resolver so
    /// dual-listed tickers route to the correct venue.
    pub exchange_mic: Option<String>,
    /// Cash amount spent per scheduled buy.
    pub amount_per_buy: Decimal,
    /// Reference per-share price applied to every emitted BUY. The user
    /// can edit individual activities afterwards (e.g. once a real fill
    /// price is known) — the schedule itself is deterministic so the
    /// portfolio reflects the plan immediately.
    pub unit_price: Decimal,
    pub frequency: RspFrequency,
    pub start_date: NaiveDate,
    /// Number of buys to generate. 1..=240 — caps at 240 (e.g. 20-year
    /// monthly SIP) to prevent typo'd inputs from generating thousands
    /// of activities.
    pub installments: u32,
    pub currency: String,
    /// Free-form user note attached to every emitted BUY activity for
    /// traceability ("Monthly VTI SIP, $500/mo").
    pub notes: Option<String>,
}

/// Errors the scheduler raises before any activity is emitted.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RspSchedulerError {
    #[error("amount per buy must be positive (got {0})")]
    NonPositiveAmount(Decimal),
    #[error("unit price must be positive (got {0})")]
    NonPositivePrice(Decimal),
    #[error("symbol must not be empty")]
    EmptySymbol,
    #[error("number of installments must be at least 1")]
    ZeroInstallments,
    #[error("number of installments capped at 240 (got {0}); split into multiple plans if needed")]
    TooManyInstallments(u32),
    #[error("invalid date arithmetic — {start} + {step} {unit} overflows the calendar")]
    DateOverflow {
        start: NaiveDate,
        step: u64,
        unit: &'static str,
    },
}

/// Generate the schedule of BUY activities for a recurring purchase plan.
///
/// First buy lands on `start_date`; subsequent buys advance by the
/// frequency's step (7 days for weekly, 14 for biweekly, 1/3/6/12 months
/// for monthly/quarterly/semi-annual/annual).
///
/// Quantity is computed as `amount_per_buy / unit_price`, rounded to 6
/// decimal places (the precision most brokers report for fractional
/// shares). All emitted activities are POSTED — they affect portfolio
/// calculations immediately, just like any manually-entered BUY.
///
/// Returns a `Vec<NewActivity>` in purchase-date order. The caller is
/// responsible for the actual insert.
pub fn generate_rsp_schedule(params: &RspParams) -> Result<Vec<NewActivity>, RspSchedulerError> {
    if params.amount_per_buy <= Decimal::ZERO {
        return Err(RspSchedulerError::NonPositiveAmount(params.amount_per_buy));
    }
    if params.unit_price <= Decimal::ZERO {
        return Err(RspSchedulerError::NonPositivePrice(params.unit_price));
    }
    if params.symbol.trim().is_empty() {
        return Err(RspSchedulerError::EmptySymbol);
    }
    if params.installments == 0 {
        return Err(RspSchedulerError::ZeroInstallments);
    }
    if params.installments > 240 {
        return Err(RspSchedulerError::TooManyInstallments(params.installments));
    }

    let quantity_per_buy = round_6dp(params.amount_per_buy / params.unit_price);

    let mut out = Vec::with_capacity(params.installments as usize);
    for n in 0..params.installments {
        let activity_date = step_date(params.start_date, params.frequency, n)?;
        out.push(make_buy_activity(params, activity_date, quantity_per_buy));
    }
    Ok(out)
}

fn step_date(
    start: NaiveDate,
    frequency: RspFrequency,
    n: u32,
) -> Result<NaiveDate, RspSchedulerError> {
    if let Some(days) = frequency.days_step() {
        let total_days = days.saturating_mul(u64::from(n));
        return start.checked_add_days(Days::new(total_days)).ok_or(
            RspSchedulerError::DateOverflow {
                start,
                step: total_days,
                unit: "days",
            },
        );
    }
    let months = frequency
        .months_step()
        .expect("non-week-based frequencies have a month step");
    let total_months = months.saturating_mul(n);
    start
        .checked_add_months(Months::new(total_months))
        .ok_or(RspSchedulerError::DateOverflow {
            start,
            step: u64::from(total_months),
            unit: "months",
        })
}

fn make_buy_activity(
    params: &RspParams,
    activity_date: NaiveDate,
    quantity: Decimal,
) -> NewActivity {
    NewActivity {
        id: None,
        account_id: params.account_id.clone(),
        asset: Some(AssetResolutionInput {
            id: None,
            symbol: Some(params.symbol.trim().to_uppercase()),
            exchange_mic: params.exchange_mic.clone(),
            kind: None,
            name: None,
            quote_mode: None,
            quote_ccy: None,
            instrument_type: None,
        }),
        activity_type: ACTIVITY_TYPE_BUY.to_string(),
        subtype: None,
        activity_date: activity_date.to_string(),
        quantity: Some(quantity),
        unit_price: Some(params.unit_price),
        currency: params.currency.clone(),
        fee: None,
        amount: None,
        status: Some(ActivityStatus::Posted),
        notes: params.notes.clone(),
        fx_rate: None,
        metadata: Some(format!(
            r#"{{"source":"rsp_schedule","symbol":"{}","amount_per_buy":"{}","unit_price":"{}","frequency":"{:?}","installments":{},"start_date":"{}"}}"#,
            params.symbol.trim().to_uppercase(),
            params.amount_per_buy,
            params.unit_price,
            params.frequency,
            params.installments,
            params.start_date
        )),
        needs_review: None,
        source_system: Some("RSP_SCHEDULE".to_string()),
        source_record_id: None,
        source_group_id: None,
        idempotency_key: None,
    }
}

fn round_6dp(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(6, RoundingStrategy::MidpointAwayFromZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn params(freq: RspFrequency, installments: u32) -> RspParams {
        RspParams {
            account_id: "acc-test".to_string(),
            symbol: "VTI".to_string(),
            exchange_mic: None,
            amount_per_buy: dec!(500),
            unit_price: dec!(250),
            frequency: freq,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            installments,
            currency: "USD".to_string(),
            notes: Some("Monthly VTI SIP, $500".to_string()),
        }
    }

    #[test]
    fn monthly_24_emits_24_buys_advancing_by_month() {
        let schedule = generate_rsp_schedule(&params(RspFrequency::Monthly, 24)).unwrap();
        assert_eq!(schedule.len(), 24);
        assert_eq!(schedule[0].activity_date, "2026-01-15");
        assert_eq!(schedule[1].activity_date, "2026-02-15");
        assert_eq!(schedule[11].activity_date, "2026-12-15");
        assert_eq!(schedule[12].activity_date, "2027-01-15");
        assert_eq!(schedule[23].activity_date, "2027-12-15");
        for activity in &schedule {
            assert_eq!(activity.activity_type, "BUY");
            assert_eq!(activity.quantity, Some(dec!(2.000000)));
            assert_eq!(activity.unit_price, Some(dec!(250)));
            assert_eq!(activity.currency, "USD");
            assert_eq!(activity.account_id, "acc-test");
            assert_eq!(activity.get_asset_symbol(), Some("VTI"));
            assert_eq!(activity.status, Some(ActivityStatus::Posted));
        }
    }

    #[test]
    fn weekly_4_emits_buys_7_days_apart() {
        let schedule = generate_rsp_schedule(&params(RspFrequency::Weekly, 4)).unwrap();
        assert_eq!(schedule.len(), 4);
        let dates: Vec<_> = schedule.iter().map(|a| a.activity_date.as_str()).collect();
        assert_eq!(
            dates,
            vec!["2026-01-15", "2026-01-22", "2026-01-29", "2026-02-05"]
        );
    }

    #[test]
    fn biweekly_3_emits_buys_14_days_apart() {
        let schedule = generate_rsp_schedule(&params(RspFrequency::Biweekly, 3)).unwrap();
        let dates: Vec<_> = schedule.iter().map(|a| a.activity_date.as_str()).collect();
        assert_eq!(dates, vec!["2026-01-15", "2026-01-29", "2026-02-12"]);
    }

    #[test]
    fn quarterly_8_emits_buys_3_months_apart() {
        let schedule = generate_rsp_schedule(&params(RspFrequency::Quarterly, 8)).unwrap();
        assert_eq!(schedule.len(), 8);
        assert_eq!(schedule[0].activity_date, "2026-01-15");
        assert_eq!(schedule[1].activity_date, "2026-04-15");
        assert_eq!(schedule[7].activity_date, "2027-10-15");
    }

    #[test]
    fn fractional_shares_rounded_to_6dp() {
        // $500 / $245.30 = 2.0383204...
        let mut p = params(RspFrequency::Monthly, 1);
        p.unit_price = dec!(245.30);
        let schedule = generate_rsp_schedule(&p).unwrap();
        assert_eq!(schedule[0].quantity, Some(dec!(2.038320)));
    }

    #[test]
    fn symbol_uppercased_and_trimmed() {
        let mut p = params(RspFrequency::Monthly, 1);
        p.symbol = "  vti  ".to_string();
        let schedule = generate_rsp_schedule(&p).unwrap();
        assert_eq!(schedule[0].get_asset_symbol(), Some("VTI"));
    }

    #[test]
    fn rejects_non_positive_amount() {
        let mut p = params(RspFrequency::Monthly, 12);
        p.amount_per_buy = dec!(0);
        assert!(matches!(
            generate_rsp_schedule(&p),
            Err(RspSchedulerError::NonPositiveAmount(_))
        ));
        p.amount_per_buy = dec!(-100);
        assert!(matches!(
            generate_rsp_schedule(&p),
            Err(RspSchedulerError::NonPositiveAmount(_))
        ));
    }

    #[test]
    fn rejects_non_positive_price() {
        let mut p = params(RspFrequency::Monthly, 12);
        p.unit_price = dec!(0);
        assert!(matches!(
            generate_rsp_schedule(&p),
            Err(RspSchedulerError::NonPositivePrice(_))
        ));
        p.unit_price = dec!(-1);
        assert!(matches!(
            generate_rsp_schedule(&p),
            Err(RspSchedulerError::NonPositivePrice(_))
        ));
    }

    #[test]
    fn rejects_empty_symbol() {
        let mut p = params(RspFrequency::Monthly, 12);
        p.symbol = "   ".to_string();
        assert_eq!(
            generate_rsp_schedule(&p).unwrap_err(),
            RspSchedulerError::EmptySymbol
        );
    }

    #[test]
    fn rejects_zero_installments() {
        let p = params(RspFrequency::Monthly, 0);
        assert_eq!(
            generate_rsp_schedule(&p).unwrap_err(),
            RspSchedulerError::ZeroInstallments
        );
    }

    #[test]
    fn rejects_too_many_installments() {
        let p = params(RspFrequency::Monthly, 241);
        assert!(matches!(
            generate_rsp_schedule(&p).unwrap_err(),
            RspSchedulerError::TooManyInstallments(241)
        ));
    }

    #[test]
    fn metadata_carries_traceability_back_to_rsp() {
        let schedule = generate_rsp_schedule(&params(RspFrequency::Monthly, 12)).unwrap();
        let metadata = schedule[0].metadata.as_ref().expect("metadata set");
        assert!(metadata.contains("\"source\":\"rsp_schedule\""));
        assert!(metadata.contains("\"symbol\":\"VTI\""));
        assert!(metadata.contains("\"amount_per_buy\":\"500\""));
        assert!(metadata.contains("\"unit_price\":\"250\""));
        assert!(metadata.contains("\"installments\":12"));
        assert_eq!(schedule[0].source_system.as_deref(), Some("RSP_SCHEDULE"));
    }

    #[test]
    fn passes_exchange_mic_through_to_asset() {
        let mut p = params(RspFrequency::Monthly, 1);
        p.exchange_mic = Some("XLON".to_string());
        let schedule = generate_rsp_schedule(&p).unwrap();
        assert_eq!(schedule[0].get_asset_exchange_mic(), Some("XLON"));
    }
}
