# Runbook: Supabase Postgres Lifecycle Hygiene

Quarterly review of Mizan Connect's Postgres database health.

## When to run

- **Calendar-scheduled**: last week of each quarter (alongside the broader tech-debt sweep)
- Ad-hoc: when slow-query alerts fire repeatedly, or when bloat is suspected from steady storage growth out of proportion to row growth

## Prerequisites

- Access to the Supabase project's SQL editor (and a `psql` shell against the prod-mirror)
- Familiarity with `pg_stat_statements` and `pgstattuple` outputs
- 90 minutes blocked off

## Activity 1: Slow-query review

1. Pull the worst offenders from `pg_stat_statements`:

   ```sql
   select query, calls, mean_exec_time, max_exec_time, rows
   from pg_stat_statements
   order by mean_exec_time desc
   limit 20;
   ```

2. Anything > 100ms mean exec time on a hot path is a finding. For each:
   - Read the query plan: `EXPLAIN (ANALYZE, BUFFERS) <query>`
   - Confirm it uses an index (no sequential scan on tables > 10k rows)
   - File an issue with the query + plan + fix recommendation

3. Reset stats once findings are filed:

   ```sql
   select pg_stat_statements_reset();
   ```

## Activity 2: Index coverage audit

1. Identify unused indexes:

   ```sql
   select schemaname, relname, indexrelname, idx_scan
   from pg_stat_user_indexes
   where idx_scan = 0
   and indexrelname not like '%pkey%'
   order by relname;
   ```

   Truly-zero-scan indexes that have been live > 1 quarter: candidate for removal. File a forward-migration to drop.

2. Identify missing indexes — every WHERE clause in `pg_stat_statements` top 50 should be index-backed:

   ```sql
   select query, mean_exec_time, rows
   from pg_stat_statements
   where query ~* 'where .* = '
   order by mean_exec_time desc
   limit 50;
   ```

   Cross-reference each query's WHERE column against `pg_indexes` for the relevant table. Any uncovered → file a forward-migration to add the index.

## Activity 3: Bloat monitoring

1. Run `pgstattuple` on every user-data table > 1GB:

   ```sql
   select relname, pg_size_pretty(pg_total_relation_size(oid)) as size,
          (pgstattuple(oid)).dead_tuple_percent
   from pg_class
   where relkind = 'r' and pg_total_relation_size(oid) > 1073741824
   order by pg_total_relation_size(oid) desc;
   ```

2. Any table > 20% dead-tuple percent is a candidate for `VACUUM FULL`. Schedule the VACUUM during a low-traffic window — it locks the table.

## Activity 4: Connection pool sizing

1. Check current pool config:

   ```bash
   fly secrets list --app mizan-connect | grep DATABASE_MAX_CONNECTIONS
   ```

2. Check actual concurrency: `pg_stat_activity` row count during peak traffic.

3. If pool is consistently saturated, raise the limit (mindful of Supabase's per-tier connection cap).

## Activity 5: Row-level security (RLS) review

1. List all tables with RLS:

   ```sql
   select schemaname, tablename, rowsecurity from pg_tables
   where schemaname = 'public' order by tablename;
   ```

2. Confirm every table holding user data has `rowsecurity = true`. New tables without RLS are a finding.

3. Sample-audit one policy per table — does it correctly scope to the JWT's `auth.uid()`?

## Verification

- Slow-query review complete with findings filed
- Unused indexes identified
- Missing indexes identified
- Bloat measured; VACUUM FULL scheduled where needed
- Pool sizing reviewed
- RLS coverage confirmed

## Output

Write a review report at `docs/runbooks/drill-reports/YYYY-Q{N}-supabase-lifecycle.md`:

- Date
- Reviewer
- Activity 1–5 findings
- Action items (with owners + deadlines)
- Comparison to prior quarter (trends)

## Escalation

- If you find evidence of an unauthorized query pattern (e.g. row counts that suggest a user accessed another user's data), treat as SEV-0 security incident
- If RLS is disabled on any user-data table, treat as SEV-0 and re-enable immediately

## Related

- `docs/runbooks/deploy.md`
- `docs/working-agreement.md` §10 (Database Discipline), §19.10
