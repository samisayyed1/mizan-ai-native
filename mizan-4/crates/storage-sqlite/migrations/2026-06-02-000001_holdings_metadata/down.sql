-- Track E PR-E1.a — down migration for holdings_metadata.
--
-- Diesel's down.sql convention. Per docs/working-agreement.md §10 + §19.9,
-- production migrations are forward-only — this file exists for Diesel's
-- local-dev `diesel migration redo` workflow only.
--
-- Production schema state is restored by writing a corrective forward
-- migration that re-creates the table, never by `migration redo` against
-- a production DB.

DROP INDEX IF EXISTS idx_holdings_metadata_advisor_reviewed_by;
DROP INDEX IF EXISTS idx_holdings_metadata_agent_modified_at;
DROP INDEX IF EXISTS idx_holdings_metadata_ai_estimated;
DROP INDEX IF EXISTS idx_holdings_metadata_screened_at;
DROP INDEX IF EXISTS idx_holdings_metadata_sharia;
DROP INDEX IF EXISTS idx_holdings_metadata_origin;

DROP TABLE IF EXISTS holdings_metadata;
