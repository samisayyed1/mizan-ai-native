//! Portfolio health scorer.
//!
//! Pure math entry [`score_health`] is unit-tested across canonical
//! shapes (perfect portfolio, concentrated, mostly cash, drifted).

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::BTreeMap;

use super::health_model::{HealthDriver, HealthInputs, HealthPosition, HealthReport};

const DEFAULT_NOTE: &str =
    "Health score is a heuristic, not financial advice. The four drivers (concentration, FX \
     exposure, cash drag, allocation drift) are equal-weighted; a low score means the portfolio \
     diverges from a diversified base-currency target — not that it's wrong.";

/// Cash target: any cash share at or below this fraction scores 100 on
/// the cash-drag driver; everything above degrades linearly.
const CASH_TARGET: Decimal = dec!(0.20);

const TWO_DP: u32 = 2;

/// Score a portfolio.
pub fn score_health(inputs: HealthInputs) -> HealthReport {
    let mut notes = vec![DEFAULT_NOTE.to_string()];

    let total: Decimal = inputs.positions.iter().map(|p| p.value_base).sum();
    if total <= Decimal::ZERO || inputs.positions.is_empty() {
        return HealthReport {
            score: None,
            drivers: Vec::new(),
            worst_driver: None,
            base_currency: inputs.base_currency,
            notes,
        };
    }

    let concentration = score_concentration(&inputs.positions, total);
    let fx = score_fx_exposure(&inputs.positions, total);
    let cash = score_cash_drag(&inputs.positions, total);
    let drift = score_drift(
        &inputs.positions,
        &inputs.target_allocation,
        total,
        &mut notes,
    );

    let drivers = vec![concentration, fx, cash, drift];

    let avg = drivers.iter().map(|d| d.score).sum::<Decimal>() / Decimal::from(drivers.len());
    let composite = avg.round_dp(TWO_DP);

    let worst = drivers.iter().min_by(|a, b| a.score.cmp(&b.score)).cloned();

    HealthReport {
        score: Some(composite),
        drivers,
        worst_driver: worst,
        base_currency: inputs.base_currency,
        notes,
    }
}

fn score_concentration(positions: &[HealthPosition], total: Decimal) -> HealthDriver {
    let top = positions
        .iter()
        .map(|p| p.value_base)
        .max()
        .unwrap_or(Decimal::ZERO);
    let share = top / total;
    let score = ((Decimal::ONE - share) * dec!(100))
        .round_dp(TWO_DP)
        .clamp(Decimal::ZERO, dec!(100));
    let pct = (share * dec!(100)).round_dp(1);
    HealthDriver {
        id: "concentration".into(),
        label: "Concentration".into(),
        score,
        metric: share.round_dp(4),
        note: format!("Top position is {pct}% of the portfolio."),
    }
}

fn score_fx_exposure(positions: &[HealthPosition], total: Decimal) -> HealthDriver {
    let foreign: Decimal = positions
        .iter()
        .filter(|p| p.is_foreign_currency)
        .map(|p| p.value_base)
        .sum();
    let share = foreign / total;
    let score = ((Decimal::ONE - share) * dec!(100))
        .round_dp(TWO_DP)
        .clamp(Decimal::ZERO, dec!(100));
    let pct = (share * dec!(100)).round_dp(1);
    HealthDriver {
        id: "fxExposure".into(),
        label: "FX exposure".into(),
        score,
        metric: share.round_dp(4),
        note: format!("{pct}% of the portfolio is in non-base currencies."),
    }
}

fn score_cash_drag(positions: &[HealthPosition], total: Decimal) -> HealthDriver {
    let cash: Decimal = positions
        .iter()
        .filter(|p| p.is_cash)
        .map(|p| p.value_base)
        .sum();
    let share = cash / total;

    // Below or at target → 100. Above target → linearly degrades to 0 at
    // 100% cash.
    let score = if share <= CASH_TARGET {
        dec!(100)
    } else {
        let excess = share - CASH_TARGET;
        let scale = Decimal::ONE - CASH_TARGET; // distance from target to 100%
        ((Decimal::ONE - excess / scale) * dec!(100))
            .round_dp(TWO_DP)
            .clamp(Decimal::ZERO, dec!(100))
    };
    let pct = (share * dec!(100)).round_dp(1);
    HealthDriver {
        id: "cashDrag".into(),
        label: "Cash drag".into(),
        score,
        metric: share.round_dp(4),
        note: format!(
            "{pct}% in cash (target ≤ {}%).",
            (CASH_TARGET * dec!(100)).round_dp(0)
        ),
    }
}

fn score_drift(
    positions: &[HealthPosition],
    target: &BTreeMap<String, Decimal>,
    total: Decimal,
    notes: &mut Vec<String>,
) -> HealthDriver {
    if target.is_empty() {
        notes.push("No target allocation configured — drift driver scored 100 by default.".into());
        return HealthDriver {
            id: "allocationDrift".into(),
            label: "Allocation drift".into(),
            score: dec!(100),
            metric: Decimal::ZERO,
            note: "No target allocation set — skipped.".into(),
        };
    }

    // Actual allocation by asset class.
    let mut actual: BTreeMap<String, Decimal> = BTreeMap::new();
    for p in positions {
        *actual.entry(p.asset_class.clone()).or_insert(Decimal::ZERO) += p.value_base;
    }

    // Union of target + actual keys, take absolute drift per class.
    let mut drift = Decimal::ZERO;
    let union: std::collections::BTreeSet<&String> = target.keys().chain(actual.keys()).collect();
    for cls in union {
        let actual_share = actual.get(cls).copied().unwrap_or(Decimal::ZERO) / total;
        let target_share = target.get(cls).copied().unwrap_or(Decimal::ZERO);
        drift += (actual_share - target_share).abs();
    }

    // Divide by 2 — every overweight equals an underweight elsewhere, so
    // the raw sum double-counts.
    let drift_pct = (drift / dec!(2)).clamp(Decimal::ZERO, Decimal::ONE);

    let score = ((Decimal::ONE - drift_pct) * dec!(100))
        .round_dp(TWO_DP)
        .clamp(Decimal::ZERO, dec!(100));
    let pct = (drift_pct * dec!(100)).round_dp(1);
    HealthDriver {
        id: "allocationDrift".into(),
        label: "Allocation drift".into(),
        score,
        metric: drift_pct.round_dp(4),
        note: format!("{pct}% absolute drift vs. target allocation."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(label: &str, asset_class: &str, value: Decimal, fx: bool, cash: bool) -> HealthPosition {
        HealthPosition {
            label: label.into(),
            asset_class: asset_class.into(),
            value_base: value,
            is_foreign_currency: fx,
            is_cash: cash,
        }
    }

    fn target(pairs: &[(&str, Decimal)]) -> TargetAllocation {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    #[allow(dead_code)]
    type TargetAllocation = BTreeMap<String, Decimal>;

    #[test]
    fn empty_portfolio_has_none_score() {
        let report = score_health(HealthInputs {
            positions: vec![],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        assert!(report.score.is_none());
        assert!(report.drivers.is_empty());
        assert!(report.worst_driver.is_none());
    }

    #[test]
    fn perfectly_balanced_portfolio_scores_near_100() {
        let report = score_health(HealthInputs {
            positions: vec![
                pos("VTI", "EQUITY", dec!(6000), false, false),
                pos("BND", "BOND", dec!(3000), false, false),
                pos("Cash", "CASH", dec!(1000), false, true),
            ],
            target_allocation: target(&[
                ("EQUITY", dec!(0.6)),
                ("BOND", dec!(0.3)),
                ("CASH", dec!(0.1)),
            ]),
            base_currency: Some("USD".into()),
        });
        let score = report.score.unwrap();
        // No concentration (top = 60%), no FX, cash at target, zero drift.
        assert!(score >= dec!(75), "expected >= 75, got {}", score);
    }

    #[test]
    fn single_position_concentration_drags_score() {
        let report = score_health(HealthInputs {
            positions: vec![pos("AAPL", "EQUITY", dec!(10000), false, false)],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        let concentration = report
            .drivers
            .iter()
            .find(|d| d.id == "concentration")
            .unwrap();
        assert_eq!(concentration.score, Decimal::ZERO);
        // Worst driver is concentration.
        assert_eq!(report.worst_driver.unwrap().id, "concentration");
    }

    #[test]
    fn all_foreign_currency_zeros_fx_driver() {
        let report = score_health(HealthInputs {
            positions: vec![
                pos("Toyota", "EQUITY", dec!(5000), true, false),
                pos("Samsung", "EQUITY", dec!(5000), true, false),
            ],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        let fx = report
            .drivers
            .iter()
            .find(|d| d.id == "fxExposure")
            .unwrap();
        assert_eq!(fx.score, Decimal::ZERO);
        assert_eq!(fx.metric, dec!(1.0000));
    }

    #[test]
    fn cash_at_or_below_target_scores_100() {
        let report = score_health(HealthInputs {
            positions: vec![
                pos("VTI", "EQUITY", dec!(8000), false, false),
                pos("Cash", "CASH", dec!(2000), false, true),
            ],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        let cash = report.drivers.iter().find(|d| d.id == "cashDrag").unwrap();
        assert_eq!(cash.score, dec!(100));
    }

    #[test]
    fn all_cash_zeros_cash_driver() {
        let report = score_health(HealthInputs {
            positions: vec![pos("Cash", "CASH", dec!(10000), false, true)],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        let cash = report.drivers.iter().find(|d| d.id == "cashDrag").unwrap();
        assert_eq!(cash.score, Decimal::ZERO);
    }

    #[test]
    fn drift_skipped_when_no_target_set() {
        let report = score_health(HealthInputs {
            positions: vec![pos("VTI", "EQUITY", dec!(1000), false, false)],
            target_allocation: BTreeMap::new(),
            base_currency: None,
        });
        let drift = report
            .drivers
            .iter()
            .find(|d| d.id == "allocationDrift")
            .unwrap();
        assert_eq!(drift.score, dec!(100));
        assert!(report.notes.iter().any(|n| n.contains("No target")));
    }

    #[test]
    fn fully_drifted_portfolio_zeros_drift_driver() {
        // 100% equity vs. 100% bond target → full drift.
        let report = score_health(HealthInputs {
            positions: vec![pos("VTI", "EQUITY", dec!(10000), false, false)],
            target_allocation: target(&[("BOND", dec!(1.0))]),
            base_currency: Some("USD".into()),
        });
        let drift = report
            .drivers
            .iter()
            .find(|d| d.id == "allocationDrift")
            .unwrap();
        assert_eq!(drift.score, Decimal::ZERO);
    }

    #[test]
    fn worst_driver_is_lowest_scoring() {
        // Concentrated, foreign-currency, all cash — concentration is 0,
        // FX is 0, cash is 0 (tie). Any of those is acceptable as worst.
        let report = score_health(HealthInputs {
            positions: vec![pos("Foreign cash", "CASH", dec!(10000), true, true)],
            target_allocation: BTreeMap::new(),
            base_currency: Some("USD".into()),
        });
        let worst = report.worst_driver.unwrap();
        assert_eq!(worst.score, Decimal::ZERO);
    }
}
