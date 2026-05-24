//! Amortization schedule builder.
//!
//! Pure math entry [`build_schedule`] is unit-tested across canonical loan
//! shapes (zero-rate, standard mortgage, over-paid, single-period).

use chrono::{Datelike, NaiveDate};
use rust_decimal::{Decimal, MathematicalOps};
use rust_decimal_macros::dec;

use super::amortization_model::{AmortizationInputs, AmortizationReport, Installment};

/// Hard cap so a degenerate input (very small payment, very high rate)
/// can't spin a multi-thousand-row schedule. 50 years is longer than any
/// real consumer loan.
const MAX_SCHEDULE_MONTHS: u32 = 600;

const TWO_DP: u32 = 2;

const DEFAULT_NOTE: &str =
    "Schedule assumes the current rate and payment stay fixed for the life of the loan. \
     Variable-rate, prepayment penalties, and escrow are out of scope.";

/// Build the amortization schedule from the inputs.
///
/// Returns an empty schedule and zero totals when `current_balance` ≤ 0.
/// When `monthly_payment` is omitted, an EMI is derived from the balance,
/// rate, and remaining term (defaulting to 360 months if no term given).
pub fn build_schedule(inputs: AmortizationInputs) -> AmortizationReport {
    let mut notes = vec![DEFAULT_NOTE.to_string()];

    // Trivial: already paid off.
    if inputs.current_balance <= Decimal::ZERO {
        return AmortizationReport {
            monthly_payment: Decimal::ZERO,
            total_paid: Decimal::ZERO,
            total_interest: Decimal::ZERO,
            total_principal: Decimal::ZERO,
            payoff_date: inputs.start_date,
            schedule: Vec::new(),
            currency: inputs.currency,
            notes,
        };
    }

    let monthly_rate = inputs.annual_rate / dec!(12);
    let term_months = inputs
        .remaining_months
        .unwrap_or(360)
        .min(MAX_SCHEDULE_MONTHS);

    let payment = match inputs.monthly_payment {
        Some(p) if p > Decimal::ZERO => p,
        _ => derive_emi(
            inputs.current_balance,
            monthly_rate,
            term_months,
            &mut notes,
        ),
    };

    // Walk the schedule forward.
    let mut balance = inputs.current_balance;
    let mut schedule = Vec::with_capacity(term_months as usize);
    let mut total_interest = Decimal::ZERO;
    let mut total_principal = Decimal::ZERO;
    let mut total_paid = Decimal::ZERO;
    let mut due_date = inputs.start_date;

    for period in 1..=MAX_SCHEDULE_MONTHS {
        if balance <= Decimal::ZERO {
            break;
        }

        let interest = (balance * monthly_rate).round_dp(TWO_DP).max(Decimal::ZERO);
        let mut principal = (payment - interest).round_dp(TWO_DP);
        let mut this_payment = payment;

        // Final installment: if remaining balance is less than the
        // computed principal, pay just what's owed.
        if principal >= balance {
            principal = balance;
            this_payment = (principal + interest).round_dp(TWO_DP);
        }

        let new_balance = (balance - principal).round_dp(TWO_DP).max(Decimal::ZERO);

        schedule.push(Installment {
            period,
            due_date,
            payment: this_payment,
            interest,
            principal,
            balance_after: new_balance,
        });

        total_interest += interest;
        total_principal += principal;
        total_paid += this_payment;
        balance = new_balance;

        due_date = add_one_month(due_date);

        if balance <= Decimal::ZERO {
            break;
        }
    }

    // If the loop bailed at the cap with balance still > 0 (payment too
    // small to amortise), surface it.
    if balance > Decimal::ZERO {
        notes.push(format!(
            "Payment is too small to retire the balance within {} months — schedule capped. \
             Increase the monthly payment or extend the term.",
            MAX_SCHEDULE_MONTHS
        ));
    }

    let payoff_date = schedule
        .last()
        .map(|i| i.due_date)
        .unwrap_or(inputs.start_date);

    AmortizationReport {
        monthly_payment: payment,
        total_paid: total_paid.round_dp(TWO_DP),
        total_interest: total_interest.round_dp(TWO_DP),
        total_principal: total_principal.round_dp(TWO_DP),
        payoff_date,
        schedule,
        currency: inputs.currency,
        notes,
    }
}

/// Standard EMI formula. Falls back to flat principal/term when the rate
/// is zero or negative.
fn derive_emi(
    principal: Decimal,
    monthly_rate: Decimal,
    term: u32,
    notes: &mut Vec<String>,
) -> Decimal {
    if term == 0 {
        notes.push("Term of zero months — assumed lump-sum payoff today.".into());
        return principal;
    }
    if monthly_rate <= Decimal::ZERO {
        // Zero-interest loan: flat principal-only payments.
        return (principal / Decimal::from(term)).round_dp(TWO_DP);
    }

    let n = term as i64;
    // (1 + r)^n via Decimal's powi.
    let one_plus_r = Decimal::ONE + monthly_rate;
    let pow = one_plus_r.powi(n);

    let numerator = principal * monthly_rate * pow;
    let denominator = pow - Decimal::ONE;
    if denominator == Decimal::ZERO {
        notes.push("Degenerate rate/term combination — used flat amortization.".into());
        return (principal / Decimal::from(term)).round_dp(TWO_DP);
    }
    (numerator / denominator).round_dp(TWO_DP)
}

/// Add one calendar month, clamping the day if the target month is shorter
/// (Jan 31 → Feb 28/29, etc.).
fn add_one_month(d: NaiveDate) -> NaiveDate {
    let (mut y, mut m) = (d.year(), d.month());
    m += 1;
    if m == 13 {
        m = 1;
        y += 1;
    }
    // Clamp the day to the last day of the target month.
    let target_day = d.day().min(days_in_month(y, m));
    NaiveDate::from_ymd_opt(y, m, target_day).expect("clamped date must be valid")
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => unreachable!("month must be 1..=12"),
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn inputs(
        balance: Decimal,
        annual_rate: Decimal,
        payment: Option<Decimal>,
        term: Option<u32>,
        start: NaiveDate,
    ) -> AmortizationInputs {
        AmortizationInputs {
            current_balance: balance,
            annual_rate,
            monthly_payment: payment,
            remaining_months: term,
            start_date: start,
            currency: Some("USD".into()),
        }
    }

    #[test]
    fn zero_balance_returns_empty_schedule() {
        let report = build_schedule(inputs(
            Decimal::ZERO,
            dec!(0.05),
            None,
            Some(12),
            d("2026-01-01"),
        ));
        assert!(report.schedule.is_empty());
        assert_eq!(report.monthly_payment, Decimal::ZERO);
        assert_eq!(report.total_interest, Decimal::ZERO);
    }

    #[test]
    fn zero_rate_flat_principal_only_schedule() {
        // $1200 / 12 months at 0% APR → $100/mo flat, $0 interest.
        let report = build_schedule(inputs(
            dec!(1200),
            Decimal::ZERO,
            None,
            Some(12),
            d("2026-01-01"),
        ));
        assert_eq!(report.schedule.len(), 12);
        assert_eq!(report.monthly_payment, dec!(100));
        assert_eq!(report.total_interest, Decimal::ZERO);
        assert_eq!(report.total_principal, dec!(1200));
        assert_eq!(report.payoff_date, d("2026-12-01"));
        assert_eq!(report.schedule.last().unwrap().balance_after, Decimal::ZERO);
    }

    #[test]
    fn standard_mortgage_30_year_at_6pct() {
        // $300k @ 6% APR for 360 months → EMI ~$1798.65
        let report = build_schedule(inputs(
            dec!(300000),
            dec!(0.06),
            None,
            Some(360),
            d("2026-01-15"),
        ));
        // ±1 month tolerance: 2dp rounding on the EMI may leave a sub-dollar
        // stub balance that needs one extra installment.
        assert!(
            (359..=361).contains(&report.schedule.len()),
            "expected ~360 installments, got {}",
            report.schedule.len()
        );
        // EMI is within $1 of the textbook figure.
        let diff = (report.monthly_payment - dec!(1798.65)).abs();
        assert!(
            diff < dec!(1.0),
            "EMI {} drifted too far",
            report.monthly_payment
        );
        assert!(report.total_interest > dec!(300000));
        assert_eq!(report.schedule.last().unwrap().balance_after, Decimal::ZERO);
        // First payment is overwhelmingly interest.
        assert!(report.schedule[0].interest > report.schedule[0].principal);
        // Final payment is overwhelmingly principal.
        let last = report.schedule.last().unwrap();
        assert!(last.principal > last.interest);
    }

    #[test]
    fn explicit_monthly_payment_overrides_emi_derivation() {
        // User pays $500/mo on a $10k balance at 5% APR.
        let report = build_schedule(inputs(
            dec!(10000),
            dec!(0.05),
            Some(dec!(500)),
            None,
            d("2026-01-01"),
        ));
        assert_eq!(report.monthly_payment, dec!(500));
        // Schedule terminates with zero balance.
        assert_eq!(report.schedule.last().unwrap().balance_after, Decimal::ZERO);
        // Final installment is a stub (<$500) because the remaining
        // principal is smaller.
        let final_payment = report.schedule.last().unwrap().payment;
        assert!(final_payment <= dec!(500));
    }

    #[test]
    fn payment_too_small_caps_schedule_and_notes() {
        // $1/mo on a $10k balance at 10% APR — never amortises, must cap.
        let report = build_schedule(inputs(
            dec!(10000),
            dec!(0.10),
            Some(dec!(1)),
            None,
            d("2026-01-01"),
        ));
        assert_eq!(report.schedule.len() as u32, MAX_SCHEDULE_MONTHS);
        assert!(report.notes.iter().any(|n| n.contains("capped")));
    }

    #[test]
    fn single_month_payoff() {
        // Balance < monthly payment: schedule is exactly one installment.
        let report = build_schedule(inputs(
            dec!(500),
            dec!(0.06),
            Some(dec!(600)),
            None,
            d("2026-01-15"),
        ));
        assert_eq!(report.schedule.len(), 1);
        let only = &report.schedule[0];
        // Interest = 500 * 0.06/12 = 2.50
        assert_eq!(only.interest, dec!(2.50));
        assert_eq!(only.principal, dec!(500));
        assert_eq!(only.payment, dec!(502.50));
        assert_eq!(only.balance_after, Decimal::ZERO);
    }

    #[test]
    fn month_rollover_handles_leap_year() {
        // Start Jan 31 2024 (leap year) → Feb 29 → Mar 29.
        let report = build_schedule(inputs(
            dec!(3000),
            Decimal::ZERO,
            Some(dec!(1000)),
            Some(3),
            d("2024-01-31"),
        ));
        assert_eq!(report.schedule[0].due_date, d("2024-01-31"));
        assert_eq!(report.schedule[1].due_date, d("2024-02-29"));
        assert_eq!(report.schedule[2].due_date, d("2024-03-29"));
    }

    #[test]
    fn month_rollover_handles_non_leap_year() {
        // Start Jan 31 2026 (non-leap) → Feb 28 → Mar 28.
        let report = build_schedule(inputs(
            dec!(3000),
            Decimal::ZERO,
            Some(dec!(1000)),
            Some(3),
            d("2026-01-31"),
        ));
        assert_eq!(report.schedule[1].due_date, d("2026-02-28"));
    }

    #[test]
    fn total_principal_equals_starting_balance() {
        let report = build_schedule(inputs(
            dec!(50000),
            dec!(0.07),
            None,
            Some(60),
            d("2026-01-01"),
        ));
        // Within $1 of original balance (rounding).
        let diff = (report.total_principal - dec!(50000)).abs();
        assert!(
            diff < dec!(1.0),
            "principal sum drifted: {}",
            report.total_principal
        );
    }
}
