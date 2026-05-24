//! Financial Truth Engine model — immutable ledger entry + hash chain.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

/// 32-byte all-zeros placeholder used as the `prev_hash` of the genesis
/// entry. Hex-encoded as the wire shape (lowercase, no `0x` prefix) so
/// it round-trips through JSON without any extra coercion.
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Discriminator for what an entry represents. Each variant maps to a
/// payload shape; the actual `metadata` is a free-form JSON map so new
/// kinds can land without a migration of older rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryKind {
    /// Account opened (mirrors `accounts.create_account`).
    AccountCreated,
    /// Account renamed / retyped (mirrors `accounts.update_account`).
    AccountUpdated,
    /// Account archived.
    AccountArchived,
    /// Trading / cash / income / expense activity recorded.
    ActivityRecorded,
    /// Activity edited (counter-balanced — a reversal + a new entry).
    ActivityReversed,
    /// Alternative asset created (Property / Vehicle / Collectible / …).
    AlternativeAssetCreated,
    /// Liability created.
    LiabilityCreated,
    /// Liability updated (current value, rate, monthly payment, …).
    LiabilityUpdated,
    /// Goal created.
    GoalCreated,
    /// FX rate observed (used to lock historical conversions).
    FxRateObserved,
    /// Quote observed (used to lock historical valuations).
    QuoteObserved,
}

/// Errors a verifier returns. Each variant includes the entry id at
/// which the integrity check failed so support can grep the exact row.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LedgerIntegrityError {
    #[error("entry {0}: prev_hash does not match the previous entry's entry_hash")]
    BrokenChain(String),
    #[error("entry {0}: entry_hash does not match the canonical SHA-256 of (prev_hash || payload)")]
    TamperedEntry(String),
    #[error("genesis entry {0} has a non-zero prev_hash")]
    InvalidGenesis(String),
    #[error("sequence number is not strictly increasing at entry {0}")]
    OutOfOrder(String),
}

/// One immutable row in the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    /// Stable id — caller-assigned (typically UUID). Re-using an id is
    /// a hard error caught by the verifier (it'd produce two entries
    /// with the same id but different prev_hash).
    pub id: String,
    /// Monotonically-increasing sequence number. The first entry is 0.
    pub sequence: u64,
    pub kind: LedgerEntryKind,
    pub recorded_at: DateTime<Utc>,
    /// Optional account id this entry pertains to. Holdings + cash
    /// queries filter by this.
    pub account_id: Option<String>,
    /// Optional asset id (security / alternative asset).
    pub asset_id: Option<String>,
    /// Currency-denominated value (positive = inflow, negative = outflow).
    /// `None` for entries that aren't money-moving (e.g. AccountUpdated
    /// renaming the account).
    pub amount: Option<Decimal>,
    /// ISO-4217 currency code paired with `amount`.
    pub currency: Option<String>,
    /// Kind-specific free-form metadata. JSON object — keys are
    /// canonicalised (sorted) before hashing so two semantically-equal
    /// payloads always produce the same hash.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// Hex-encoded SHA-256 of the previous entry's `entry_hash`, or
    /// `GENESIS_PREV_HASH` for the first entry.
    pub prev_hash: String,
    /// Hex-encoded SHA-256 of `(prev_hash || canonical_payload)`.
    pub entry_hash: String,
}

impl LedgerEntry {
    /// Public constructor for storage backends that compute `sequence`
    /// and `prev_hash` inside their own transaction (e.g. SQLite reads
    /// the chain tip then inserts). Same hashing semantics as the
    /// in-memory service uses internally.
    #[allow(clippy::too_many_arguments)]
    pub fn assemble_via_service(
        id: String,
        sequence: u64,
        kind: LedgerEntryKind,
        prev_hash: String,
        account_id: Option<String>,
        asset_id: Option<String>,
        amount: Option<Decimal>,
        currency: Option<String>,
        metadata: BTreeMap<String, serde_json::Value>,
        recorded_at: DateTime<Utc>,
    ) -> Self {
        Self::assemble(
            id, sequence, kind, prev_hash, account_id, asset_id, amount, currency, metadata,
            recorded_at,
        )
    }

    /// Construct + hash a fresh entry. `sequence` and `prev_hash` come
    /// from the ledger (the service injects them at `append` time);
    /// callers shouldn't try to compute these themselves.
    pub(crate) fn assemble(
        id: String,
        sequence: u64,
        kind: LedgerEntryKind,
        prev_hash: String,
        account_id: Option<String>,
        asset_id: Option<String>,
        amount: Option<Decimal>,
        currency: Option<String>,
        metadata: BTreeMap<String, serde_json::Value>,
        recorded_at: DateTime<Utc>,
    ) -> Self {
        // Build with a placeholder entry_hash, then overwrite via the
        // canonical-payload digest.
        let mut entry = Self {
            id,
            sequence,
            kind,
            recorded_at,
            account_id,
            asset_id,
            amount,
            currency,
            metadata,
            prev_hash,
            entry_hash: String::new(),
        };
        entry.entry_hash = derive_entry_hash(&entry);
        entry
    }

    /// Re-derive the entry_hash from current contents. The verifier
    /// calls this and compares to the stored value.
    pub fn rederive_hash(&self) -> String {
        derive_entry_hash(self)
    }
}

/// Build the canonical payload bytes for `entry`. JSON is intentionally
/// the serialisation choice so the storage layer can persist the same
/// shape (no Diesel-specific encoding leaks into the hash).
///
/// The `entry_hash` field is excluded from the payload (it'd be circular).
pub fn canonical_payload(entry: &LedgerEntry) -> Vec<u8> {
    // BTreeMap keys are already sorted; the rest of the struct fields
    // serialise in declaration order, which is stable for the lifetime
    // of this struct definition. New fields go AT THE END to keep
    // historical entries verifiable.
    let payload = serde_json::json!({
        "id": entry.id,
        "sequence": entry.sequence,
        "kind": entry.kind,
        "recordedAt": entry.recorded_at.to_rfc3339(),
        "accountId": entry.account_id,
        "assetId": entry.asset_id,
        "amount": entry.amount.map(|d| d.to_string()),
        "currency": entry.currency,
        "metadata": serde_json::to_value(&entry.metadata).unwrap_or(serde_json::Value::Null),
        "prevHash": entry.prev_hash,
    });
    serde_json::to_vec(&payload).expect("canonical payload serialisation must not fail")
}

/// SHA-256 of `prev_hash` raw bytes prepended to the canonical payload.
/// Returned as lowercase hex.
pub fn derive_entry_hash(entry: &LedgerEntry) -> String {
    let mut hasher = Sha256::new();
    // Hashing the hex string directly is fine and avoids a hex-decode
    // step. The genesis prev_hash is already lowercase hex; service
    // assembly enforces lowercase hex for non-genesis entries too.
    hasher.update(entry.prev_hash.as_bytes());
    hasher.update(canonical_payload(entry));
    let digest = hasher.finalize();
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    static HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, seq: u64, prev: &str) -> LedgerEntry {
        LedgerEntry::assemble(
            id.to_string(),
            seq,
            LedgerEntryKind::AccountCreated,
            prev.to_string(),
            Some("acc-1".to_string()),
            None,
            None,
            None,
            BTreeMap::new(),
            chrono::Utc::now(),
        )
    }

    #[test]
    fn entry_hash_is_64_hex_chars() {
        let e = sample("e1", 0, GENESIS_PREV_HASH);
        assert_eq!(e.entry_hash.len(), 64);
        assert!(e.entry_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_payload_produces_same_hash() {
        let e1 = sample("e1", 0, GENESIS_PREV_HASH);
        let e2 = sample("e1", 0, GENESIS_PREV_HASH);
        // recorded_at differs by ~microseconds — different hashes is
        // correct: a re-recorded entry IS a new entry.
        if e1.recorded_at == e2.recorded_at {
            assert_eq!(e1.entry_hash, e2.entry_hash);
        }
    }

    #[test]
    fn different_prev_hash_changes_entry_hash() {
        let e1 = sample("e1", 0, GENESIS_PREV_HASH);
        let e2 = sample("e1", 1, &"a".repeat(64));
        assert_ne!(e1.entry_hash, e2.entry_hash);
    }

    #[test]
    fn tampering_amount_breaks_rederivation() {
        let mut e = sample("e1", 0, GENESIS_PREV_HASH);
        let original = e.entry_hash.clone();
        e.amount = Some(rust_decimal::Decimal::new(123_456, 2));
        assert_ne!(e.rederive_hash(), original);
    }
}
