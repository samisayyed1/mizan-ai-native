//! Financial Truth Engine — §A1 + §A2 foundation.
//!
//! Per MIZAN_AI_NATIVE_PLAN.md v3.1 §A1 (Financial Truth Engine) and
//! §A2 (Immutable Ledger + Derived State): every state-changing
//! operation appends one or more entries to an immutable, hash-chained
//! ledger. Holdings, balances, performance, net worth — every "what's
//! my X?" answer is computed by replaying the ledger, not by reading
//! a mutable cell.
//!
//! This module ships the model + append API + verification + tests.
//! Full integration (activities-write → ledger, holdings-read ← ledger,
//! reconciliation engine) lands in dedicated follow-on PRs because
//! each is a deep migration of an existing surface.
//!
//! ## Invariants (enforced here)
//!
//! 1. Entries are **append-only**. The trait has no mutate/delete API.
//! 2. Each entry carries a `prev_hash` pointing at the previous entry's
//!    `entry_hash`. The very first entry's `prev_hash` is all-zeros.
//! 3. `entry_hash` = SHA-256 over `(prev_hash || canonical_payload)`.
//!    Canonical = JSON with sorted keys; we use serde_json::to_vec on
//!    a `BTreeMap`-backed envelope so the bytes are deterministic.
//! 4. Verification walks the chain and re-derives each `entry_hash`.
//!    Any mismatch returns a precise `LedgerIntegrityError` naming
//!    the entry id at which the chain broke.

mod model;
mod service;

pub use model::{
    canonical_payload, derive_entry_hash, LedgerEntry, LedgerEntryKind, LedgerIntegrityError,
    GENESIS_PREV_HASH,
};
pub use service::{AppendInput, InMemoryTruthLedger, TruthLedger, TruthLedgerRetryQueue};
