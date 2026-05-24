//! Read-only detection of assets that probably represent the *same* real
//! instrument but were stored as separate rows.
//!
//! The system already prevents **exact**-`instrument_key` duplicates (the
//! generated key `EQUITY:AAPL@XNAS` is looked up before any insert). The
//! residual problem is the *same* instrument landing under **different** keys —
//! most commonly an entry imported with no exchange (`EQUITY:AAPL`) sitting
//! next to one that has a MIC (`EQUITY:AAPL@XNAS`), which split a user's
//! holdings across two assets.
//!
//! This module only **flags** such candidates for the user to review and merge
//! deliberately (Mizan already has a manual `merge_unknown_asset` path). It
//! never merges anything itself: a false positive here costs a suggestion, not
//! data. To keep false positives low it is deliberately conservative — it will
//! not group two assets that carry *different* explicit exchanges, since those
//! are usually genuine cross-listings (e.g. the same company on XNAS and XLON),
//! not duplicates.

use super::assets_model::Asset;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Why a set of assets was flagged as possible duplicates. A group may carry
/// more than one reason when it was joined transitively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DuplicateReason {
    /// The assets carry an identical ISIN (`metadata.identifiers.isin`).
    SharedIsin { isin: String },
    /// Same instrument type + symbol + quote currency, but at least one asset
    /// has no exchange (MIC) set — so it is ambiguous and most likely the same
    /// listing as the one that does.
    AmbiguousExchange,
}

/// A set of assets that probably represent the same real-world instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateAssetGroup {
    /// The duplicate asset ids, sorted for stable output.
    pub asset_ids: Vec<String>,
    /// One or more concrete reasons the group was flagged.
    pub reasons: Vec<DuplicateReason>,
}

/// Uppercased ISIN from `metadata.identifiers.isin`, if present and non-empty.
fn isin_of(asset: &Asset) -> Option<String> {
    asset
        .metadata
        .as_ref()?
        .get("identifiers")?
        .get("isin")?
        .as_str()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
}

/// Normalised MIC, if the asset has a non-empty exchange set.
fn mic_of(asset: &Asset) -> Option<String> {
    asset
        .instrument_exchange_mic
        .as_ref()
        .map(|m| m.trim().to_uppercase())
        .filter(|m| !m.is_empty())
}

/// Grouping key for the ambiguous-exchange rule: `(type, SYMBOL, quote_ccy)`.
/// `None` for non-market assets (no instrument symbol/type), which are never
/// grouped by this rule. Quote currency is part of the key so an asset priced
/// in USD is never merged with one priced in EUR, and the FX/crypto base
/// symbol (`BTC`) doesn't collapse different pairs (`BTC/USD` vs `BTC/EUR`).
fn symbol_key(asset: &Asset) -> Option<(String, String, String)> {
    let symbol = asset.instrument_symbol.as_ref()?.trim().to_uppercase();
    if symbol.is_empty() {
        return None;
    }
    let instrument_type = asset.instrument_type.as_ref()?;
    Some((
        format!("{instrument_type:?}"),
        symbol,
        asset.quote_ccy.trim().to_uppercase(),
    ))
}

/// Find groups of assets that probably represent the same instrument.
///
/// Pure and deterministic: the same input always yields the same output, with
/// asset ids and groups sorted. Returns an empty vec when nothing is flagged.
pub fn find_duplicate_asset_groups(assets: &[Asset]) -> Vec<DuplicateAssetGroup> {
    let n = assets.len();
    let mut uf = UnionFind::new(n);

    // Rule 1 — shared ISIN: an identical ISIN is a strong same-security signal.
    let mut by_isin: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, asset) in assets.iter().enumerate() {
        if let Some(isin) = isin_of(asset) {
            by_isin.entry(isin).or_default().push(i);
        }
    }
    for indices in by_isin.values() {
        for pair in indices.windows(2) {
            uf.union(pair[0], pair[1]);
        }
    }

    // Rule 2 — ambiguous exchange: within one (type, symbol, ccy) bucket, an
    // asset with no MIC is probably the same as one that has a MIC — but only
    // when there's a single candidate MIC. If two or more distinct MICs are
    // present, the no-MIC asset is genuinely ambiguous and the MIC'd assets are
    // likely separate cross-listings, so we do NOT connect them.
    let mut by_symbol: HashMap<(String, String, String), Vec<usize>> = HashMap::new();
    for (i, asset) in assets.iter().enumerate() {
        if let Some(key) = symbol_key(asset) {
            by_symbol.entry(key).or_default().push(i);
        }
    }
    for indices in by_symbol.values() {
        let no_mic: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| mic_of(&assets[i]).is_none())
            .collect();
        let mut mic_buckets: HashMap<String, Vec<usize>> = HashMap::new();
        for &i in indices {
            if let Some(mic) = mic_of(&assets[i]) {
                mic_buckets.entry(mic).or_default().push(i);
            }
        }
        // Assets with no exchange but the same symbol/type/ccy are duplicates
        // of each other regardless of how many MICs exist elsewhere.
        for pair in no_mic.windows(2) {
            uf.union(pair[0], pair[1]);
        }
        // Two assets sharing the same explicit MIC + key are duplicates too.
        for bucket in mic_buckets.values() {
            for pair in bucket.windows(2) {
                uf.union(pair[0], pair[1]);
            }
        }
        // Exactly one candidate exchange → attach the ambiguous no-MIC asset(s).
        if mic_buckets.len() == 1 {
            if let (Some(&first_no_mic), Some(bucket)) =
                (no_mic.first(), mic_buckets.values().next())
            {
                uf.union(first_no_mic, bucket[0]);
            }
        }
    }

    // Assemble connected components into reported groups.
    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        components.entry(root).or_default().push(i);
    }

    let mut groups: Vec<DuplicateAssetGroup> = Vec::new();
    for members in components.values() {
        if members.len() < 2 {
            continue;
        }
        let reasons = derive_reasons(assets, members);
        // Only surface a group we can concretely explain.
        if reasons.is_empty() {
            continue;
        }
        let mut asset_ids: Vec<String> = members.iter().map(|&i| assets[i].id.clone()).collect();
        asset_ids.sort();
        groups.push(DuplicateAssetGroup { asset_ids, reasons });
    }
    groups.sort_by(|a, b| a.asset_ids.cmp(&b.asset_ids));
    groups
}

/// Reconstruct the human-readable reasons for an already-formed component by
/// inspecting its members (independent of the union order).
fn derive_reasons(assets: &[Asset], members: &[usize]) -> Vec<DuplicateReason> {
    let mut reasons: Vec<DuplicateReason> = Vec::new();

    // Shared ISINs (any ISIN held by 2+ members).
    let mut isin_counts: HashMap<String, usize> = HashMap::new();
    for &i in members {
        if let Some(isin) = isin_of(&assets[i]) {
            *isin_counts.entry(isin).or_default() += 1;
        }
    }
    let mut shared_isins: Vec<String> = isin_counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(isin, _)| isin)
        .collect();
    shared_isins.sort();
    for isin in shared_isins {
        reasons.push(DuplicateReason::SharedIsin { isin });
    }

    // Ambiguous exchange: 2+ members share a symbol key and at least one has no MIC.
    let mut key_has_no_mic: HashMap<(String, String, String), bool> = HashMap::new();
    let mut key_count: HashMap<(String, String, String), usize> = HashMap::new();
    for &i in members {
        if let Some(key) = symbol_key(&assets[i]) {
            *key_count.entry(key.clone()).or_default() += 1;
            let entry = key_has_no_mic.entry(key).or_insert(false);
            *entry = *entry || mic_of(&assets[i]).is_none();
        }
    }
    let ambiguous = key_count
        .iter()
        .any(|(key, &count)| count >= 2 && *key_has_no_mic.get(key).unwrap_or(&false));
    if ambiguous {
        reasons.push(DuplicateReason::AmbiguousExchange);
    }

    reasons
}

/// Minimal union-find (disjoint-set) with path compression + union by rank.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{AssetKind, InstrumentType, QuoteMode};
    use serde_json::json;

    fn equity(id: &str, symbol: &str, mic: Option<&str>, ccy: &str) -> Asset {
        Asset {
            id: id.to_string(),
            kind: AssetKind::default(),
            quote_mode: QuoteMode::Market,
            quote_ccy: ccy.to_string(),
            instrument_type: Some(InstrumentType::Equity),
            instrument_symbol: Some(symbol.to_string()),
            instrument_exchange_mic: mic.map(|m| m.to_string()),
            ..Default::default()
        }
    }

    fn with_isin(mut asset: Asset, isin: &str) -> Asset {
        asset.metadata = Some(json!({ "identifiers": { "isin": isin } }));
        asset
    }

    #[test]
    fn no_assets_no_groups() {
        assert!(find_duplicate_asset_groups(&[]).is_empty());
    }

    #[test]
    fn distinct_instruments_are_not_grouped() {
        let assets = vec![
            equity("1", "AAPL", Some("XNAS"), "USD"),
            equity("2", "MSFT", Some("XNAS"), "USD"),
        ];
        assert!(find_duplicate_asset_groups(&assets).is_empty());
    }

    #[test]
    fn missing_exchange_flags_ambiguous_duplicate() {
        // Same symbol/ccy, one with a MIC and one without → flagged.
        let assets = vec![
            equity("1", "AAPL", Some("XNAS"), "USD"),
            equity("2", "AAPL", None, "USD"),
        ];
        let groups = find_duplicate_asset_groups(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].asset_ids, vec!["1", "2"]);
        assert_eq!(groups[0].reasons, vec![DuplicateReason::AmbiguousExchange]);
    }

    #[test]
    fn different_explicit_exchanges_are_not_merged() {
        // Genuine cross-listings — must NOT be flagged.
        let assets = vec![
            equity("1", "AAPL", Some("XNAS"), "USD"),
            equity("2", "AAPL", Some("XLON"), "USD"),
        ];
        assert!(find_duplicate_asset_groups(&assets).is_empty());
    }

    #[test]
    fn ambiguous_no_mic_not_attached_when_multiple_exchanges() {
        // Two distinct cross-listings + one ambiguous no-MIC entry. The no-MIC
        // entry is genuinely ambiguous (could be either), so it must not fuse
        // the two cross-listings into one group, and a lone no-MIC asset is
        // not itself a group.
        let assets = vec![
            equity("1", "AAPL", Some("XNAS"), "USD"),
            equity("2", "AAPL", Some("XLON"), "USD"),
            equity("3", "AAPL", None, "USD"),
        ];
        assert!(find_duplicate_asset_groups(&assets).is_empty());
    }

    #[test]
    fn different_quote_currencies_are_not_merged() {
        let assets = vec![
            equity("1", "AAPL", None, "USD"),
            equity("2", "AAPL", None, "EUR"),
        ];
        assert!(find_duplicate_asset_groups(&assets).is_empty());
    }

    #[test]
    fn shared_isin_flags_duplicate_even_with_different_keys() {
        // Two rows for the same security, only an ISIN in common.
        let assets = vec![
            with_isin(equity("1", "SHOP", Some("XTSE"), "CAD"), "CA82509L1076"),
            with_isin(equity("2", "SHOP.TO", None, "CAD"), "ca82509l1076"),
        ];
        let groups = find_duplicate_asset_groups(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].asset_ids, vec!["1", "2"]);
        assert_eq!(
            groups[0].reasons,
            vec![DuplicateReason::SharedIsin {
                isin: "CA82509L1076".to_string()
            }]
        );
    }

    #[test]
    fn transitive_group_carries_both_reasons() {
        // 1 & 2 share an ISIN; 2 & 3 share symbol with an ambiguous exchange.
        // All three collapse into one group with both reasons.
        let assets = vec![
            with_isin(equity("1", "FOO", Some("XNAS"), "USD"), "US1111111111"),
            with_isin(equity("2", "FOO", None, "USD"), "US1111111111"),
            equity("3", "FOO", Some("XNAS"), "USD"),
        ];
        let groups = find_duplicate_asset_groups(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].asset_ids, vec!["1", "2", "3"]);
        assert!(groups[0].reasons.contains(&DuplicateReason::SharedIsin {
            isin: "US1111111111".to_string()
        }));
        assert!(groups[0]
            .reasons
            .contains(&DuplicateReason::AmbiguousExchange));
    }

    #[test]
    fn output_is_deterministic_and_sorted() {
        let assets = vec![
            equity("z", "AAPL", None, "USD"),
            equity("a", "AAPL", Some("XNAS"), "USD"),
        ];
        let groups = find_duplicate_asset_groups(&assets);
        assert_eq!(groups.len(), 1);
        // asset_ids sorted ascending regardless of input order.
        assert_eq!(groups[0].asset_ids, vec!["a", "z"]);
    }
}
