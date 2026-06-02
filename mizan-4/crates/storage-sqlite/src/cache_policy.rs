//! Cache policy registry — single source of truth for cache table TTLs
//! and eviction policies across the desktop SQLite store.
//!
//! Per `docs/working-agreement.md` §10 (Database Discipline) and §19.7
//! (Cache Eviction Workers): every cache table in SQLite must have an
//! explicit TTL declared here. The CI lint at `scripts/lint-cache-policy.sh`
//! (added in PR-I1.b) walks the schema for any table not registered here
//! and fails the build.
//!
//! ## Why this matters
//!
//! The working agreement past-bug history (§13) records multiple silent-fallback
//! and stale-data bugs (QA Pass 3 date parser, QA Pass 8 silent FX, QA Pass 11
//! cash-totals consistency). A cache table without a declared TTL is a
//! silent-staleness landmine: data gets older, downstream reads silently
//! consume bad data, and the bug doesn't surface until a user notices a wrong
//! number on screen.
//!
//! The registry enforces the rule at compile-time / build-time:
//! - Every cache table appears in `CACHE_POLICIES`
//! - The eviction worker (`cache_eviction.rs`, PR-I2) reads the registry
//!   on startup and on the daily 3am sweep
//! - Migrations touching a registered cache table emit a manifest comment
//!   `-- caches-evicted: [list]` (CI lint enforces this)
//!
//! ## Adding a new cache table
//!
//! 1. Create the table in a new migration
//! 2. Add the migration's `-- caches-evicted: <table_name>` manifest comment
//! 3. Add a `CachePolicy` entry to `CACHE_POLICIES` below
//! 4. The eviction worker auto-picks it up — no code change needed elsewhere

use std::time::Duration;

/// How a cache entry's age is calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeFrom {
    /// The row's `created_at` column.
    CreatedAt,
    /// The row's `updated_at` column.
    UpdatedAt,
    /// The row's domain-specific timestamp column (named in `CachePolicy::age_column`).
    Custom,
}

/// How a cache table is evicted when entries exceed their TTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionStrategy {
    /// Delete expired rows. Default for transient caches.
    Delete,
    /// Roll up expired rows into an aggregate before deleting raw rows.
    /// Used for `projection_snapshots`-style tables where the latest 30d
    /// is full-resolution and older rows are kept as monthly aggregates.
    RollupThenDelete,
    /// Move expired rows to cold storage in Mizan Connect, then delete locally.
    /// Used for `agent_audit_log` per working agreement §18.3 (12-month
    /// full-resolution retention, archive beyond).
    ArchiveThenDelete,
    /// Keep all rows; recompute on read. Used when "stale" is a real state
    /// that surfaces via Mizan Badge `'stale'` rather than being purged.
    KeepMarkStale,
}

/// One row in the cache policy registry.
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// SQLite table name.
    pub table: &'static str,
    /// How long a cache entry is considered fresh.
    pub ttl: Duration,
    /// Which column determines an entry's age.
    pub age_from: AgeFrom,
    /// Column name when `age_from == Custom`. Empty otherwise.
    pub age_column: &'static str,
    /// What to do when an entry expires.
    pub eviction: EvictionStrategy,
    /// Human-readable purpose, surfaced in support diagnostic bundles
    /// (§A17) and in admin tooling.
    pub purpose: &'static str,
}

/// The registry. Every cache table in the SQLite store appears here.
///
/// **Authoritative.** Any table missing from this list is rejected by the
/// CI lint at `scripts/lint-cache-policy.sh`. Any migration creating a
/// cache table must add a matching entry here in the same PR.
///
/// Entries grouped by track (per `docs/plans/00-master-plan.md`) so the
/// review surface is obvious when adding new cache tables for a track.
pub const CACHE_POLICIES: &[CachePolicy] = &[
    // ── Market data caches (existing) ─────────────────────────────────────
    CachePolicy {
        table: "quotes",
        ttl: Duration::from_secs(15),
        age_from: AgeFrom::Custom,
        age_column: "as_of",
        eviction: EvictionStrategy::KeepMarkStale,
        purpose: "Intraday market-data quotes (Twelve Data). 15s freshness target during market hours; stale-mark via Mizan Badge `'stale'` outside.",
    },
    CachePolicy {
        table: "fx_rates",
        ttl: Duration::from_secs(60),
        age_from: AgeFrom::Custom,
        age_column: "as_of",
        eviction: EvictionStrategy::KeepMarkStale,
        purpose: "FX rate snapshots. Historical rows immutable (per §A1/§A2 truth-ledger property); only the 'latest' row evicted on TTL.",
    },
    // ── Investor brief + insights digest (existing) ───────────────────────
    CachePolicy {
        table: "daily_brief_runs",
        ttl: Duration::from_secs(24 * 60 * 60),
        age_from: AgeFrom::CreatedAt,
        age_column: "",
        eviction: EvictionStrategy::Delete,
        purpose: "Investor Daily Brief generated artifacts. Retain 24h then drop; user can re-request.",
    },
    // ── Sync run ledger (append-only, but TTL-bounded for size) ──────────
    CachePolicy {
        table: "sync_run_ledger",
        ttl: Duration::from_secs(90 * 24 * 60 * 60),
        age_from: AgeFrom::CreatedAt,
        age_column: "",
        eviction: EvictionStrategy::ArchiveThenDelete,
        purpose: "Sync run audit log (Plaid/SnapTrade/CSV/Yahoo). 90d local retention; older archived to Mizan Connect.",
    },
    // ── Market news (multi-source news mesh cache) ───────────────────────
    CachePolicy {
        table: "market_news",
        ttl: Duration::from_secs(30 * 24 * 60 * 60),
        age_from: AgeFrom::Custom,
        age_column: "published_at",
        eviction: EvictionStrategy::Delete,
        purpose: "Local cache for multi-source market news (deduplicated by SHA-256 id_hash). 30d retention; older items dropped — fresh news always available from cloud worker (Track D).",
    },
    // ── Tracks C/D — activated by the 2026-06-02 migration batch ────────
    CachePolicy {
        table: "user_memory",
        ttl: Duration::from_secs(365 * 24 * 60 * 60),
        age_from: AgeFrom::Custom,
        age_column: "expires_at",
        eviction: EvictionStrategy::Delete,
        purpose: "Long-term user memory (vector store). Per-row expires_at honours source: 'agent-inferred' 12mo, 'user-stated' never.",
    },
    CachePolicy {
        table: "news_items",
        ttl: Duration::from_secs(90 * 24 * 60 * 60),
        age_from: AgeFrom::CreatedAt,
        age_column: "",
        eviction: EvictionStrategy::Delete,
        purpose: "Cached news (relevant + global) per spec §10 / working agreement §18.3. 90d retention; cloud worker resyncs fresh items.",
    },
    CachePolicy {
        table: "projection_snapshots",
        ttl: Duration::from_secs(30 * 24 * 60 * 60),
        age_from: AgeFrom::CreatedAt,
        age_column: "",
        eviction: EvictionStrategy::RollupThenDelete,
        purpose: "Predictive layer outputs (Monte Carlo NW trajectory, cash-flow forecast, retirement). Latest 30d full-res; older rolled to monthly aggregates per §18.3 via cache_eviction::Rollup impl in Track C PR-C15.",
    },
    CachePolicy {
        table: "agent_audit_log",
        ttl: Duration::from_secs(12 * 30 * 24 * 60 * 60),
        age_from: AgeFrom::CreatedAt,
        age_column: "",
        eviction: EvictionStrategy::ArchiveThenDelete,
        purpose: "Per-tool-call AI agent audit. 12mo full-res; archived to Mizan Connect cold storage per §18.3.",
    },
    //
    // ── Track F — hawl_anchors (NOT a cache — durable financial state, no TTL)
    //   `hawl_anchors` is intentionally NOT registered here. Hawl anchors are
    //   per-asset-cohort lunar-year tracking state per spec §11.3; they're
    //   updated on every Zakat run and must not be evicted.
    //
    // ── Track C — reconciliation_queue (NOT a cache — durable until user resolves)
    //   `reconciliation_queue` rows live until the user clears them by choosing
    //   keep-manual / replace-with-sync / merge. Eviction would silently drop
    //   unresolved conflicts.
    //
    // ── Track E — holdings_metadata (NOT a cache — durable provenance state)
    //   `holdings_metadata` carries badge provenance + Sharia/AI/Advisor
    //   metadata. Per-row durable until the underlying holding is removed
    //   (FK cascade via accounts).
];

/// Look up a cache policy by table name. Returns `None` for tables that
/// are NOT caches (durable state) — callers that try to evict an
/// unregistered table get nothing back and must handle that as a
/// "table is durable, do not touch" case.
#[must_use]
pub fn policy_for(table: &str) -> Option<&'static CachePolicy> {
    CACHE_POLICIES.iter().find(|p| p.table == table)
}

/// Iterate the registry. Useful for the eviction worker and the
/// CI lint.
#[must_use]
pub fn all_policies() -> &'static [CachePolicy] {
    CACHE_POLICIES
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn no_duplicate_table_names() {
        let mut seen = HashSet::new();
        for p in CACHE_POLICIES {
            assert!(
                seen.insert(p.table),
                "duplicate cache policy entry for table {}",
                p.table
            );
        }
    }

    #[test]
    fn ttl_is_positive() {
        for p in CACHE_POLICIES {
            assert!(
                p.ttl.as_secs() > 0,
                "cache policy for {} has zero or negative TTL — \
                 use EvictionStrategy::KeepMarkStale with a meaningful TTL \
                 to flag staleness without auto-deletion",
                p.table
            );
        }
    }

    #[test]
    fn purpose_is_non_empty() {
        // Per §A17 support diagnostic bundles, every cache must explain
        // what it caches in human terms.
        for p in CACHE_POLICIES {
            assert!(
                !p.purpose.is_empty(),
                "cache policy for {} has empty purpose — fill it in so the \
                 diagnostic bundle and admin tooling can render it",
                p.table
            );
        }
    }

    #[test]
    fn custom_age_column_filled_when_strategy_is_custom() {
        for p in CACHE_POLICIES {
            if p.age_from == AgeFrom::Custom {
                assert!(
                    !p.age_column.is_empty(),
                    "cache policy for {} uses AgeFrom::Custom but age_column is empty",
                    p.table
                );
            } else {
                assert!(
                    p.age_column.is_empty(),
                    "cache policy for {} uses AgeFrom::{:?} but age_column is set — \
                     age_column is only meaningful with AgeFrom::Custom",
                    p.table,
                    p.age_from
                );
            }
        }
    }

    #[test]
    fn policy_for_returns_expected_entry() {
        let q = policy_for("quotes").expect("quotes cache must be registered");
        assert_eq!(q.age_column, "as_of");
        assert_eq!(q.eviction, EvictionStrategy::KeepMarkStale);
    }

    #[test]
    fn policy_for_returns_none_for_durable_table() {
        // truth_ledger is durable (append-only); must NOT have a policy
        assert!(policy_for("truth_ledger").is_none());
        // accounts is durable user state; must NOT have a policy
        assert!(policy_for("accounts").is_none());
    }
}
