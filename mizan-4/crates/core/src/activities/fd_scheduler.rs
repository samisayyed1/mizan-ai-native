//! Fixed-deposit (FD) interest schedule generator.
//!
//! A user-facing fixed deposit is shaped by a small set of parameters:
//!
//! * principal (e.g. 10,000)
//! * annual interest rate (e.g. 5.0 %)
//! * term in months (e.g. 12)
//! * payment frequency (monthly / quarterly / semi-annual / annual / at maturity)
//! * start date
//! * currency
//!
//! From that we deterministically emit a list of INTEREST activities at the
//! correct payment dates. Mizan's snapshot calculator already treats
//! `INTEREST` as a return (it grows cash without touching `net_contribution`),
//! so the FD's interest naturally feeds CAGR/TWR through the same path
//! every other interest payment uses. Principal is **not** modelled as a
//! separate position — the user keeps it in their cash account and the
//! interest payments accrue alongside.
//!
//! The generator is a **pure function** so it's trivially testable; the
//! caller (ActivityService::create_fixed_deposit_schedule) is responsible
//! for actually inserting the resulting `NewActivity` rows.

use crate::activities::activities_constants::ACTIVITY_TYPE_INTEREST;
use crate::activities::activities_model::NewActivity;
use chrono::{Months, NaiveDate};
use rust_decimal::prelude::*;

/// Cadence of FD interest payments. Maps to a fixed number of payments per
/// year; `AtMaturity` is a single lump-sum at the end of the term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FdPaymentFrequency {
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
    AtMaturity,
}

impl FdPaymentFrequency {
    /// Months between successive payments. None for `AtMaturity` (single
    /// payment at the end of the term).
    fn months_between(self) -> Option<u32> {
        match self {
            Self::Monthly => Some(1),
            Self::Quarterly => Some(3),
            Self::SemiAnnual => Some(6),
            Self::Annual => Some(12),
            Self::AtMaturity => None,
        }
    }

    /// Number of payments per year. Used to size each periodic interest
    /// payment as `principal * rate / payments_per_year`.
    fn payments_per_year(self) -> u32 {
        match self {
            Self::Monthly => 12,
            Self::Quarterly => 4,
            Self::SemiAnnual => 2,
            Self::Annual => 1,
            Self::AtMaturity => 1, // unused (only one payment, sized by term)
        }
    }
}

/// Caller-provided FD parameters.
///
/// `annual_rate_percent` is the human-readable rate (e.g. `5.0` for 5 %),
/// not a decimal fraction. The scheduler converts internally.
#[derive(Debug, Clone)]
pub struct FdParams {
    pub account_id: String,
    pub principal: Decimal,
    /// Annual rate as a percent (5.0 = 5%).
    pub annual_rate_percent: Decimal,
    /// Tenure of the deposit in whole months.
    pub term_months: u32,
    pub payment_frequency: FdPaymentFrequency,
    pub start_date: NaiveDate,
    pub currency: String,
    /// Free-form user note attached to every emitted INTEREST activity for
    /// traceability ("HSBC AED FD @ 5%, 12 mo, started 2026-01-15").
    pub notes: Option<String>,
}

/// Errors the scheduler raises before any activity is emitted.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FdSchedulerError {
    #[error("principal must be positive (got {0})")]
    NonPositivePrincipal(Decimal),
    #[error("annual rate must be non-negative (got {0})")]
    NegativeRate(Decimal),
    #[error("term must be at least 1 month")]
    ZeroTerm,
    #[error("term ({term_months} mo) is shorter than payment cadence ({frequency:?})")]
    TermShorterThanCadence {
        term_months: u32,
        frequency: FdPaymentFrequency,
    },
    #[error("invalid date arithmetic — {start} + {months} months overflows the calendar")]
    DateOverflow { start: NaiveDate, months: u32 },
}

/// Generate the schedule of INTEREST activities for a fixed deposit.
///
/// Uses simple interest by default — periodic interest =
/// `principal * (rate / 100) / payments_per_year`. Compound interest
/// across periods is intentionally out of scope until v3.4 (most retail
/// FDs in MENA / South Asia quote simple-interest yields).
///
/// At-maturity payouts compute total interest as
/// `principal * rate * (term_months / 12)` (proportional, supports any
/// term length).
///
/// Returns a `Vec<NewActivity>` ready to feed into `ActivityService::
/// create_activity`. The caller is responsible for the actual insert + the
/// fact that activities are inserted as a sequence (failures partway
/// through don't roll back — that's a service-layer concern).
pub fn generate_fd_schedule(params: &FdParams) -> Result<Vec<NewActivity>, FdSchedulerError> {
    if params.principal <= Decimal::ZERO {
        return Err(FdSchedulerError::NonPositivePrincipal(params.principal));
    }
    if params.annual_rate_percent < Decimal::ZERO {
        return Err(FdSchedulerError::NegativeRate(params.annual_rate_percent));
    }
    if params.term_months == 0 {
        return Err(FdSchedulerError::ZeroTerm);
    }

    let rate_decimal = params.annual_rate_percent / Decimal::from(100);

    if let Some(cadence_months) = params.payment_frequency.months_between() {
        if params.term_months < cadence_months {
            return Err(FdSchedulerError::TermShorterThanCadence {
                term_months: params.term_months,
                frequency: params.payment_frequency,
            });
        }
    }

    let activities = match params.payment_frequency {
        FdPaymentFrequency::AtMaturity => vec![at_maturity_activity(params, rate_decimal)?],
        _ => periodic_activities(params, rate_decimal)?,
    };

    Ok(activities)
}

fn at_maturity_activity(
    params: &FdParams,
    rate_decimal: Decimal,
) -> Result<NewActivity, FdSchedulerError> {
    let maturity_date = add_months(params.start_date, params.term_months)?;
    // total_interest = principal * rate * (term_months / 12)
    let term_years = Decimal::from(params.term_months) / Decimal::from(12);
    let total_interest = round_2dp(params.principal * rate_decimal * term_years);
    Ok(make_interest_activity(
        params,
        maturity_date,
        total_interest,
    ))
}

fn periodic_activities(
    params: &FdParams,
    rate_decimal: Decimal,
) -> Result<Vec<NewActivity>, FdSchedulerError> {
    let cadence_months = params
        .payment_frequency
        .months_between()
        .expect("non-AtMaturity frequencies have a cadence");
    let payments_per_year = params.payment_frequency.payments_per_year();
    let periodic_interest =
        round_2dp(params.principal * rate_decimal / Decimal::from(payments_per_year));

    let total_payments = params.term_months / cadence_months;
    let mut out = Vec::with_capacity(total_payments as usize);
    for n in 1..=total_payments {
        let payment_date = add_months(params.start_date, n * cadence_months)?;
        out.push(make_interest_activity(
            params,
            payment_date,
            periodic_interest,
        ));
    }
    Ok(out)
}

fn make_interest_activity(
    params: &FdParams,
    activity_date: NaiveDate,
    amount: Decimal,
) -> NewActivity {
    NewActivity {
        id: None,
        account_id: params.account_id.clone(),
        asset: None, // pure cash interest, no asset reference
        activity_type: ACTIVITY_TYPE_INTEREST.to_string(),
        subtype: None,
        activity_date: activity_date.to_string(),
        quantity: None,
        unit_price: None,
        currency: params.currency.clone(),
        fee: None,
        amount: Some(amount),
        status: None,
        notes: params.notes.clone(),
        fx_rate: None,
        metadata: Some(format!(
            r#"{{"source":"fd_schedule","principal":"{}","annual_rate_percent":"{}","term_months":{},"frequency":"{:?}","start_date":"{}"}}"#,
            params.principal,
            params.annual_rate_percent,
            params.term_months,
            params.payment_frequency,
            params.start_date
        )),
        needs_review: None,
        source_system: Some("FD_SCHEDULE".to_string()),
        source_record_id: None,
        source_group_id: None,
        idempotency_key: None,
    }
}

fn add_months(date: NaiveDate, months: u32) -> Result<NaiveDate, FdSchedulerError> {
    date.checked_add_months(Months::new(months))
        .ok_or(FdSchedulerError::DateOverflow {
            start: date,
            months,
        })
}

fn round_2dp(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn params(freq: FdPaymentFrequency, term: u32) -> FdParams {
        FdParams {
            account_id: "acc-test".to_string(),
            principal: dec!(10000),
            annual_rate_percent: dec!(5),
            term_months: term,
            payment_frequency: freq,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            currency: "USD".to_string(),
            notes: Some("HSBC USD FD @ 5%, 12 mo".to_string()),
        }
    }

    #[test]
    fn quarterly_12mo_emits_4_payments_of_125_each() {
        // 10000 principal × 5% / 4 = 125 per quarter
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::Quarterly, 12)).unwrap();
        assert_eq!(schedule.len(), 4);
        for activity in &schedule {
            assert_eq!(activity.activity_type, "INTEREST");
            assert_eq!(activity.amount, Some(dec!(125.00)));
            assert_eq!(activity.currency, "USD");
            assert_eq!(activity.account_id, "acc-test");
        }
        let dates: Vec<_> = schedule.iter().map(|a| a.activity_date.as_str()).collect();
        assert_eq!(
            dates,
            vec!["2026-04-15", "2026-07-15", "2026-10-15", "2027-01-15"]
        );
    }

    #[test]
    fn monthly_6mo_emits_6_payments() {
        // 10000 × 5% / 12 ≈ 41.67 per month
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::Monthly, 6)).unwrap();
        assert_eq!(schedule.len(), 6);
        let amount = schedule[0].amount.unwrap();
        assert_eq!(amount, dec!(41.67));
        // First payment one month after start
        assert_eq!(schedule[0].activity_date, "2026-02-15");
        assert_eq!(schedule[5].activity_date, "2026-07-15");
    }

    #[test]
    fn at_maturity_12mo_single_payment_of_500() {
        // 10000 × 5% × 1 year = 500
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::AtMaturity, 12)).unwrap();
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].amount, Some(dec!(500.00)));
        assert_eq!(schedule[0].activity_date, "2027-01-15");
    }

    #[test]
    fn at_maturity_6mo_pro_rata_payment_of_250() {
        // 10000 × 5% × 0.5 year = 250
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::AtMaturity, 6)).unwrap();
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].amount, Some(dec!(250.00)));
    }

    #[test]
    fn at_maturity_18mo_pro_rata_payment_of_750() {
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::AtMaturity, 18)).unwrap();
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].amount, Some(dec!(750.00)));
    }

    #[test]
    fn semi_annual_24mo_emits_4_payments_of_250() {
        // 10000 × 5% / 2 = 250 every 6 months
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::SemiAnnual, 24)).unwrap();
        assert_eq!(schedule.len(), 4);
        for activity in &schedule {
            assert_eq!(activity.amount, Some(dec!(250.00)));
        }
    }

    #[test]
    fn rejects_non_positive_principal() {
        let mut p = params(FdPaymentFrequency::Quarterly, 12);
        p.principal = dec!(0);
        assert!(matches!(
            generate_fd_schedule(&p),
            Err(FdSchedulerError::NonPositivePrincipal(_))
        ));
        p.principal = dec!(-100);
        assert!(matches!(
            generate_fd_schedule(&p),
            Err(FdSchedulerError::NonPositivePrincipal(_))
        ));
    }

    #[test]
    fn rejects_negative_rate() {
        let mut p = params(FdPaymentFrequency::Quarterly, 12);
        p.annual_rate_percent = dec!(-1);
        assert!(matches!(
            generate_fd_schedule(&p),
            Err(FdSchedulerError::NegativeRate(_))
        ));
    }

    #[test]
    fn allows_zero_rate_no_payments_emitted_with_zero_amount() {
        // Edge case — interest-free FD (e.g. demand deposit). Schedule still
        // emits zero-amount entries on the cadence so the user can visually
        // confirm the deposit is tracked.
        let mut p = params(FdPaymentFrequency::Quarterly, 12);
        p.annual_rate_percent = dec!(0);
        let schedule = generate_fd_schedule(&p).unwrap();
        assert_eq!(schedule.len(), 4);
        for a in &schedule {
            assert_eq!(a.amount, Some(dec!(0.00)));
        }
    }

    #[test]
    fn rejects_zero_term() {
        let mut p = params(FdPaymentFrequency::Quarterly, 12);
        p.term_months = 0;
        assert_eq!(
            generate_fd_schedule(&p).unwrap_err(),
            FdSchedulerError::ZeroTerm
        );
    }

    #[test]
    fn rejects_term_shorter_than_cadence() {
        // 2-month term with quarterly cadence → ambiguous, reject early.
        let p = params(FdPaymentFrequency::Quarterly, 2);
        assert!(matches!(
            generate_fd_schedule(&p).unwrap_err(),
            FdSchedulerError::TermShorterThanCadence { .. }
        ));
    }

    #[test]
    fn metadata_carries_traceability_back_to_fd() {
        let schedule = generate_fd_schedule(&params(FdPaymentFrequency::Annual, 12)).unwrap();
        let metadata = schedule[0].metadata.as_ref().expect("metadata set");
        assert!(metadata.contains("\"source\":\"fd_schedule\""));
        assert!(metadata.contains("\"principal\":\"10000\""));
        assert!(metadata.contains("\"annual_rate_percent\":\"5\""));
        assert_eq!(schedule[0].source_system.as_deref(), Some("FD_SCHEDULE"));
    }
}
