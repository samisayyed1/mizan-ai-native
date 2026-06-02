-- ============================================================================
-- Track F PR-F1 — Hawl anchor tracking per asset cohort
-- ----------------------------------------------------------------------------
-- Per spec §11.3 and `docs/plans/06-track-f.md`:
--
-- > Per-asset-cohort lunar-year tracking. New `hawl_anchors` table on desktop:
-- > cohort_id, anchor_date (lunar calendar conversion), current_qualifying_amount,
-- > last_evaluated. The Zakat engine reads this every time it runs.
-- > ZakatHawlApproaching notifications fire at 30/7/1 day before Hawl completion
-- > for each cohort.
--
-- A "cohort" is a logical grouping of assets that share a Hawl (lunar-year)
-- anniversary. The Hanafi school's classical treatment is one Hawl for the
-- whole Nisab-qualifying wealth; some users prefer per-asset Hawls for
-- newly-acquired assets. The cohort_id is the key that bridges either view.
--
-- This is DURABLE state (not a cache) — Hawl anchors are updated on every
-- Zakat calculation and must never be evicted by the cache_policy worker.
-- Explicitly absent from CACHE_POLICIES per ADR 0008.
--
-- caches-evicted: (none — durable financial state)
-- ============================================================================

CREATE TABLE hawl_anchors (
    -- ── Identity ─────────────────────────────────────────────────────
    -- cohort_id is user-defined or derived by the Zakat engine. Examples:
    --   "default"            — single-cohort treatment (classical Hanafi)
    --   "cash-2026"          — cohort started when this cash balance first crossed Nisab
    --   "emaar-sukuk-2024"   — cohort for a specific holding acquired 2024
    cohort_id           TEXT NOT NULL,

    -- ── User scoping ────────────────────────────────────────────────
    -- Per-user — different users may have different cohort structures
    -- depending on their scholarly preference (stored in `user_memory`).
    user_id             TEXT NOT NULL,

    -- ── Lunar anchor ────────────────────────────────────────────────
    -- The Gregorian date of the most recent Hawl completion (or first
    -- crossing of Nisab for a new cohort). The next Hawl completes
    -- 354.367 Gregorian days later (Islamic lunar year). The Zakat
    -- engine computes the next Hawl date at evaluation time — it is
    -- NOT stored, to keep the conversion logic in one place.
    anchor_date         DATE NOT NULL,

    -- ── Qualifying amount ──────────────────────────────────────────
    -- The base-currency value of this cohort's qualifying wealth at
    -- the most recent Hawl evaluation. Stored as text-encoded Decimal
    -- (rust_decimal::Decimal::to_string()) — never f64 in money paths
    -- (working agreement §5).
    current_qualifying_amount TEXT NOT NULL,
    -- The currency the qualifying amount is denominated in. Always the
    -- user's base currency at evaluation time — historical conversions
    -- are not re-computed when FX rates change (the immutable-history
    -- property of `net_worth_snapshot` applies here too).
    base_currency       TEXT NOT NULL,

    -- ── Evaluation timestamps ──────────────────────────────────────
    last_evaluated      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (user_id, cohort_id)
);

-- ── Indexes ──────────────────────────────────────────────────────────

-- Approaching-Hawl scan: the ZakatHawlApproaching insights rule (§9.3)
-- scans for cohorts whose anchor_date + 354 days falls in the near future.
-- An index on anchor_date supports that scan.
CREATE INDEX idx_hawl_anchors_anchor_date
    ON hawl_anchors(anchor_date);

-- Stale-evaluation scan: cohorts whose last_evaluated is too old get
-- re-evaluated by the Zakat engine on next user action.
CREATE INDEX idx_hawl_anchors_last_evaluated
    ON hawl_anchors(last_evaluated);
