//! Liability amortization module — Pro liability-payoff report (M4.2).
//!
//! Given a fixed-rate amortising loan (mortgage, auto loan, student loan,
//! personal loan), compute the per-period principal/interest split, the
//! cumulative balance trajectory, and a projected payoff date.
//!
//! The math is the standard French amortization formula:
//!
//! ```text
//! monthly_rate = annual_rate / 12
//! EMI = principal * monthly_rate * (1 + monthly_rate)^n
//!                                / ((1 + monthly_rate)^n - 1)
//! ```
//!
//! where `n` is the number of monthly payments.
//!
//! Inputs come from the user's existing liability record (current balance,
//! interest rate, optional monthly payment override, origination date, loan
//! duration). The output is the schedule from "today forward" — historical
//! payments are not reconstructed because the actual payment history is not
//! stored.
//!
//! **Not financial advice.** Numbers are arithmetic projections under the
//! assumption of unchanged payment + rate; variable-rate loans, prepayment
//! penalties, and escrow are out of scope.

mod amortization_model;
mod amortization_service;

pub use amortization_model::*;
pub use amortization_service::*;
