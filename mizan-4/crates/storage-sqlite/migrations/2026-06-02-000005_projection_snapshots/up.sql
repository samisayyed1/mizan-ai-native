-- ============================================================================
-- Track C PR-C15 (foundation) — projection_snapshots predictive cache
-- ----------------------------------------------------------------------------
-- Per spec §7.6 (Predictive Layer):
--
-- > Background scheduled jobs (running in a Tauri sidecar process) produce
-- > continuous projections: net worth trajectory (Monte Carlo), cash flow
-- > forecast, retirement projection. Stored in projection_snapshots table
-- > for fast reads.
--
-- Latest 30 days at full resolution; older rows rolled up into monthly
-- aggregates by the RollupThenDelete eviction strategy (cache_policy.rs
-- entry + cache_eviction::Rollup impl per Track C PR-C15 wiring).
--
-- caches-evicted: projection_snapshots
-- ============================================================================

CREATE TABLE projection_snapshots (
    id                TEXT NOT NULL PRIMARY KEY,
    user_id           TEXT NOT NULL,
    projection_kind   TEXT NOT NULL CHECK (
        projection_kind IN (
            'net_worth_monte_carlo',
            'cash_flow_forecast',
            'retirement_projection',
            'goal_tracking'
        )
    ),
    horizon_years     INTEGER NOT NULL,                -- 1, 5, 10, 30, etc.
    -- Output bins per spec §7.6: probability distribution stored as JSON.
    -- The compact JSON shape keeps reads fast; the heavy Monte Carlo math
    -- runs in the Tauri sidecar, not on the read path.
    output_json       TEXT NOT NULL,
    -- The Monte Carlo seed is captured so reruns are reproducible.
    seed              INTEGER NOT NULL,
    -- Input fingerprint — a hash of the current allocation + contribution
    -- rate + horizon — lets us short-circuit when nothing material changed.
    input_fingerprint TEXT NOT NULL,
    -- Whether this is a full-resolution daily row or a monthly aggregate
    -- (rolled up by the cache_eviction worker after 30 days).
    is_aggregate      INTEGER NOT NULL DEFAULT 0 CHECK (is_aggregate IN (0, 1)),
    created_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_projection_snapshots_user_kind_created
    ON projection_snapshots(user_id, projection_kind, created_at DESC);

-- The eviction worker uses this index to find expired daily rows for rollup
CREATE INDEX idx_projection_snapshots_rollup_candidates
    ON projection_snapshots(created_at)
    WHERE is_aggregate = 0;

-- Fingerprint lookup for short-circuit: if input_fingerprint already
-- exists with the same horizon, return cached.
CREATE INDEX idx_projection_snapshots_fingerprint
    ON projection_snapshots(user_id, projection_kind, horizon_years, input_fingerprint);
