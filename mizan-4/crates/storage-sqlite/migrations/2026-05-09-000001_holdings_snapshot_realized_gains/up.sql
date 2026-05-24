-- Add a JSON-blob column to persist the per-asset realized-P&L
-- accumulator on each snapshot. Defaults to "{}" so existing rows
-- decode cleanly as an empty HashMap; the next portfolio recalc will
-- repopulate them from the SELL activity stream.
ALTER TABLE holdings_snapshots ADD COLUMN realized_gains TEXT NOT NULL DEFAULT '{}';
