//! Service for computing portfolio allocations by taxonomy.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use log::debug;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Round a list of `(key, value)` shares into 2dp percentages that
/// sum to exactly 100.00 via the Largest-Remainder Method (LRM,
/// a.k.a. Hare-Niemeyer).
///
/// Why this is necessary: naïve independent rounding
/// `(value / total * 100).round_dp(2)` per row drifts the sum away
/// from 100. Three equal-thirds round to 33.33 each → sum = 99.99.
/// Five-asset portfolios with uneven values commonly drift by ±0.02
/// to ±0.05. The UI then shows pie-chart legends adding to 99.97% or
/// 100.04%, which looks wrong + breaks any allocation-drift threshold
/// math the rule engine downstream relies on.
///
/// LRM steps:
/// 1. Compute exact percentage `value / total * 100` per row.
/// 2. Take floor at 2dp; sum the floors.
/// 3. Distribute the deficit (`100 - sum_of_floors`) by giving +0.01
///    to the rows with the largest fractional remainder, one at a
///    time, until sum = 100.00.
///
/// Returns a HashMap keyed by the input key (typically a category_id)
/// for O(1) lookup at the caller. Empty input (or total ≤ 0) returns
/// an empty map; the caller falls back to 0% per row.
pub(super) fn largest_remainder_percentages<K: std::hash::Hash + Eq + Clone>(
    values: &[(K, Decimal)],
    total: Decimal,
) -> HashMap<K, Decimal> {
    if total <= Decimal::ZERO || values.is_empty() {
        return HashMap::new();
    }
    // 1) exact percentages, 2) floor each at 2dp, 3) track the
    // fractional remainder for the distribution pass.
    let mut working: Vec<(K, Decimal, Decimal)> = values
        .iter()
        .map(|(k, v)| {
            let exact = (*v / total) * dec!(100);
            let floored =
                exact.trunc_with_scale(2);
            let remainder = exact - floored;
            (k.clone(), floored, remainder)
        })
        .collect();
    let sum_floors: Decimal = working.iter().map(|(_, f, _)| *f).sum();
    let mut deficit_cents = ((dec!(100) - sum_floors) * dec!(100))
        .round_dp(0)
        .to_string()
        .parse::<i64>()
        .unwrap_or(0);

    if deficit_cents > 0 {
        // Sort by remainder desc, break ties on largest floored value
        // (= largest absolute share) so the +0.01 lands on the row
        // where it's least visible.
        let mut idx: Vec<usize> = (0..working.len()).collect();
        idx.sort_by(|&a, &b| {
            working[b]
                .2
                .cmp(&working[a].2)
                .then_with(|| working[b].1.cmp(&working[a].1))
        });
        for i in idx {
            if deficit_cents <= 0 {
                break;
            }
            working[i].1 += dec!(0.01);
            deficit_cents -= 1;
        }
    }
    working
        .into_iter()
        .map(|(k, pct, _)| (k, pct))
        .collect()
}

use crate::errors::Result;
use crate::portfolio::holdings::{Holding, HoldingSummary, HoldingType, HoldingsServiceTrait};
use crate::taxonomies::{Category, TaxonomyServiceTrait};

use super::{AllocationHoldings, CategoryAllocation, PortfolioAllocations, TaxonomyAllocation};

/// Trait for allocation service.
#[async_trait]
pub trait AllocationServiceTrait: Send + Sync {
    /// Computes portfolio allocations for an account.
    /// If account_id is "PORTFOLIO", computes for all accounts.
    async fn get_portfolio_allocations(
        &self,
        account_id: &str,
        base_currency: &str,
    ) -> Result<PortfolioAllocations>;

    /// Returns holdings filtered by a taxonomy category with full category metadata.
    /// Used for drill-down views when user clicks on an allocation category.
    async fn get_holdings_by_allocation(
        &self,
        account_id: &str,
        base_currency: &str,
        taxonomy_id: &str,
        category_id: &str,
    ) -> Result<AllocationHoldings>;
}

/// Service for computing taxonomy-based portfolio allocations.
pub struct AllocationService {
    holdings_service: Arc<dyn HoldingsServiceTrait>,
    taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
}

impl AllocationService {
    pub fn new(
        holdings_service: Arc<dyn HoldingsServiceTrait>,
        taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
    ) -> Self {
        Self {
            holdings_service,
            taxonomy_service,
        }
    }

    /// Aggregates holdings into a taxonomy allocation.
    /// For hierarchical taxonomies (GICS, Regions), rolls up to top-level categories
    /// and populates children for drill-down.
    #[allow(clippy::too_many_arguments)]
    fn aggregate_by_taxonomy(
        &self,
        holdings: &[Holding],
        taxonomy_id: &str,
        taxonomy_name: &str,
        taxonomy_color: &str,
        categories: &[Category],
        assignments_by_asset: &HashMap<String, Vec<(String, String, i32)>>, // asset_id -> [(taxonomy_id, category_id, weight)]
        total_value: Decimal,
        rollup_to_top_level: bool,
    ) -> TaxonomyAllocation {
        // Build category lookup maps
        let category_by_id: HashMap<&str, &Category> =
            categories.iter().map(|c| (c.id.as_str(), c)).collect();

        // For rollup: map child categories to their top-level ancestor
        let top_level_map: HashMap<&str, &str> = if rollup_to_top_level {
            self.build_top_level_map(categories)
        } else {
            // Identity map - each category maps to itself
            categories
                .iter()
                .map(|c| (c.id.as_str(), c.id.as_str()))
                .collect()
        };

        // Aggregate values by category (original assignments, not rolled up)
        // Key: original category_id, Value: (value, top_level_id)
        let mut original_values: HashMap<String, (Decimal, String)> = HashMap::new();
        // Aggregate values by top-level category (rolled up)
        let mut rolled_up_values: HashMap<String, Decimal> = HashMap::new();

        for holding in holdings {
            // Skip cash holdings for sector/region allocation (not for asset_classes)
            // Cash has asset_class classifications but not sector/region classifications
            if holding.holding_type == HoldingType::Cash && taxonomy_id != "asset_classes" {
                continue;
            }

            let asset_id = match &holding.instrument {
                Some(instrument) => &instrument.id,
                None => continue,
            };

            let market_value = holding.market_value.base;

            // Cash holdings have synthetic IDs (no DB record / no taxonomy assignments).
            // Assign them directly to CASH_BANK_DEPOSITS for the asset_classes taxonomy.
            if holding.holding_type == HoldingType::Cash && taxonomy_id == "asset_classes" {
                let cash_category = "CASH_BANK_DEPOSITS";
                let top_level_id = if rollup_to_top_level {
                    top_level_map
                        .get(cash_category)
                        .copied()
                        .unwrap_or(cash_category)
                } else {
                    cash_category
                };

                let entry = original_values
                    .entry(cash_category.to_string())
                    .or_insert((Decimal::ZERO, top_level_id.to_string()));
                entry.0 += market_value;

                *rolled_up_values
                    .entry(top_level_id.to_string())
                    .or_insert(Decimal::ZERO) += market_value;
                continue;
            }

            // Get assignments for this asset and taxonomy
            if let Some(asset_assignments) = assignments_by_asset.get(asset_id) {
                let taxonomy_assignments: Vec<_> = asset_assignments
                    .iter()
                    .filter(|(tid, _, _)| tid == taxonomy_id)
                    .collect();

                if taxonomy_assignments.is_empty() {
                    // No assignment for this taxonomy - count as "Unknown"
                    *rolled_up_values
                        .entry("__UNKNOWN__".to_string())
                        .or_insert(Decimal::ZERO) += market_value;
                } else {
                    for (_, category_id, weight) in taxonomy_assignments {
                        // Convert weight from basis points (0-10000) to decimal (0-1)
                        let weight_decimal = Decimal::from(*weight) / dec!(10000);
                        let weighted_value = market_value * weight_decimal;

                        // Get top-level category
                        let top_level_id = if rollup_to_top_level {
                            top_level_map
                                .get(category_id.as_str())
                                .copied()
                                .unwrap_or(category_id.as_str())
                        } else {
                            category_id.as_str()
                        };

                        // Track original category values (for children)
                        let entry = original_values
                            .entry(category_id.clone())
                            .or_insert((Decimal::ZERO, top_level_id.to_string()));
                        entry.0 += weighted_value;

                        // Track rolled-up values
                        *rolled_up_values
                            .entry(top_level_id.to_string())
                            .or_insert(Decimal::ZERO) += weighted_value;
                    }
                }
            } else {
                // No assignments at all - count as "Unknown"
                *rolled_up_values
                    .entry("__UNKNOWN__".to_string())
                    .or_insert(Decimal::ZERO) += market_value;
            }
        }

        // Pre-compute LRM-rounded percentages for top-level + per-parent
        // children so the rendered breakdown adds to exactly 100.00%.
        let top_level_pairs: Vec<(String, Decimal)> = rolled_up_values
            .iter()
            .filter(|(_, v)| **v > Decimal::ZERO)
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let top_level_percent_map =
            largest_remainder_percentages(&top_level_pairs, total_value);

        // Children are per-parent; compute LRM-rounded percentages
        // PER top-level scope (so each pie slice's children sum to
        // the slice's own percentage * 100). Because each child %
        // is computed against `total_value` (not the parent's value),
        // their sum across the whole portfolio still adds to 100.00.
        let children_pairs: Vec<(String, Decimal)> = original_values
            .iter()
            .filter(|(cid, (v, top_id))| **cid != *top_id && *v > Decimal::ZERO)
            .map(|(cid, (v, _))| (cid.clone(), *v))
            .collect();
        let children_percent_map =
            largest_remainder_percentages(&children_pairs, total_value);

        // Build children map: top_level_id -> Vec<CategoryAllocation>
        let mut children_map: HashMap<String, Vec<CategoryAllocation>> = HashMap::new();
        if rollup_to_top_level {
            for (cat_id, (value, top_level_id)) in &original_values {
                // Only add as child if different from top-level (i.e., it was rolled up)
                if cat_id != top_level_id && *value > Decimal::ZERO {
                    let (name, color) = category_by_id
                        .get(cat_id.as_str())
                        .map(|c| (c.name.clone(), c.color.clone()))
                        .unwrap_or_else(|| (cat_id.clone(), "#808080".to_string()));

                    let percentage = children_percent_map
                        .get(cat_id)
                        .copied()
                        .unwrap_or(Decimal::ZERO);

                    children_map.entry(top_level_id.clone()).or_default().push(
                        CategoryAllocation {
                            category_id: cat_id.clone(),
                            category_name: name,
                            color,
                            value: *value,
                            percentage,
                            children: Vec::new(),
                        },
                    );
                }
            }
            // Sort children by value descending
            for children in children_map.values_mut() {
                children.sort_by_key(|b| std::cmp::Reverse(b.value));
            }
        }

        // Build top-level category allocations
        let mut allocations: Vec<CategoryAllocation> = rolled_up_values
            .into_iter()
            .filter(|(_, value)| *value > Decimal::ZERO)
            .map(|(cat_id, value)| {
                let (name, color) = if cat_id == "__UNKNOWN__" {
                    ("Unknown".to_string(), "#878580".to_string())
                } else {
                    category_by_id
                        .get(cat_id.as_str())
                        .map(|c| (c.name.clone(), c.color.clone()))
                        .unwrap_or_else(|| (cat_id.clone(), "#808080".to_string()))
                };

                let percentage = top_level_percent_map
                    .get(&cat_id)
                    .copied()
                    .unwrap_or(Decimal::ZERO);

                let children = children_map.remove(&cat_id).unwrap_or_default();

                CategoryAllocation {
                    category_id: cat_id,
                    category_name: name,
                    color,
                    value,
                    percentage,
                    children,
                }
            })
            .collect();

        // Sort by value descending
        allocations.sort_by_key(|b| std::cmp::Reverse(b.value));

        TaxonomyAllocation {
            taxonomy_id: taxonomy_id.to_string(),
            taxonomy_name: taxonomy_name.to_string(),
            color: taxonomy_color.to_string(),
            categories: allocations,
        }
    }

    /// Builds a map from each category to its top-level ancestor.
    /// Top-level categories are those with parent_id = None.
    fn build_top_level_map<'a>(&self, categories: &'a [Category]) -> HashMap<&'a str, &'a str> {
        let mut result: HashMap<&str, &str> = HashMap::new();

        // Build parent lookup
        let parent_map: HashMap<&str, Option<&str>> = categories
            .iter()
            .map(|c| (c.id.as_str(), c.parent_id.as_deref()))
            .collect();

        for category in categories {
            let top_level = self.find_top_level_ancestor(&category.id, &parent_map);
            result.insert(category.id.as_str(), top_level);
        }

        result
    }

    /// Recursively finds the top-level ancestor of a category.
    #[allow(clippy::only_used_in_recursion)]
    fn find_top_level_ancestor<'a>(
        &self,
        category_id: &'a str,
        parent_map: &HashMap<&str, Option<&'a str>>,
    ) -> &'a str {
        match parent_map.get(category_id) {
            Some(Some(parent_id)) => self.find_top_level_ancestor(parent_id, parent_map),
            _ => category_id, // No parent - this is the top level
        }
    }
}

#[async_trait]
impl AllocationServiceTrait for AllocationService {
    async fn get_portfolio_allocations(
        &self,
        account_id: &str,
        base_currency: &str,
    ) -> Result<PortfolioAllocations> {
        debug!(
            "Computing portfolio allocations for account {} in {}",
            account_id, base_currency
        );

        // 1. Get holdings
        let holdings = self
            .holdings_service
            .get_holdings(account_id, base_currency)
            .await?;

        if holdings.is_empty() {
            return Ok(PortfolioAllocations::default());
        }

        // 2. Compute total portfolio value (excluding cash for some allocations)
        let total_value: Decimal = holdings
            .iter()
            .filter(|h| h.holding_type != HoldingType::Cash)
            .map(|h| h.market_value.base)
            .sum();

        let total_with_cash: Decimal = holdings.iter().map(|h| h.market_value.base).sum();

        // 3. Get all taxonomies with categories
        let taxonomies = self.taxonomy_service.get_taxonomies_with_categories()?;

        // 4. Collect all asset IDs from holdings
        let asset_ids: Vec<String> = holdings
            .iter()
            .filter_map(|h| h.instrument.as_ref().map(|i| i.id.clone()))
            .collect();

        // 5. Get all assignments for these assets
        let mut assignments_by_asset: HashMap<String, Vec<(String, String, i32)>> = HashMap::new();

        for asset_id in &asset_ids {
            let assignments = self.taxonomy_service.get_asset_assignments(asset_id)?;
            let entries: Vec<(String, String, i32)> = assignments
                .into_iter()
                .map(|a| (a.taxonomy_id, a.category_id, a.weight))
                .collect();
            if !entries.is_empty() {
                assignments_by_asset.insert(asset_id.clone(), entries);
            }
        }

        // 6. Find each taxonomy and its categories
        let mut asset_classes_alloc =
            TaxonomyAllocation::empty("asset_classes", "Asset Classes", "#879a39");
        let mut sectors_alloc = TaxonomyAllocation::empty("industries_gics", "Sectors", "#da702c");
        let mut regions_alloc = TaxonomyAllocation::empty("regions", "Regions", "#8b7ec8");
        let mut risk_alloc = TaxonomyAllocation::empty("risk_category", "Risk Category", "#d14d41");
        let mut security_types_alloc =
            TaxonomyAllocation::empty("instrument_type", "Instrument Type", "#4385be");
        let mut custom_allocs: Vec<TaxonomyAllocation> = Vec::new();

        for twc in taxonomies {
            let taxonomy = &twc.taxonomy;
            let categories = &twc.categories;

            match taxonomy.id.as_str() {
                "asset_classes" => {
                    // Asset classes include cash, use total_with_cash
                    // Cash holdings now have proper instruments with classifications
                    asset_classes_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        &taxonomy.name,
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_with_cash,
                        true, // Roll up to top-level asset classes
                    );
                }
                "industries_gics" => {
                    sectors_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        "Sectors", // Use friendly name
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_value,
                        true, // Roll up to top-level GICS sectors
                    );
                }
                "regions" => {
                    regions_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        "Regions",
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_value,
                        true, // Roll up to top-level regions
                    );
                }
                "risk_category" => {
                    risk_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        "Risk Category",
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_value,
                        false, // No rollup for risk
                    );
                }
                "instrument_type" => {
                    security_types_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        "Instrument Type",
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_value,
                        true, // Roll up to top-level instrument types
                    );
                }
                _ if !taxonomy.is_system => {
                    // User-created custom taxonomies only (skip system placeholder "custom_groups")
                    let custom_alloc = self.aggregate_by_taxonomy(
                        &holdings,
                        &taxonomy.id,
                        &taxonomy.name,
                        &taxonomy.color,
                        categories,
                        &assignments_by_asset,
                        total_value,
                        false,
                    );
                    // Only include if there are real categories (not just Unknown)
                    if !custom_alloc.categories.is_empty() {
                        custom_allocs.push(custom_alloc);
                    }
                }
                _ => {}
            }
        }

        Ok(PortfolioAllocations {
            asset_classes: asset_classes_alloc,
            sectors: sectors_alloc,
            regions: regions_alloc,
            risk_category: risk_alloc,
            security_types: security_types_alloc,
            custom_groups: custom_allocs,
            total_value: total_with_cash,
        })
    }

    async fn get_holdings_by_allocation(
        &self,
        account_id: &str,
        base_currency: &str,
        taxonomy_id: &str,
        category_id: &str,
    ) -> Result<AllocationHoldings> {
        debug!(
            "Getting holdings for category {} in taxonomy {} for account {}",
            category_id, taxonomy_id, account_id
        );

        // Get taxonomy with categories for hierarchy lookup and metadata
        let taxonomy_with_cats = self.taxonomy_service.get_taxonomy(taxonomy_id)?;
        let empty_categories: Vec<Category> = Vec::new();

        // Extract taxonomy metadata
        let (taxonomy_name, taxonomy_color, categories) = match &taxonomy_with_cats {
            Some(twc) => (
                twc.taxonomy.name.clone(),
                twc.taxonomy.color.clone(),
                &twc.categories,
            ),
            None => (
                "Unknown".to_string(),
                "#808080".to_string(),
                &empty_categories,
            ),
        };

        // Look up category metadata
        let (category_name, category_color) = if category_id == "__UNKNOWN__" {
            ("Unknown".to_string(), "#878580".to_string())
        } else {
            categories
                .iter()
                .find(|c| c.id == category_id)
                .map(|c| (c.name.clone(), c.color.clone()))
                .unwrap_or_else(|| (category_id.to_string(), taxonomy_color.clone()))
        };

        // Get all holdings for the account
        let holdings = self
            .holdings_service
            .get_holdings(account_id, base_currency)
            .await?;

        if holdings.is_empty() {
            return Ok(AllocationHoldings {
                taxonomy_id: taxonomy_id.to_string(),
                taxonomy_name,
                category_id: category_id.to_string(),
                category_name,
                color: category_color,
                holdings: Vec::new(),
                total_value: Decimal::ZERO,
                currency: base_currency.to_string(),
            });
        }

        // Build map from category to top-level ancestor
        let top_level_map: HashMap<&str, &str> = self.build_top_level_map(categories);

        // Get all assignments for this category (including child categories)
        // First, find all category IDs that roll up to the target category
        let matching_category_ids: Vec<&str> = if category_id == "__UNKNOWN__" {
            vec!["__UNKNOWN__"]
        } else {
            categories
                .iter()
                .filter(|c| {
                    // Include this category if it equals or rolls up to the target
                    c.id == category_id
                        || top_level_map.get(c.id.as_str()).copied() == Some(category_id)
                })
                .map(|c| c.id.as_str())
                .collect()
        };

        // Get assignments for all matching categories
        let mut asset_to_weight: HashMap<String, i32> = HashMap::new();
        for cat_id in &matching_category_ids {
            if *cat_id == "__UNKNOWN__" {
                continue;
            }
            if let Ok(assignments) = self
                .taxonomy_service
                .get_category_assignments(taxonomy_id, cat_id)
            {
                for assignment in assignments {
                    *asset_to_weight
                        .entry(assignment.asset_id.clone())
                        .or_insert(0) += assignment.weight;
                }
            }
        }

        // Calculate total value of matched holdings for weight calculation
        let mut matched_holdings: Vec<(&Holding, i32)> = Vec::new();

        for holding in &holdings {
            let asset_id = match &holding.instrument {
                Some(instrument) => &instrument.id,
                None => continue,
            };

            // Cash holdings: match if drilling into CASH or CASH_BANK_DEPOSITS
            if holding.holding_type == HoldingType::Cash
                && taxonomy_id == "asset_classes"
                && matching_category_ids
                    .iter()
                    .any(|id| *id == "CASH" || *id == "CASH_BANK_DEPOSITS")
            {
                matched_holdings.push((holding, 10000)); // 100% weight
                continue;
            }

            // Check if this holding matches the category
            if category_id == "__UNKNOWN__" {
                // For "Unknown", include holdings with no assignment for this taxonomy
                let has_assignment = self
                    .taxonomy_service
                    .get_asset_assignments(asset_id)
                    .map(|assignments| assignments.iter().any(|a| a.taxonomy_id == taxonomy_id))
                    .unwrap_or(false);

                if !has_assignment {
                    matched_holdings.push((holding, 10000)); // 100% weight
                }
            } else if let Some(&weight) = asset_to_weight.get(asset_id) {
                matched_holdings.push((holding, weight));
            }
        }

        // Calculate total matched value for weight calculation
        let total_matched_value: Decimal = matched_holdings
            .iter()
            .map(|(h, weight)| {
                let weight_decimal = Decimal::from(*weight) / dec!(10000);
                h.market_value.base * weight_decimal
            })
            .sum();

        // Build summaries
        let mut summaries: Vec<HoldingSummary> = matched_holdings
            .into_iter()
            .map(|(holding, weight)| {
                let weight_decimal = Decimal::from(weight) / dec!(10000);
                let weighted_value = holding.market_value.base * weight_decimal;
                let weight_in_category = if total_matched_value > Decimal::ZERO {
                    (weighted_value / total_matched_value * dec!(100)).round_dp(2)
                } else {
                    Decimal::ZERO
                };

                HoldingSummary {
                    // Use instrument.id (the asset ID) for navigation, not holding.id (composite ID)
                    id: holding
                        .instrument
                        .as_ref()
                        .map(|i| i.id.clone())
                        .unwrap_or_else(|| holding.id.clone()),
                    symbol: holding
                        .instrument
                        .as_ref()
                        .map(|i| i.symbol.clone())
                        .unwrap_or_default(),
                    name: holding.instrument.as_ref().and_then(|i| i.name.clone()),
                    holding_type: holding.holding_type.clone(),
                    quantity: holding.quantity,
                    market_value: weighted_value,
                    currency: holding.base_currency.clone(),
                    weight_in_category,
                }
            })
            .collect();

        // Sort by market value descending
        summaries.sort_by_key(|b| std::cmp::Reverse(b.market_value));

        Ok(AllocationHoldings {
            taxonomy_id: taxonomy_id.to_string(),
            taxonomy_name,
            category_id: category_id.to_string(),
            category_name,
            color: category_color,
            holdings: summaries,
            total_value: total_matched_value,
            currency: base_currency.to_string(),
        })
    }
}

#[cfg(test)]
mod lrm_tests {
    use super::*;

    /// LRM invariant: the rounded percentages MUST sum to exactly
    /// 100.00 whenever the input has at least one positive value.
    /// This is the property that was previously violated and that
    /// caused pie-chart legends to display 99.99% or 100.01%.
    #[test]
    fn three_thirds_sum_to_exactly_100() {
        let pairs = vec![
            ("a".to_string(), dec!(100)),
            ("b".to_string(), dec!(100)),
            ("c".to_string(), dec!(100)),
        ];
        let pcts = largest_remainder_percentages(&pairs, dec!(300));
        let sum: Decimal = pcts.values().copied().sum();
        assert_eq!(sum, dec!(100.00), "got {pcts:?}");
        // Each share is 33.33 or 33.34 — one row absorbs the deficit.
        for v in pcts.values() {
            assert!(*v == dec!(33.33) || *v == dec!(33.34), "got {v}");
        }
    }

    #[test]
    fn five_uneven_categories_sum_to_exactly_100() {
        // 1, 1, 1, 1, 6 — naïve rounding tends to drift by 0.04.
        let pairs = vec![
            ("a".to_string(), dec!(1)),
            ("b".to_string(), dec!(1)),
            ("c".to_string(), dec!(1)),
            ("d".to_string(), dec!(1)),
            ("e".to_string(), dec!(6)),
        ];
        let pcts = largest_remainder_percentages(&pairs, dec!(10));
        let sum: Decimal = pcts.values().copied().sum();
        assert_eq!(sum, dec!(100.00));
    }

    #[test]
    fn single_category_gets_full_hundred() {
        let pairs = vec![("only".to_string(), dec!(1234.567))];
        let pcts = largest_remainder_percentages(&pairs, dec!(1234.567));
        assert_eq!(pcts.get("only"), Some(&dec!(100.00)));
    }

    #[test]
    fn empty_or_zero_total_returns_empty_map() {
        let empty: Vec<(String, Decimal)> = Vec::new();
        assert!(largest_remainder_percentages(&empty, dec!(100)).is_empty());
        let zero_pairs = vec![("a".to_string(), dec!(0))];
        assert!(largest_remainder_percentages(&zero_pairs, dec!(0)).is_empty());
    }

    /// Property-style sweep — for any random portfolio shape with N≤10
    /// rows the sum must be exactly 100.00. We hand-pick adversarial
    /// shapes that are known to break naïve rounding.
    #[test]
    fn lrm_sum_invariant_across_adversarial_shapes() {
        let shapes: Vec<Vec<Decimal>> = vec![
            vec![dec!(1)],
            vec![dec!(1), dec!(1)],
            vec![dec!(1), dec!(1), dec!(1)],
            vec![dec!(1), dec!(1), dec!(1), dec!(1), dec!(1), dec!(1), dec!(1)],
            // 7 thirds + 1 large
            vec![dec!(1); 7].into_iter().chain([dec!(50)]).collect(),
            // Drift-classic: 7 + 3 + 2
            vec![dec!(7), dec!(3), dec!(2)],
            // Many small + one giant
            vec![dec!(0.01); 50].into_iter().chain([dec!(99.5)]).collect(),
        ];
        for (idx, shape) in shapes.iter().enumerate() {
            let total: Decimal = shape.iter().copied().sum();
            let pairs: Vec<(String, Decimal)> = shape
                .iter()
                .enumerate()
                .map(|(i, v)| (format!("c{i}"), *v))
                .collect();
            let pcts = largest_remainder_percentages(&pairs, total);
            let sum: Decimal = pcts.values().copied().sum();
            assert_eq!(
                sum,
                dec!(100.00),
                "shape {idx} ({shape:?}) summed to {sum} not 100.00"
            );
        }
    }
}
