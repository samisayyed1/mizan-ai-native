//! SQLite-backed §A12 NetWorthSnapshotService.

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::prelude::*;
use std::str::FromStr;
use std::sync::Arc;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::write_actor::spawn_writer;
    use crate::db::{create_pool, init, run_migrations};
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    fn build() -> (SqliteNetWorthSnapshotService, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = init(&dir.path().to_string_lossy()).unwrap();
        run_migrations(&db_path).unwrap();
        let pool = create_pool(&db_path).unwrap();
        let writer = spawn_writer(pool.as_ref().clone()).unwrap();
        (SqliteNetWorthSnapshotService::new(pool, writer), dir)
    }

    fn sample(date: NaiveDate, assets: i64, liab: i64) -> NetWorthSnapshotInput {
        NetWorthSnapshotInput {
            snapshot_date: date,
            base_currency: "USD".to_string(),
            total_assets: Decimal::from(assets),
            total_liabilities: Decimal::from(liab),
            breakdown: vec![
                NetWorthBreakdownEntry {
                    key: "SECURITIES".to_string(),
                    value: Decimal::from(assets),
                },
                NetWorthBreakdownEntry {
                    key: "LIABILITY".to_string(),
                    value: Decimal::from(liab),
                },
            ],
            source: SnapshotSource::AppOpen,
        }
    }

    #[tokio::test]
    async fn upsert_replaces_same_day_row() {
        let (svc, _dir) = build();
        let d = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();

        svc.upsert(sample(d, 100_000, 20_000)).await.unwrap();
        let after_first = svc.range(d, d).await.unwrap();
        assert_eq!(after_first.len(), 1);
        assert_eq!(after_first[0].net_worth, dec!(80_000));

        // Re-upsert same date with different totals — must replace, not append.
        svc.upsert(sample(d, 110_000, 20_000)).await.unwrap();
        let after_second = svc.range(d, d).await.unwrap();
        assert_eq!(after_second.len(), 1, "same-date upsert must replace");
        assert_eq!(after_second[0].net_worth, dec!(90_000));
    }

    #[tokio::test]
    async fn range_returns_sorted_window_inclusive() {
        let (svc, _dir) = build();
        for day in [10, 12, 14, 16] {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample(d, day as i64 * 1_000, 0)).await.unwrap();
        }
        let got = svc
            .range(
                NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
                NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].snapshot_date < got[1].snapshot_date);
    }

    #[tokio::test]
    async fn latest_returns_most_recent_row() {
        let (svc, _dir) = build();
        for day in [10, 11, 12] {
            let d = NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
            svc.upsert(sample(d, 1_000, 0)).await.unwrap();
        }
        let latest = svc.latest().await.unwrap().unwrap();
        use chrono::Datelike;
        assert_eq!(latest.snapshot_date.day(), 12);
    }

    #[tokio::test]
    async fn breakdown_round_trips_through_json_blob() {
        let (svc, _dir) = build();
        let d = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let input = NetWorthSnapshotInput {
            snapshot_date: d,
            base_currency: "USD".to_string(),
            total_assets: dec!(50_000),
            total_liabilities: dec!(10_000),
            breakdown: vec![
                NetWorthBreakdownEntry {
                    key: "SECURITIES".into(),
                    value: dec!(30_000),
                },
                NetWorthBreakdownEntry {
                    key: "CASH".into(),
                    value: dec!(20_000),
                },
                NetWorthBreakdownEntry {
                    key: "LIABILITY".into(),
                    value: dec!(10_000),
                },
            ],
            source: SnapshotSource::AppOpen,
        };
        svc.upsert(input).await.unwrap();
        let got = svc.get(d).await.unwrap().unwrap();
        assert_eq!(got.breakdown.len(), 3);
        assert_eq!(got.breakdown[0].key, "SECURITIES");
        assert_eq!(got.breakdown[0].value, dec!(30_000));
    }
}
