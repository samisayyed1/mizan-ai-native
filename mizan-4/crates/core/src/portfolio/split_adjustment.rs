//! Shared stock-split adjustment math — the single source of truth used by
//! BOTH sides of valuation so the share count and the price-per-share always
//! agree:
//!
//!  - the **quantity / cost-basis** side, where the snapshot calculator
//!    normalises every historical activity into current-share terms
//!    (`snapshot_service::adjust_activities_for_splits`), and
//!  - the **price** side, where the valuation read path divides historical
//!    closes by the same cumulative factor so a pre-split close is expressed
//!    in the same current-share basis the snapshot positions already use.
//!
//! Both derive from the same canonical `Split` activities (the immutable
//! ledger record; ratio carried in `Activity::amount`). Nothing here mutates
//! a stored quote or activity — the adjustment is computed at read time,
//! which keeps it deterministic and reconcilable: the same activities + the
//! same raw quotes always yield the same adjusted series.
//!
//! ## Why a price adjustment is needed at all
//!
//! `snapshot_service` rewrites a Buy of 10 shares @ $500 made before a 4:1
//! split into 40 shares @ $125, so every snapshot — including ones dated
//! *before* the split — reports the position in post-split (current) shares.
//! If such a position is then valued with the **raw** pre-split close ($500),
//! the result is `40 × $500 = $20,000` against a true `$5,000` — inflated by
//! the cumulative split ratio. Dividing the close by that same ratio
//! (`$500 / 4 = $125`) restores agreement: `40 × $125 = $5,000`.

use crate::activities::{Activity, ACTIVITY_TYPE_SPLIT};
use chrono::{DateTime, NaiveDate, Utc};
use log::warn;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Per-asset split events: `asset_id -> [(event_date, ratio)]`, sorted
/// ascending by date and de-duplicated by date. Ratio convention matches the
/// rest of the engine: `4.0` = 4:1 forward split, `0.2` = 1:5 reverse split.
pub type SplitFactors = HashMap<String, Vec<(NaiveDate, Decimal)>>;

/// Collect per-asset split events from `Split` activities whose (user-tz) date
/// falls within `[start, end]`.
///
/// `to_user_date` converts an activity's UTC instant to the user's local
/// calendar date; passing the *same* converter the quantity side uses keeps
/// the two adjustments aligned to the same calendar day.
///
/// A split is an asset-level event but is recorded per-account, so when the
/// same asset is held in multiple accounts the same split can appear several
/// times — the per-date de-duplication prevents over-applying it.
pub fn collect_split_factors(
    activities: &[Activity],
    to_user_date: impl Fn(DateTime<Utc>) -> NaiveDate,
    start: NaiveDate,
    end: NaiveDate,
) -> SplitFactors {
    let mut factors: SplitFactors = HashMap::new();
    for activity in activities.iter().filter(|a| {
        a.activity_type == ACTIVITY_TYPE_SPLIT
            && to_user_date(a.activity_date) >= start
            && to_user_date(a.activity_date) <= end
    }) {
        let Some(asset_id) = activity.asset_id.as_ref() else {
            warn!(
                "Missing asset_id for Split activity {} on {}. Ignoring split.",
                activity.id, activity.activity_date
            );
            continue;
        };
        match activity.amount {
            Some(ratio) if ratio.is_sign_positive() && !ratio.is_zero() => {
                factors
                    .entry(asset_id.clone())
                    .or_default()
                    .push((to_user_date(activity.activity_date), ratio));
            }
            Some(ratio) => warn!(
                "Invalid split ratio {} for Split activity {} on {}. Ignoring split.",
                ratio, activity.id, activity.activity_date
            ),
            None => warn!(
                "Missing amount for Split activity {} for asset {} on {}. Ignoring split.",
                activity.id, asset_id, activity.activity_date
            ),
        }
    }
    for splits in factors.values_mut() {
        splits.sort_by_key(|k| k.0);
        splits.dedup_by_key(|k| k.0);
    }
    factors
}

/// Cumulative split factor for an event occurring on `as_of`: the product of
/// every split ratio dated **strictly after** `as_of`.
///
/// Multiplying a historical quantity by this brings it into the current-share
/// basis; dividing a historical price by it does the same. Returns
/// [`Decimal::ONE`] when there are no later splits (the common case — any date
/// after the most recent split, including today).
pub fn cumulative_factor_after(splits: &[(NaiveDate, Decimal)], as_of: NaiveDate) -> Decimal {
    let mut factor = Decimal::ONE;
    for (date, ratio) in splits {
        if *date > as_of {
            factor *= ratio;
        }
    }
    factor
}

/// Split-adjust a raw closing price into the current-share basis:
/// `adjusted = raw / Π(ratio for splits dated > date)`.
///
/// `splits` is the per-asset list from [`collect_split_factors`] (already
/// sorted/deduped); pass an empty slice for an asset with no splits and the
/// raw close is returned unchanged.
pub fn split_adjusted_close(
    raw_close: Decimal,
    splits: &[(NaiveDate, Decimal)],
    date: NaiveDate,
) -> Decimal {
    let factor = cumulative_factor_after(splits, date);
    // A zero factor would only arise from a corrupt zero-ratio split, which
    // `collect_split_factors` already filters out; guard anyway so we never
    // divide by zero and silently produce an infinity/NaN-equivalent.
    if factor.is_zero() || factor == Decimal::ONE {
        raw_close
    } else {
        raw_close / factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn no_splits_is_identity() {
        let splits: Vec<(NaiveDate, Decimal)> = vec![];
        assert_eq!(cumulative_factor_after(&splits, d(2020, 1, 1)), dec!(1));
        assert_eq!(
            split_adjusted_close(dec!(123.45), &splits, d(2020, 1, 1)),
            dec!(123.45)
        );
    }

    #[test]
    fn factor_counts_only_strictly_later_splits() {
        // A single 4:1 split on 2020-08-31.
        let splits = vec![(d(2020, 8, 31), dec!(4))];
        // Before the split → divide by 4.
        assert_eq!(cumulative_factor_after(&splits, d(2019, 1, 1)), dec!(4));
        // Exactly on the split day → split is not "after", factor 1.
        assert_eq!(cumulative_factor_after(&splits, d(2020, 8, 31)), dec!(1));
        // After the split → factor 1 (already current basis).
        assert_eq!(cumulative_factor_after(&splits, d(2021, 1, 1)), dec!(1));
    }

    #[test]
    fn pre_split_close_is_divided_into_current_basis() {
        let splits = vec![(d(2020, 8, 31), dec!(4))];
        // $500 raw close before a 4:1 split == $125 in current shares.
        assert_eq!(
            split_adjusted_close(dec!(500), &splits, d(2019, 1, 1)),
            dec!(125)
        );
        // On/after the split the close is already current-basis.
        assert_eq!(
            split_adjusted_close(dec!(125), &splits, d(2021, 1, 1)),
            dec!(125)
        );
    }

    #[test]
    fn multiple_splits_compound() {
        // 2:1 then 3:1; a date before both is divided by 6.
        let splits = vec![(d(2018, 6, 1), dec!(2)), (d(2020, 6, 1), dec!(3))];
        assert_eq!(cumulative_factor_after(&splits, d(2017, 1, 1)), dec!(6));
        // Between the two splits → only the 3:1 is later.
        assert_eq!(cumulative_factor_after(&splits, d(2019, 1, 1)), dec!(3));
    }

    #[test]
    fn reverse_split_inflates_pre_split_close() {
        // 1:5 reverse split (ratio 0.2): pre-split $10 close → $50 current.
        let splits = vec![(d(2022, 1, 10), dec!(0.2))];
        assert_eq!(
            split_adjusted_close(dec!(10), &splits, d(2021, 1, 1)),
            dec!(50)
        );
    }
}
