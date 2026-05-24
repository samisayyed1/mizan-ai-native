---
name: mizan-migration-author
description: Use when authoring a new SQLite (mizan-4/crates/storage-sqlite/migrations/) or Postgres (mizan-connect/migrations/) migration. Enforces forward-only, idempotent, tested against clean + previously-migrated DBs, with the v3.1 addendum §15 migration test matrix.
---

# Migration recipe (desktop SQLite + cloud Postgres)

Forward-only is the floor, not the ceiling. Every migration must be
**idempotent** (re-runnable) and **safe to apply mid-rollout** (no app
crash on transient schema mismatch).

## Desktop SQLite (Diesel)

Location: `mizan-4/crates/storage-sqlite/migrations/<ts>_<name>/{up.sql, down.sql}`.

- `down.sql` is required by Diesel but **must not be invoked in
  production**. Treat it as a developer escape hatch only.
- Use `CREATE TABLE IF NOT EXISTS`, `ALTER TABLE ... ADD COLUMN`
  (SQLite supports this without rewriting the table). Avoid `DROP TABLE`
  on any user-financial table.
- Cost-basis / balances columns: store as `TEXT` and decode with
  `rust_decimal::Decimal::from_str`. **Never `REAL`.**
- Foreign keys explicit. `PRAGMA foreign_keys = ON` is set at app start.
- Update `crates/storage-sqlite/src/schema.rs` via `diesel migration run`
  - commit the regenerated file.

## Cloud Postgres (SQLx)

Location: `mizan-connect/migrations/<ts>_<name>.sql`.

- Single file, single statement-set, forward-only. No `down.sql`.
- `gen_random_uuid()` for UUIDs (extension `pgcrypto` enabled in 0001).
- After SQL change, run `cargo sqlx prepare` and commit
  `sqlx-data.json` so offline builds compile.
- Use `IF NOT EXISTS` for table creation; use safe `ADD COLUMN ... NULL`
  - backfill + later `SET NOT NULL` in a follow-up migration for
    non-null adds against populated tables.

## Migration test matrix (v3.1 §15 — binding)

Every new migration must pass against:

1. Clean DB.
2. Previous-version DB (latest released schema).
3. DB with Plaid data (one item, multiple accounts).
4. DB with SnapTrade data (one connection, multiple positions).
5. Manual-only DB (no providers).
6. Corrupted / partial-row DB (one orphan row).
7. Seeded-examples DB (3 example liabilities).
8. User-created-liabilities DB.

Add an integration test under `tests/` that boots a fresh DB,
applies the migration, asserts the expected schema and that pre-existing
fixtures still query correctly.

## Rollback policy

**Never manually edit a production DB.** If a migration corrupts data,
the rollback path is:

1. Restore the most recent encrypted backup (desktop) or Postgres
   point-in-time (cloud).
2. Ship a corrective migration.
3. Re-deploy.

## When done

Have the `migration-reviewer` subagent audit the migration before
merging. Run `mizan-pr-checklist`.
