-- Production hot-path indexes
--
-- Performance audit (pre-soft-launch) identified the activities table
-- as the dominant portfolio-recalc cost driver at scale (5k+ activities).
-- The base init_db migration created separate indexes on `account_id`
-- and `activity_date` but no composite. Almost every portfolio query
-- selects "all activities for THIS account between date A and B" —
-- which SQLite resolves by scanning the `account_id` index and then
-- filtering each result row by date.
--
-- Composite (account_id, activity_date DESC) lets the planner walk
-- the index in chronological order without a separate filter pass.
-- DESC suffix matches the most common access pattern: latest activity
-- first.
--
-- Also adds a standalone date index on daily_account_valuation for
-- the "monthly performance report" query path that filters by date
-- across all accounts.

CREATE INDEX IF NOT EXISTS ix_activities_account_date
  ON activities(account_id, activity_date DESC);

CREATE INDEX IF NOT EXISTS ix_daily_account_valuation_date
  ON daily_account_valuation(valuation_date);
