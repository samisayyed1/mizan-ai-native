//! Truth Engine ledger trait + in-memory implementation.

#[cfg(any(test, feature = "test-utils"))]
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg(test)]
use super::model::canonical_payload;
#[cfg(any(test, feature = "test-utils"))]
use super::model::{derive_entry_hash, GENESIS_PREV_HASH};
use super::model::{LedgerEntry, LedgerEntryKind, LedgerIntegrityError};
use crate::Result;

/// Builder-style input for appending — the service computes sequence,
/// prev_hash, entry_hash, so callers can't accidentally inject a
/// wrong chain pointer.
#[derive(Debug, Clone, Default)]
pub struct AppendInput {
    pub id: String,
    pub kind: Option<LedgerEntryKind>,
    pub account_id: Option<String>,
    pub asset_id: Option<String>,
    pub amount: Option<Decimal>,
    pub currency: Option<String>,
    pub metadata: BTreeMap<String, Value>,
    /// Override the default `Utc::now()` for tests / replay scenarios.
    pub recorded_at: Option<DateTime<Utc>>,
}

/// Durable queue used by write-path callers to persist ledger appends
/// that failed transiently after the originating row already committed.
/// Implementations live in the storage layer (so they can be backed by
/// the same db connection); `mizan-core` only depends on the trait.
#[async_trait]
pub trait TruthLedgerRetryQueue: Send + Sync {
    /// Persist an AppendInput for later replay. Idempotent on
    /// `AppendInput.id` — re-enqueueing bumps the attempt count + last_error.
    async fn enqueue(&self, input: &AppendInput, reason: &str) -> Result<()>;
}

/// Append-only immutable ledger.
#[async_trait]
pub trait TruthLedger: Send + Sync {
    /// Append one entry. Returns the persisted entry with its assigned
    /// sequence, prev_hash, and entry_hash. Errors when:
    ///   - `input.kind` is None
    ///   - `input.id` is empty
    ///   - the same `id` is already in the ledger (re-append rejected)
    async fn append(&self, input: AppendInput) -> Result<LedgerEntry>;

    /// Verify the full chain. Returns Ok(()) on success or a precise
    /// `LedgerIntegrityError` naming the first bad entry.
    async fn verify(&self) -> std::result::Result<(), LedgerIntegrityError>;

    /// Read all entries in order. Bounded by `limit` — None = unlimited.
    async fn all(&self, limit: Option<usize>) -> Result<Vec<LedgerEntry>>;

    /// Read entries for a given account_id. Used by holdings + cash
    /// queries.
    async fn by_account(&self, account_id: &str) -> Result<Vec<LedgerEntry>>;
}

/// In-memory `TruthLedger` impl — gated behind `#[cfg(any(test, feature = "test-utils"))]`
/// so production binaries don't link this scaffolding. Consumers that need the
/// fake ledger for tests opt in via `mizan-financial-truth = { workspace = true,
/// features = ["test-utils"] }`. Pattern mirrors tokio's `test-util` feature.
#[cfg(any(test, feature = "test-utils"))]
pub struct InMemoryTruthLedger {
    entries: RwLock<Vec<LedgerEntry>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl InMemoryTruthLedger {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.read().expect("ledger poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Default for InMemoryTruthLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[async_trait]
impl TruthLedger for InMemoryTruthLedger {
    async fn append(&self, input: AppendInput) -> Result<LedgerEntry> {
        let kind = input.kind.ok_or_else(|| {
            crate::FinancialTruthError::InvalidInput("LedgerEntry kind is required".to_string())
        })?;
        if input.id.trim().is_empty() {
            return Err(crate::FinancialTruthError::InvalidInput(
                "LedgerEntry id cannot be empty".to_string(),
            ));
        }

        let mut entries = self.entries.write().expect("ledger poisoned");

        // Hard reject of id reuse — every entry must have a unique id.
        if entries.iter().any(|e| e.id == input.id) {
            return Err(crate::FinancialTruthError::InvalidInput(format!(
                "LedgerEntry id {} already exists; ledger is append-only",
                input.id
            )));
        }

        let sequence = entries.len() as u64;
        let prev_hash = entries
            .last()
            .map(|e| e.entry_hash.clone())
            .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());

        let recorded_at = input.recorded_at.unwrap_or_else(Utc::now);
        let entry = LedgerEntry::assemble(
            input.id,
            sequence,
            kind,
            prev_hash,
            input.account_id,
            input.asset_id,
            input.amount,
            input.currency,
            input.metadata,
            recorded_at,
        );

        entries.push(entry.clone());
        Ok(entry)
    }

    async fn verify(&self) -> std::result::Result<(), LedgerIntegrityError> {
        let entries = self.entries.read().expect("ledger poisoned");

        let mut expected_prev = GENESIS_PREV_HASH.to_string();

        for (expected_seq_usize, entry) in entries.iter().enumerate() {
            let expected_seq = expected_seq_usize as u64;
            if entry.sequence != expected_seq {
                return Err(LedgerIntegrityError::OutOfOrder(entry.id.clone()));
            }
            if entry.prev_hash != expected_prev {
                if expected_seq == 0 {
                    return Err(LedgerIntegrityError::InvalidGenesis(entry.id.clone()));
                }
                return Err(LedgerIntegrityError::BrokenChain(entry.id.clone()));
            }
            if derive_entry_hash(entry) != entry.entry_hash {
                return Err(LedgerIntegrityError::TamperedEntry(entry.id.clone()));
            }
            expected_prev = entry.entry_hash.clone();
        }
        Ok(())
    }

    async fn all(&self, limit: Option<usize>) -> Result<Vec<LedgerEntry>> {
        let entries = self.entries.read().expect("ledger poisoned");
        let n = limit.unwrap_or(entries.len()).min(entries.len());
        Ok(entries[..n].to_vec())
    }

    async fn by_account(&self, account_id: &str) -> Result<Vec<LedgerEntry>> {
        let entries = self.entries.read().expect("ledger poisoned");
        Ok(entries
            .iter()
            .filter(|e| e.account_id.as_deref() == Some(account_id))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::model::LedgerEntryKind;
    use rust_decimal_macros::dec;

    fn input(id: &str, kind: LedgerEntryKind) -> AppendInput {
        AppendInput {
            id: id.to_string(),
            kind: Some(kind),
            account_id: Some("acc-1".to_string()),
            asset_id: None,
            amount: Some(dec!(100)),
            currency: Some("USD".to_string()),
            metadata: BTreeMap::new(),
            recorded_at: None,
        }
    }

    /// Used by `canonical_payload` ↓ — pulled into test scope so its
    /// pub-use is exercised even when no other test references it
    /// directly.
    #[test]
    fn canonical_payload_is_pub_and_deterministic() {
        let l = InMemoryTruthLedger::new();
        // futures::executor avoids needing a tokio runtime for this one
        // assertion. Other tests use the #[tokio::test] macro.
        let entry =
            futures::executor::block_on(l.append(input("e1", LedgerEntryKind::AccountCreated)))
                .unwrap();
        let p1 = canonical_payload(&entry);
        let p2 = canonical_payload(&entry);
        assert_eq!(p1, p2, "canonical payload must be deterministic");
    }

    #[tokio::test]
    async fn appends_and_chains_two_entries() {
        let ledger = InMemoryTruthLedger::new();
        let e1 = ledger
            .append(input("e1", LedgerEntryKind::AccountCreated))
            .await
            .unwrap();
        let e2 = ledger
            .append(input("e2", LedgerEntryKind::ActivityRecorded))
            .await
            .unwrap();

        assert_eq!(e1.sequence, 0);
        assert_eq!(e1.prev_hash, GENESIS_PREV_HASH);
        assert_eq!(e2.sequence, 1);
        assert_eq!(e2.prev_hash, e1.entry_hash);

        ledger.verify().await.unwrap();
    }

    #[tokio::test]
    async fn verify_detects_tampered_amount() {
        let ledger = InMemoryTruthLedger::new();
        let _ = ledger
            .append(input("e1", LedgerEntryKind::AccountCreated))
            .await
            .unwrap();
        let _ = ledger
            .append(input("e2", LedgerEntryKind::ActivityRecorded))
            .await
            .unwrap();

        // Tamper with the first entry in place. Can't go through `append`
        // — we have to mutate the stored vec.
        {
            let mut entries = ledger.entries.write().unwrap();
            entries[0].amount = Some(dec!(99999));
            // Hash stays stale on purpose — that's what verify catches.
        }

        let err = ledger.verify().await.unwrap_err();
        assert_eq!(err, LedgerIntegrityError::TamperedEntry("e1".to_string()));
    }

    #[tokio::test]
    async fn verify_detects_broken_chain() {
        let ledger = InMemoryTruthLedger::new();
        let _ = ledger
            .append(input("e1", LedgerEntryKind::AccountCreated))
            .await
            .unwrap();
        let _ = ledger
            .append(input("e2", LedgerEntryKind::ActivityRecorded))
            .await
            .unwrap();

        // Tamper with the chain pointer on the second entry.
        {
            let mut entries = ledger.entries.write().unwrap();
            entries[1].prev_hash = "f".repeat(64);
            // Re-derive the entry_hash from the tampered prev so we hit
            // BrokenChain instead of TamperedEntry.
            entries[1].entry_hash = derive_entry_hash(&entries[1]);
        }

        let err = ledger.verify().await.unwrap_err();
        assert_eq!(err, LedgerIntegrityError::BrokenChain("e2".to_string()));
    }

    #[tokio::test]
    async fn rejects_duplicate_id() {
        let ledger = InMemoryTruthLedger::new();
        ledger
            .append(input("e1", LedgerEntryKind::AccountCreated))
            .await
            .unwrap();
        let err = ledger
            .append(input("e1", LedgerEntryKind::ActivityRecorded))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn rejects_empty_id_or_missing_kind() {
        let ledger = InMemoryTruthLedger::new();
        let mut bad = input("", LedgerEntryKind::AccountCreated);
        assert!(ledger.append(bad.clone()).await.is_err());

        bad.id = "ok-id".to_string();
        bad.kind = None;
        assert!(ledger.append(bad).await.is_err());
    }

    #[tokio::test]
    async fn by_account_filters() {
        let ledger = InMemoryTruthLedger::new();
        let mut a = input("e1", LedgerEntryKind::ActivityRecorded);
        a.account_id = Some("acc-1".to_string());
        ledger.append(a).await.unwrap();

        let mut b = input("e2", LedgerEntryKind::ActivityRecorded);
        b.account_id = Some("acc-2".to_string());
        ledger.append(b).await.unwrap();

        let acc1_only = ledger.by_account("acc-1").await.unwrap();
        assert_eq!(acc1_only.len(), 1);
        assert_eq!(acc1_only[0].id, "e1");
    }
}
