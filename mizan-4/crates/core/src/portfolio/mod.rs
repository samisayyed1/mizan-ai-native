pub mod allocation;
pub mod amortization;
pub mod fire;
pub mod health;
pub mod holdings;
pub mod income;
pub mod net_worth;
pub mod performance;
pub mod snapshot;
pub mod split_adjustment;
pub mod synthesis;
pub mod valuation;
// Track H PR-H3.b — zakat moved out of mizan-core into the
// `mizan-zakat` crate. Consumers import directly:
//   use mizan_zakat::{ZakatService, ZakatServiceTrait, ZakatInputs, ZakatReport};
