//! Zakat (zakāh) calculation module — Pro headline feature (M3.7).
//!
//! Computes the Shariah-mandated 2.5% annual alms on a Muslim's wealth, per
//! the formula the manual specifies (and standard Hanafi/majority practice):
//!
//!   net_base = liquid_cash + precious_metals_value + tradable_assets
//!              - short_term_debts
//!   zakat_due = if net_base < nisab then 0 else 0.025 * net_base
//!
//! `nisab` is the threshold below which no Zakat is owed; the caller supplies
//! it in the user's base currency (typically the local equivalent of either
//! 85 grams of gold or 595 grams of silver, whichever is lower — the user or
//! their imam picks which standard to use).
//!
//! **Not financial advice.** This module reports an arithmetic result; the
//! user is responsible for what counts as Zakatable in their own
//! jurisprudence (Hanafi vs. Shafi'i vs. Maliki etc. differ on edge cases
//! like business inventory, agricultural produce, jewelry, etc.).

mod zakat_model;
mod zakat_service;
mod zakat_traits;

pub use zakat_model::*;
pub use zakat_service::*;
pub use zakat_traits::*;
