//! Amortization domain types.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Inputs to an amortization schedule. All money amounts in the same
/// currency — the service does not convert FX.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmortizationInputs {
    /// Current outstanding principal balance.
    pub current_balance: Decimal,
    /// Annual interest rate as a decimal (e.g. 0.0625 for 6.25%). Negative
    /// or zero rates are accepted — zero produces a flat principal-only
    /// schedule.
    pub annual_rate: Decimal,
    /// Optional monthly payment (EMI). When `None`, the service derives the
    /// EMI from `current_balance` + `annual_rate` + `remaining_months`. When
    /// supplied, the schedule honors the user's actual payment which may
    /// differ slightly from the theoretical EMI.
    #[serde(default)]
    pub monthly_payment: Option<Decimal>,
    /// Remaining number of monthly payments. When `None`, the service caps
    /// the projection at 600 months (50 years) so the schedule terminates
    /// even for over-paid edge cases.
    #[serde(default)]
    pub remaining_months: Option<u32>,
    /// Date from which the schedule starts (typically today). Each
    /// installment is dated one month after the prior.
    pub start_date: NaiveDate,
    /// Currency code for display only; the math is unit-free.
    #[serde(default)]
    pub currency: Option<String>,
}

/// One monthly installment in the schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installment {
    /// 1-indexed sequence number (1, 2, 3...).
    pub period: u32,
    /// Date this payment falls due.
    pub due_date: NaiveDate,
    /// Total payment for this period (principal + interest). For the final
    /// installment this may be smaller than the EMI when only a stub
    /// balance remains.
    pub payment: Decimal,
    /// Portion of `payment` applied to interest.
    pub interest: Decimal,
    /// Portion of `payment` applied to principal.
    pub principal: Decimal,
    /// Remaining principal balance after this payment. Always ≥ 0; the
    /// final installment lands at exactly 0.
    pub balance_after: Decimal,
}

/// Aggregate output of an amortization assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmortizationReport {
    /// Effective monthly payment used to build the schedule.
    pub monthly_payment: Decimal,
    /// Total amount the user will pay across the full schedule.
    pub total_paid: Decimal,
    /// Sum of all interest payments — the cost of carrying the loan.
    pub total_interest: Decimal,
    /// Sum of all principal payments (equals `current_balance` ± rounding).
    pub total_principal: Decimal,
    /// Date of the final installment.
    pub payoff_date: NaiveDate,
    /// Per-period schedule, oldest first.
    pub schedule: Vec<Installment>,
    /// Currency code copied from the inputs.
    #[serde(default)]
    pub currency: Option<String>,
    /// Disclaimer + assumption notes for the UI.
    pub notes: Vec<String>,
}
