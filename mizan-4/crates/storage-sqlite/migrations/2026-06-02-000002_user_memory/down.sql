-- Track C PR-C1.a — down migration for user_memory.
-- Per working agreement §10 + §19.9 production migrations are forward-only;
-- this file exists for Diesel local-dev `migration redo` only.

DROP INDEX IF EXISTS idx_user_memory_category;
DROP INDEX IF EXISTS idx_user_memory_last_used_at;
DROP INDEX IF EXISTS idx_user_memory_expires_at;
DROP INDEX IF EXISTS idx_user_memory_user;

DROP TABLE IF EXISTS user_memory;
