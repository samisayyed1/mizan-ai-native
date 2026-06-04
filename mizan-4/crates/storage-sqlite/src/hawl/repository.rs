//! `HawlAnchorRepository` — read-only access to `hawl_anchors`.
//!
//! Per Goal v3 §V step C5.d.1: the insights scheduler needs to hydrate
//! `InsightsInput.hawl_anchors_approaching` from the durable
//! `hawl_anchors` state. This repo is the read path; the write path
//! lives in the Zakat engine at evaluation time.
//!
//! Read shape: caller passes the user's `today` (their local-timezone
//! date) and the engine returns rows whose lunar-year completion
//! lands within the maturity-approach window the
//! `eval_hawl_approaching` rule cares about
//! (`mizan-insights::HAWL_DAY_THRESHOLDS` = 1/7/30 days).
//!
//! Plus a small grace window past 90 days so the caller can pre-filter
//! locally — keeps SQL simple, lets the Rust rule pick the actual step.

use chrono::{Duration, NaiveDate};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use log::debug;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;

use crate::db::get_connection;
use crate::schema::hawl_anchors;
use mizan_core::errors::{DatabaseError, Error, Result};

/// One row from `hawl_anchors` plus the caller-computed
/// `days_to_completion`. The repository computes the days value here
/// so the scheduler doesn't have to know the Islamic-lunar-year arithmetic
/// (354 Gregorian days per the migration header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HawlAnchorRow {
    pub cohort_id: String,
    pub user_id: String,
    pub anchor_date: NaiveDate,
    pub current_qualifying_amount: Decimal,
    pub base_currency: String,
    /// Days remaining until lunar-year completion. Negative = passed
    /// (the Zakat engine hasn't yet stamped a new anchor; the row is
    /// returned for visibility but the scheduler typically filters).
    pub days_to_completion: i64,
}

/// Lunar-year length in Gregorian days. 354 is the commonly-used
/// integer approximation of 354.367; the Zakat engine uses the precise
/// decimal at calculation time, but for "approaching" notifications
/// the integer day count is fine.
const LUNAR_YEAR_DAYS: i64 = 354;

/// Approaching-window grace: caller may ask for "next 90 days" by
/// passing `90`; the repo filters rows whose completion falls within
/// `[-1d, +grace_days]`. The negative-side `-1` lets the caller see
/// rows that just-passed today (engine hasn't stamped yet).
pub struct HawlAnchorRepository {
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
}

impl HawlAnchorRepository {
    pub fn new(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>) -> Self {
        Self { pool }
    }

    /// Read rows whose lunar-year completion lands within the next
    /// `grace_days` days of `today`. Sorted ascending by days-remaining
    /// so the smallest-step candidates render first in the bell panel.
    pub fn approaching(&self, today: NaiveDate, grace_days: i64) -> Result<Vec<HawlAnchorRow>> {
        let mut conn = get_connection(&self.pool)?;

        let rows: Vec<(String, String, NaiveDate, String, String)> = hawl_anchors::table
            .select((
                hawl_anchors::cohort_id,
                hawl_anchors::user_id,
                hawl_anchors::anchor_date,
                hawl_anchors::current_qualifying_amount,
                hawl_anchors::base_currency,
            ))
            .load::<(String, String, NaiveDate, String, String)>(&mut conn)
            .map_err(|e| Error::Database(DatabaseError::QueryFailed(e.to_string())))?;

        let mut out = Vec::with_capacity(rows.len());
        let lower = today - Duration::days(1);
        let upper = today + Duration::days(grace_days);

        for (cohort_id, user_id, anchor_date, amount_text, base_currency) in rows {
            // Lunar year ends at anchor_date + 354 days. If that falls
            // outside the window, skip the row (cheap CPU filter; the
            // table is small per the migration's per-cohort design).
            let completion = anchor_date + Duration::days(LUNAR_YEAR_DAYS);
            if completion < lower || completion > upper {
                continue;
            }
            let days_to_completion = (completion - today).num_days();

            let qualifying_amount = match Decimal::from_str(&amount_text) {
                Ok(d) => d,
                Err(e) => {
                    debug!(
                        "Insights/Hawl: skipping cohort {} — current_qualifying_amount parse failed: {}",
                        cohort_id, e
                    );
                    continue;
                }
            };

            out.push(HawlAnchorRow {
                cohort_id,
                user_id,
                anchor_date,
                current_qualifying_amount: qualifying_amount,
                base_currency,
                days_to_completion,
            });
        }

        // Sort smallest days first so the bell panel renders the
        // most-urgent first when the scheduler converts to
        // HawlAnchorCandidate.
        out.sort_by_key(|r| r.days_to_completion);

        Ok(out)
    }
}
