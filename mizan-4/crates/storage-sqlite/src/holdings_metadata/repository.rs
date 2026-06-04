//! `HoldingsMetadataRepository` — read-only access to the
//! `holdings_metadata` table for insights hydration.
//!
//! Today this exposes the `recent_sharia_flips` reader for Track C
//! PR-C5.d.5. Other columns (`origin`, `ai_estimated`, `tags`,
//! `advisor_reviewed_at`, `agent_modified_at`) are read by the badge
//! renderer through separate paths; only insights need the flip
//! detection here.

use chrono::NaiveDate;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::get_connection;
use crate::schema::holdings_metadata;
use mizan_core::errors::{DatabaseError, Error, Result};

/// One detected Sharia-status flip between yesterday's verdict and
/// today's. The scheduler converts these into
/// `mizan_insights::ShariaStatusChange`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShariaStatusFlip {
    pub account_id: String,
    pub holding_symbol: String,
    pub prior_verdict: String,
    pub new_verdict: String,
}

pub struct HoldingsMetadataRepository {
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
}

impl HoldingsMetadataRepository {
    pub fn new(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>) -> Self {
        Self { pool }
    }

    /// Detect Sharia-status flips by comparing each holding's
    /// `sharia_status` on `today` against its most-recent non-NULL
    /// value **before** `today`.
    ///
    /// Skips rows where either side is NULL (the holding wasn't
    /// screened previously, or hasn't been screened today) — those
    /// are first-time-screened rather than "flipped".
    ///
    /// Returns one flip per `(account_id, holding_symbol)` whose
    /// verdict actually changed.
    pub fn recent_sharia_flips(&self, today: NaiveDate) -> Result<Vec<ShariaStatusFlip>> {
        let mut conn = get_connection(&self.pool)?;

        // Pull every screened row up through `today`. The table is
        // small (one row per (account_id, holding_symbol, as_of_date))
        // and the scheduler reads only on insights ticks, so a single
        // table scan + Rust grouping is the simplest correct shape.
        let rows: Vec<(String, String, NaiveDate, Option<String>)> = holdings_metadata::table
            .select((
                holdings_metadata::account_id,
                holdings_metadata::holding_symbol,
                holdings_metadata::as_of_date,
                holdings_metadata::sharia_status,
            ))
            .filter(holdings_metadata::as_of_date.le(today))
            .filter(holdings_metadata::sharia_status.is_not_null())
            .load::<(String, String, NaiveDate, Option<String>)>(&mut conn)
            .map_err(|e| Error::Database(DatabaseError::QueryFailed(e.to_string())))?;

        Ok(detect_flips(&rows, today))
    }
}

/// Pure-math helper for `recent_sharia_flips` — testable without a
/// pool. Groups rows by `(account_id, holding_symbol)`, finds the
/// latest verdict ≤ `today` per holding and the most-recent prior
/// verdict (date strictly before that latest date), emits a flip if
/// the two verdicts differ.
pub fn detect_flips(
    rows: &[(String, String, NaiveDate, Option<String>)],
    today: NaiveDate,
) -> Vec<ShariaStatusFlip> {
    let mut by_holding: HashMap<(String, String), Vec<(NaiveDate, String)>> = HashMap::new();
    for (account_id, symbol, as_of, status) in rows {
        if *as_of > today {
            continue;
        }
        let Some(verdict) = status else { continue };
        let key = (account_id.clone(), symbol.clone());
        by_holding
            .entry(key)
            .or_default()
            .push((*as_of, verdict.clone()));
    }

    let mut out = Vec::new();
    for ((account_id, holding_symbol), mut history) in by_holding {
        // Sort descending by date — latest first.
        history.sort_by_key(|r| std::cmp::Reverse(r.0));
        let Some((latest_date, latest_verdict)) = history.first().cloned() else {
            continue;
        };
        // Prior = the most-recent row strictly before latest_date with
        // a different verdict.
        let prior = history
            .iter()
            .skip(1)
            .find(|(date, verdict)| *date < latest_date && verdict != &latest_verdict);
        let Some((_, prior_verdict)) = prior else {
            continue;
        };
        out.push(ShariaStatusFlip {
            account_id,
            holding_symbol,
            prior_verdict: prior_verdict.clone(),
            new_verdict: latest_verdict,
        });
    }

    // Stable order so the scheduler's dedupe-key cache stays stable
    // across ticks — sort by (account_id, holding_symbol).
    out.sort_by(|a, b| {
        a.account_id
            .cmp(&b.account_id)
            .then(a.holding_symbol.cmp(&b.holding_symbol))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        account: &str,
        symbol: &str,
        date: (i32, u32, u32),
        verdict: Option<&str>,
    ) -> (String, String, NaiveDate, Option<String>) {
        (
            account.into(),
            symbol.into(),
            NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            verdict.map(|s| s.to_string()),
        )
    }

    #[test]
    fn empty_input_returns_empty() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        assert!(detect_flips(&[], today).is_empty());
    }

    #[test]
    fn single_row_no_history_returns_empty() {
        // First-time screening — no flip, just a new verdict
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![row("acc1", "SPUS", (2026, 6, 4), Some("compliant"))];
        assert!(detect_flips(&rows, today).is_empty());
    }

    #[test]
    fn same_verdict_across_rows_no_flip() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 5, 28), Some("compliant")),
            row("acc1", "SPUS", (2026, 6, 4), Some("compliant")),
        ];
        assert!(detect_flips(&rows, today).is_empty());
    }

    #[test]
    fn detects_compliant_to_mixed_flip() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 5, 28), Some("compliant")),
            row("acc1", "SPUS", (2026, 6, 4), Some("mixed")),
        ];
        let flips = detect_flips(&rows, today);
        assert_eq!(flips.len(), 1);
        assert_eq!(flips[0].account_id, "acc1");
        assert_eq!(flips[0].holding_symbol, "SPUS");
        assert_eq!(flips[0].prior_verdict, "compliant");
        assert_eq!(flips[0].new_verdict, "mixed");
    }

    #[test]
    fn future_rows_filtered() {
        // Caller may pass in future-dated rows; helper ignores them.
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 5, 28), Some("compliant")),
            row("acc1", "SPUS", (2026, 7, 1), Some("mixed")),
        ];
        // Without the future row, only the single past entry remains
        // → no flip.
        assert!(detect_flips(&rows, today).is_empty());
    }

    #[test]
    fn multiple_holdings_emit_independently() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 5, 28), Some("compliant")),
            row("acc1", "SPUS", (2026, 6, 4), Some("mixed")),
            row("acc1", "WAHED", (2026, 5, 28), Some("mixed")),
            row("acc1", "WAHED", (2026, 6, 4), Some("compliant")),
        ];
        let flips = detect_flips(&rows, today);
        assert_eq!(flips.len(), 2);
        // Sorted by symbol — SPUS before WAHED
        assert_eq!(flips[0].holding_symbol, "SPUS");
        assert_eq!(flips[1].holding_symbol, "WAHED");
        assert_eq!(flips[1].prior_verdict, "mixed");
        assert_eq!(flips[1].new_verdict, "compliant");
    }

    #[test]
    fn back_and_forth_flip_returns_latest_transition() {
        // compliant → mixed → compliant; the latest row is compliant,
        // the most recent prior with a different verdict is mixed.
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "X", (2026, 5, 15), Some("compliant")),
            row("acc1", "X", (2026, 5, 28), Some("mixed")),
            row("acc1", "X", (2026, 6, 4), Some("compliant")),
        ];
        let flips = detect_flips(&rows, today);
        assert_eq!(flips.len(), 1);
        assert_eq!(flips[0].prior_verdict, "mixed");
        assert_eq!(flips[0].new_verdict, "compliant");
    }

    #[test]
    fn unsorted_input_handled() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 6, 4), Some("mixed")),
            row("acc1", "SPUS", (2026, 5, 28), Some("compliant")),
        ];
        let flips = detect_flips(&rows, today);
        assert_eq!(flips.len(), 1);
        assert_eq!(flips[0].prior_verdict, "compliant");
        assert_eq!(flips[0].new_verdict, "mixed");
    }

    #[test]
    fn null_verdicts_skipped() {
        // Caller may pass NULL rows even though the SQL filter
        // excludes them — defensive guard.
        let today = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let rows = vec![
            row("acc1", "SPUS", (2026, 5, 28), None),
            row("acc1", "SPUS", (2026, 6, 4), Some("compliant")),
        ];
        // The single non-null row has no prior → no flip
        assert!(detect_flips(&rows, today).is_empty());
    }
}
