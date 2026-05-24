//! SQLite-backed §A12 NetWorthSnapshotService.

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::prelude::*;
use std::sync::Arc;
use std::str::FromStr;

use rust_decimal::Decimal;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::net_worth_snapshots::dsl as nw_dsl;
use mizan_core::errors::{Error, Result};
use mizan_core::net_worth_snapshot::{
    NetWorthBreakdownEntry, NetWorthSnapshot, NetWorthSnapshotInput, NetWorthSnapshotService,
    SnapshotSource,
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::net_worth_snapshots)]
struct NetWorthSnapshotDB {
    snapshot_date: String,
    base_currency: String,
    total_assets: String,
    total_liabilities: String,
    net_worth: String,
    breakdown_json: String,
    source: String,
    captured_at: i64,
}

fn source_from_str(s: &str) -> SnapshotSource {
    match s {
        "scheduler" => SnapshotSource::Scheduler,
        "app_open" => SnapshotSource::AppOpen,
        "manual_trigger" => SnapshotSource::ManualTrigger,
        "backfill" => SnapshotSource::Backfill,
        _ => SnapshotSource::Scheduler,
    }
}

fn db_to_domain(r: NetWorthSnapshotDB) -> NetWorthSnapshot {
    let breakdown: Vec<NetWorthBreakdownEntry> =
        serde_json::from_str(&r.breakdown_json).unwrap_or_default();
    NetWorthSnapshot {
        snapshot_date: NaiveDate::parse_from_str(&r.snapshot_date, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Utc::now().date_naive()),
        base_currency: r.base_currency,
        total_assets: Decimal::from_str(&r.total_assets).unwrap_or_default(),
        total_liabilities: Decimal::from_str(&r.total_liabilities).unwrap_or_default(),
        net_worth: Decimal::from_str(&r.net_worth).unwrap_or_default(),
        breakdown,
        source: source_from_str(&r.source),
        recorded_at: chrono::TimeZone::timestamp_millis_opt(&chrono::Utc, r.captured_at)
            .single()
            .unwrap_or_else(chrono::Utc::now),
    }
}

fn domain_to_db(s: &NetWorthSnapshot) -> NetWorthSnapshotDB {
    NetWorthSnapshotDB {
        snapshot_date: s.snapshot_date.format("%Y-%m-%d").to_string(),
        base_currency: s.base_currency.clone(),
        total_assets: s.total_assets.to_string(),
        total_liabilities: s.total_liabilities.to_string(),
        net_worth: s.net_worth.to_string(),
        breakdown_json: serde_json::to_string(&s.breakdown).unwrap_or_else(|_| "[]".to_string()),
        source: s.source.as_str().to_string(),
        captured_at: s.recorded_at.timestamp_millis(),
    }
}

pub struct SqliteNetWorthSnapshotService {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl SqliteNetWorthSnapshotService {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl NetWorthSnapshotService for SqliteNetWorthSnapshotService {
    async fn upsert(&self, input: NetWorthSnapshotInput) -> Result<NetWorthSnapshot> {
        let snapshot = input.into_snapshot();
        let row = domain_to_db(&snapshot);
        let snapshot_clone = snapshot.clone();

        self.writer
            .exec(move |conn| -> Result<()> {
                diesel::insert_into(nw_dsl::net_worth_snapshots)
                    .values(&row)
                    .on_conflict((nw_dsl::snapshot_date, nw_dsl::base_currency))
                    .do_update()
                    .set((
                        nw_dsl::total_assets.eq(&row.total_assets),
                        nw_dsl::total_liabilities.eq(&row.total_liabilities),
                        nw_dsl::net_worth.eq(&row.net_worth),
                        nw_dsl::breakdown_json.eq(&row.breakdown_json),
                        nw_dsl::source.eq(&row.source),
                        nw_dsl::captured_at.eq(row.captured_at),
                    ))
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;
        Ok(snapshot_clone)
    }

    async fn range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<NetWorthSnapshot>> {
        let pool = Arc::clone(&self.pool);
        let from_s = from.format("%Y-%m-%d").to_string();
        let to_s = to.format("%Y-%m-%d").to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let rows: Vec<NetWorthSnapshotDB> = nw_dsl::net_worth_snapshots
                .filter(nw_dsl::snapshot_date.ge(from_s))
                .filter(nw_dsl::snapshot_date.le(to_s))
                .order(nw_dsl::snapshot_date.asc())
                .select(NetWorthSnapshotDB::as_select())
                .load::<NetWorthSnapshotDB>(&mut conn)
                .map_err(StorageError::from)?;
            Ok(rows.into_iter().map(db_to_domain).collect())
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn latest(&self) -> Result<Option<NetWorthSnapshot>> {
        let pool = Arc::clone(&self.pool);
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let row = nw_dsl::net_worth_snapshots
                .order(nw_dsl::snapshot_date.desc())
                .select(NetWorthSnapshotDB::as_select())
                .first::<NetWorthSnapshotDB>(&mut conn)
                .optional()
                .map_err(StorageError::from)?;
            Ok(row.map(db_to_domain))
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }

    async fn get(&self, date: NaiveDate) -> Result<Option<NetWorthSnapshot>> {
        let pool = Arc::clone(&self.pool);
        let date_s = date.format("%Y-%m-%d").to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = get_connection(&pool)?;
            let row = nw_dsl::net_worth_snapshots
                .filter(nw_dsl::snapshot_date.eq(date_s))
                .select(NetWorthSnapshotDB::as_select())
                .first::<NetWorthSnapshotDB>(&mut conn)
                .optional()
                .map_err(StorageError::from)?;
            Ok(row.map(db_to_domain))
        })
        .await
        .map_err(|e| Error::Unexpected(format!("spawn_blocking join error: {e}")))?
    }
}
