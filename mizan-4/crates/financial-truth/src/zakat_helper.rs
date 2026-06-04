//! Zakat → Truth Ledger entry builder — Track F PR-F4 / Goal v3 §V Phase 8.
//!
//! Per CLAUDE.md §0 rule 1, every Zakat number Mizan reports to the user
//! must be backed by an immutable hash-chained ledger entry. This module
//! is the typed factory: it accepts the components of a Zakat assessment
//! (school + the five Decimal-valued numbers + currency + recorded_at +
//! caller-supplied audit id) and returns an [`AppendInput`] ready for
//! `TruthLedger::append`.
//!
//! # Why here, not in `mizan-zakat`
//!
//! `mizan-zakat` has a back-edge dep on `mizan-core` today (per its
//! Cargo.toml comment) and we don't want to also pull
//! `mizan-financial-truth` into it. Putting the builder in this crate
//! keeps the dep direction one-way: `mizan-zakat` → caller →
//! `mizan-financial-truth::zakat_helper`, and the helper itself stays
//! Zakat-type-agnostic (it accepts raw Decimals).
//!
//! # Payload shape
//!
//! The ledger entry uses `LedgerEntryKind::ZakatComputed` with a
//! `metadata` JSON object carrying:
//!
//! ```json
//! {
//!   "school":               "hanafi" | "shafii" | "maliki" | "hanbali",
//!   "total_assessable":     "<Decimal as string>",
//!   "deductible_debts":     "<Decimal as string>",
//!   "net_zakat_base":       "<Decimal as string>",
//!   "nisab_threshold":      "<Decimal as string>",
//!   "is_above_nisab":       <bool>,
//!   "currency":             "<ISO 4217>"
//! }
//! ```
//!
//! The `amount` + `currency` + `recorded_at` fields on [`AppendInput`]
//! are populated from the inputs so the ledger row can be indexed
//! without parsing the JSON. The `id` is caller-supplied — typically a
//! deterministic UUID built from `(user_id, hawl_cohort_id, recorded_at_iso)`
//! so re-running the assessment for the same cohort on the same day
//! is idempotent (re-append rejected by `TruthLedger::append`).
//!
//! # Determinism
//!
//! The metadata JSON uses a stable key ordering (BTreeMap). Decimal
//! values are serialised as strings to preserve precision (per the
//! `serde-with-str` feature already used in `model.rs`). This means
//! the entry hash is byte-stable across runs — critical for the
//! chain-integrity verifier.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::model::LedgerEntryKind;
use crate::service::AppendInput;

/// Inputs for [`build_zakat_append_input`].
#[derive(Debug, Clone)]
pub struct ZakatLedgerInputs {
    /// Caller-supplied unique id (deterministic across re-runs of the
    /// same cohort on the same day → idempotent append).
    pub id: String,
    /// School identifier: `"hanafi"` / `"shafii"` / `"maliki"` / `"hanbali"`.
    /// Stored lowercased.
    pub school: String,
    /// Optional user / account id the entry pertains to. Used for the
    /// `by_account` query when the desktop wants to filter the trail
    /// to a single profile.
    pub account_id: Option<String>,
    pub total_assessable: Decimal,
    pub deductible_debts: Decimal,
    pub net_zakat_base: Decimal,
    pub nisab_threshold: Decimal,
    pub is_above_nisab: bool,
    pub zakat_due: Decimal,
    pub currency: String,
    /// Optional explicit timestamp — defaults to None (the ledger
    /// service stamps `Utc::now()` if absent).
    pub recorded_at: Option<DateTime<Utc>>,
}

/// Build an [`AppendInput`] for a Zakat assessment. The result is
/// ready to pass to `TruthLedger::append(...)` — the ledger fills in
/// sequence, prev_hash, and entry_hash.
pub fn build_zakat_append_input(inputs: ZakatLedgerInputs) -> AppendInput {
    let mut metadata: BTreeMap<String, Value> = BTreeMap::new();
    metadata.insert("school".into(), Value::String(inputs.school.to_lowercase()));
    metadata.insert(
        "total_assessable".into(),
        Value::String(inputs.total_assessable.to_string()),
    );
    metadata.insert(
        "deductible_debts".into(),
        Value::String(inputs.deductible_debts.to_string()),
    );
    metadata.insert(
        "net_zakat_base".into(),
        Value::String(inputs.net_zakat_base.to_string()),
    );
    metadata.insert(
        "nisab_threshold".into(),
        Value::String(inputs.nisab_threshold.to_string()),
    );
    metadata.insert("is_above_nisab".into(), Value::Bool(inputs.is_above_nisab));
    metadata.insert("currency".into(), Value::String(inputs.currency.clone()));

    AppendInput {
        id: inputs.id,
        kind: Some(LedgerEntryKind::ZakatComputed),
        account_id: inputs.account_id,
        asset_id: None,
        amount: Some(inputs.zakat_due),
        currency: Some(inputs.currency),
        metadata,
        recorded_at: inputs.recorded_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn s23_inputs() -> ZakatLedgerInputs {
        // §23 reference user: Singapore Sharia-aware millionaire's
        // approximate Ramadan Zakat — Hanafi school, ~$2.9M assessable,
        // ~$3K debts, ~$2.897M net base above $5K nisab, 2.5% → $72,425.
        ZakatLedgerInputs {
            id: "zakat-2026-ramadan-user-7".into(),
            school: "hanafi".into(),
            account_id: Some("user-7".into()),
            total_assessable: dec!(2_900_000),
            deductible_debts: dec!(3_000),
            net_zakat_base: dec!(2_897_000),
            nisab_threshold: dec!(5_000),
            is_above_nisab: true,
            zakat_due: dec!(72_425),
            currency: "SGD".into(),
            recorded_at: None,
        }
    }

    #[test]
    fn build_input_carries_zakat_due_as_amount() {
        let input = build_zakat_append_input(s23_inputs());
        assert_eq!(input.amount, Some(dec!(72_425)));
        assert_eq!(input.currency.as_deref(), Some("SGD"));
    }

    #[test]
    fn build_input_kind_is_zakat_computed() {
        let input = build_zakat_append_input(s23_inputs());
        assert_eq!(input.kind, Some(LedgerEntryKind::ZakatComputed));
    }

    #[test]
    fn build_input_id_preserved() {
        let input = build_zakat_append_input(s23_inputs());
        assert_eq!(input.id, "zakat-2026-ramadan-user-7");
    }

    #[test]
    fn metadata_carries_all_components() {
        let input = build_zakat_append_input(s23_inputs());
        assert_eq!(
            input.metadata.get("school"),
            Some(&Value::String("hanafi".into()))
        );
        assert_eq!(
            input.metadata.get("total_assessable"),
            Some(&Value::String("2900000".into()))
        );
        assert_eq!(
            input.metadata.get("deductible_debts"),
            Some(&Value::String("3000".into()))
        );
        assert_eq!(
            input.metadata.get("net_zakat_base"),
            Some(&Value::String("2897000".into()))
        );
        assert_eq!(
            input.metadata.get("nisab_threshold"),
            Some(&Value::String("5000".into()))
        );
        assert_eq!(
            input.metadata.get("is_above_nisab"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            input.metadata.get("currency"),
            Some(&Value::String("SGD".into()))
        );
    }

    #[test]
    fn metadata_key_ordering_is_stable() {
        // BTreeMap guarantees alphabetic iteration order. Pin it so
        // canonical_payload hashing is byte-stable.
        let input = build_zakat_append_input(s23_inputs());
        let keys: Vec<&String> = input.metadata.keys().collect();
        let expected = vec![
            "currency",
            "deductible_debts",
            "is_above_nisab",
            "net_zakat_base",
            "nisab_threshold",
            "school",
            "total_assessable",
        ];
        assert_eq!(
            keys.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn school_normalised_lowercase_in_metadata() {
        let mut inputs = s23_inputs();
        inputs.school = "MALIKI".into();
        let input = build_zakat_append_input(inputs);
        assert_eq!(
            input.metadata.get("school"),
            Some(&Value::String("maliki".into()))
        );
    }

    #[test]
    fn account_id_threaded() {
        let input = build_zakat_append_input(s23_inputs());
        assert_eq!(input.account_id.as_deref(), Some("user-7"));
    }

    #[test]
    fn no_asset_id_on_zakat_entry() {
        let input = build_zakat_append_input(s23_inputs());
        // Zakat is portfolio-wide, not per-asset.
        assert!(input.asset_id.is_none());
    }

    #[test]
    fn recorded_at_passes_through_when_set() {
        let ts = "2026-03-20T12:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("valid");
        let mut inputs = s23_inputs();
        inputs.recorded_at = Some(ts);
        let input = build_zakat_append_input(inputs);
        assert_eq!(input.recorded_at, Some(ts));
    }

    #[test]
    fn below_nisab_serialises_false_flag() {
        let mut inputs = s23_inputs();
        inputs.is_above_nisab = false;
        inputs.zakat_due = Decimal::ZERO;
        let input = build_zakat_append_input(inputs);
        assert_eq!(input.amount, Some(Decimal::ZERO));
        assert_eq!(
            input.metadata.get("is_above_nisab"),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn decimal_precision_preserved_via_string_serde() {
        let mut inputs = s23_inputs();
        // 2.5% of $7,777.77 → $194.44425 (5 decimal places)
        inputs.zakat_due = dec!(194.44425);
        let input = build_zakat_append_input(inputs);
        assert_eq!(input.amount, Some(dec!(194.44425)));
    }
}
