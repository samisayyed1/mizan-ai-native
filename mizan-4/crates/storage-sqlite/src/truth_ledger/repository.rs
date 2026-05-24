//! SQLite-backed §A1/§A2 TruthLedger.
//!
//! Hash-chain integrity across restarts is enforced by reading the
//! previous (sequence, entry_hash) tip inside the same write transaction
//! as the insert. The write actor serialises writes so two concurrent
//! appends can't observe the same tip and produce a fork.
//!
//! `verify()` walks the entire table in `sequence` order and re-derives
//! every hash — detects both tampering and broken chains.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::truth_ledger_entries::dsl as ledger_dsl;
use mizan_core::errors::{Error, Result, ValidationError};
use mizan_core::truth_engine::{
    derive_entry_hash, AppendInput, LedgerEntry, LedgerEntryKind, LedgerIntegrityError,
    TruthLedger, GENESIS_PREV_HASH,
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::truth_ledger_entries)]
struct TruthLedgerEntryDB {
    id: String,
    sequence: i64,
    kind: String,
    recorded_at: i64,
    account_id: Option<String>,
    asset_id: Option<String>,
    amount: Option<String>,
    currency: Option<String>,
    metadata_json: String,
    prev_hash: String,
    entry_hash: String,
}

fn kind_to_str(k: LedgerEntryKind) -> &'static str {
    match k {
        LedgerEntryKind::AccountCreated => "account_created",
        LedgerEntryKind::AccountUpdated => "account_updated",
        LedgerEntryKind::AccountArchived => "account_archived",
        LedgerEntryKind::ActivityRecorded => "activity_recorded",
        LedgerEntryKind::ActivityReversed => "activity_reversed",
        LedgerEntryKind::AlternativeAssetCreated => "alternative_asset_created",
        LedgerEntryKind::LiabilityCreated => "liability_created",
        LedgerEntryKind::LiabilityUpdated => "liability_updated",
        LedgerEntryKind::GoalCreated => "goal_created",
        LedgerEntryKind::FxRateObserved => "fx_rate_observed",
        LedgerEntryKind::QuoteObserved => "quote_observed",
    }
}

fn kind_from_str(s: &str) -> Option<LedgerEntryKind> {
    Some(match s {
        "account_created" => LedgerEntryKind::AccountCreated,
        "account_updated" => LedgerEntryKind::AccountUpdated,
        "account_archived" => LedgerEntryKind::AccountArchived,
        "activity_recorded" => LedgerEntryKind::ActivityRecorded,
        "activity_reversed" => LedgerEntryKind::ActivityReversed,
        "alternative_asset_created" => LedgerEntryKind::AlternativeAssetCreated,
        "liability_created" => LedgerEntryKind::LiabilityCreated,
        "liability_updated" => LedgerEntryKind::LiabilityUpdated,
        "goal_created" => LedgerEntryKind::GoalCreated,
        "fx_rate_observed" => LedgerEntryKind::FxRateObserved,
        "quote_observed" => LedgerEntryKind::QuoteObserved,
        _ => return None,
    })
}

fn millis_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

/// Translate a stored row into the domain `LedgerEntry`. Returns the
/// row's id alongside an Err so callers can surface which row blew up.
fn db_to_domain(row: &TruthLedgerEntryDB) -> std::result::Result<LedgerEntry, String> {
    let kind = kind_from_str(&row.kind)
        .ok_or_else(|| format!("unknown kind '{}' at row id={}", row.kind, row.id))?;
    let metadata: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&row.metadata_json).unwrap_or_default();
    let amount = match &row.amount {
        Some(s) => Some(
            Decimal::from_str(s)
                .map_err(|e| format!("amount parse failed at id={}: {e}", row.id))?,
        ),
        None => None,
    };
    Ok(LedgerEntry {
        id: row.id.clone(),
        sequence: row.sequence as u64,
        kind,
        recorded_at: millis_to_dt(row.recorded_at),
        account_id: row.account_id.clone(),
        asset_id: row.asset_id.clone(),
        amount,
        currency: row.currency.clone(),
        metadata,
        prev_hash: row.prev_hash.clone(),
        entry_hash: row.entry_hash.clone(),
    })
}

fn domain_to_db(e: &LedgerEntry) -> TruthLedgerEntryDB {
    TruthLedgerEntryDB {
        id: e.id.clone(),
        sequence: e.sequence as i64,
        kind: kind_to_str(e.kind).to_string(),
        recorded_at: e.recorded_at.timestamp_millis(),
        account_id: e.account_id.clone(),
        asset_id: e.asset_id.clone(),
        amount: e.amount.map(|d| d.to_string()),
        currency: e.currency.clone(),
        metadata_json: serde_json::to_string(&e.metadata).unwrap_or_else(|_| "{}".to_string()),
        prev_hash: e.prev_hash.clone(),
        entry_hash: e.entry_hash.clone(),
    }
}

pub struct SqliteTruthLedger {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl SqliteTruthLedger {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl TruthLedger for SqliteTruthLedger {
    async fn append(&self, input: AppendInput) -> Result<LedgerEntry> {
        let kind = input
            .kind
            .ok_or_else(|| Error::Validation(ValidationError::InvalidInput(
                "LedgerEntry kind is required".to_string(),
            )))?;
        if input.id.trim().is_empty() {
            return Err(Error::Validation(ValidationError::InvalidInput(
                "LedgerEntry id cannot be empty".to_string(),
            )));
        }

        let id = input.id.clone();
        let account_id = input.account_id.clone();
        let asset_id = input.asset_id.clone();
        let amount = input.amount;
        let currency = input.currency.clone();
        let metadata = input.metadata.clone();
        let recorded_at = input.recorded_at.unwrap_or_else(Utc::now);

        let entry = self
            .writer
            .exec(move |conn| -> Result<LedgerEntry> {
                // Reject id reuse — append-only.
                let exists: i64 = ledger_dsl::truth_ledger_entries
                    .filter(ledger_dsl::id.eq(&id))
                    .count()
                    .get_result::<i64>(conn)
                    .map_err(StorageError::from)?;
                if exists > 0 {
                    return Err(Error::Validation(ValidationError::InvalidInput(format!(
                        "LedgerEntry id {} already exists; ledger is append-only",
                        id
                    ))));
                }

                // Read the chain tip inside the same transaction so two
                // concurrent appenders can't both observe the same prev.
                let tip: Option<(i64, String)> = ledger_dsl::truth_ledger_entries
                    .order(ledger_dsl::sequence.desc())
                    .select((ledger_dsl::sequence, ledger_dsl::entry_hash))
                    .first::<(i64, String)>(conn)
                    .optional()
                    .map_err(StorageError::from)?;

                let (sequence, prev_hash) = match tip {
                    Some((seq, hash)) => ((seq as u64) + 1, hash),
                    None => (0u64, GENESIS_PREV_HASH.to_string()),
                };

                // Build the entry (computes entry_hash from the canonical
                // payload + prev_hash).
                let domain = LedgerEntry::assemble_via_service(
                    id,
                    sequence,
                    kind,
                    prev_hash,
                    account_id,
                    asset_id,
                    amount,
                    currency,
                    metadata,
                    recorded_at,
                );
                let row = domain_to_db(&domain);
                diesel::insert_into(ledger_dsl::truth_ledger_entries)
                    .values(&row)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(domain)
            })
            .await?;

        Ok(entry)
    }

    async fn verify(&self) -> std::result::Result<(), LedgerIntegrityError> {
        let pool = Arc::clone(&self.pool);
        tokio::task::spawn_blocking(move || -> std::result::Result<(), LedgerIntegrityError> {
            let mut conn = get_connection(&pool).map_err(|e| {
                LedgerIntegrityError::TamperedEntry(format!("db connection failed: {e}"))
            })?;
            let rows: Vec<TruthLedgerEntryDB> = ledger_dsl::truth_ledger_entries
                .order(ledger_dsl::sequence.asc())
                .select(TruthLedgerEntryDB::as_select())
                .load::<TruthLedgerEntryDB>(&mut conn)
                .map_err(|e| LedgerIntegrityError::TamperedEntry(format!("load failed: {e}")))?;

            let mut expected_prev = GENESIS_PREV_HASH.to_string();
            let mut expected_seq: u64 = 0;
            for row in rows.iter() {
                let entry = db_to_domain(row).map_err(LedgerIntegrityError::TamperedEntry)?;
                if entry.sequence != expected_seq {
                    return Err(LedgerIntegrityError::OutOfOrder(entry.id.clone()));
                }
                if entry.prev_hash != expected_prev {
                    if expected_seq == 0 {
                        return Err(LedgerIntegrityError::InvalidGenesis(entry.id.clone()));
                    }
                    return Err(LedgerIntegrityError::BrokenChain(entry.id.clone()));
                }
                if derive_entry_hash(&entry) != entry.entry_hash {
                    return Err(LedgerIntegrityError::TamperedEntry(entry.id.clone()));
                }
                expected_prev = entry.entry_hash.clone();
                expected_seq += 1;
            }
            Ok(())
        })
        .await
        .map_err(|e| {
            LedgerIntegrityError::TamperedEntry(format!("spawn_blocking join error: {e}"))
        })?
    }

    async fn all(&self, limit: Option<usize>) -> Result<Vec<LedgerEntry>> {
        let pool = Arc::clone(&self.pool);
        let limit = limit.map(|n| n as i64);
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let mut q = ledger_dsl::truth_ledger_entries
                .order(ledger_dsl::sequence.asc())
                .into_boxed();
            if let Some(l) = limit {
                q = q.limit(l);
            }
            let rows: Vec<TruthLedgerEntryDB> = q
                .select(TruthLedgerEntryDB::as_select())
                .load::<TruthLedgerEntryDB>(&mut conn)
                .map_err(StorageError::from)?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows.iter() {
                let entry =
                    db_to_domain(row).map_err(|e| Error::Unexpected(format!("decode: {e}")))?;
                out.push(entry);
            }
            Ok(out)
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn by_account(&self, account_id: &str) -> Result<Vec<LedgerEntry>> {
        let pool = Arc::clone(&self.pool);
        let account_id = account_id.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let rows: Vec<TruthLedgerEntryDB> = ledger_dsl::truth_ledger_entries
                .filter(ledger_dsl::account_id.eq(account_id))
                .order(ledger_dsl::sequence.asc())
                .select(TruthLedgerEntryDB::as_select())
                .load::<TruthLedgerEntryDB>(&mut conn)
                .map_err(StorageError::from)?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows.iter() {
                let entry =
                    db_to_domain(row).map_err(|e| Error::Unexpected(format!("decode: {e}")))?;
                out.push(entry);
            }
            Ok(out)
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }
}
