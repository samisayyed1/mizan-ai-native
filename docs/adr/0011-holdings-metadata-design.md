# ADR 0011 — Holdings Metadata Design (Pre-Track E)

**Status:** Accepted
**Date:** 2026-06-02
**Deciders:** Sami Sayyed
**Track:** E (Mizan Badge Expansion) — pre-requisite PR-E0

## Context

The [Mizan Evolution Master Plan](../plans/00-master-plan.md) Track E (Mizan Badge Expansion) assumed a relational `holdings` table with per-row columns for new metadata:

- `sharia_status` (compliant / non_compliant / mixed / unrated, nullable) + `last_screened_at`
- `ai_estimated` (boolean) + `ai_confidence` + `ai_value_range_low` + `ai_value_range_high`
- `tags` (jsonb array)
- Modifier states (`stale`, `pending-reconciliation`, etc. — actually computed at read time, not stored)

A Phase 0 reality check (2026-06-02) discovered:

> The desktop holdings model is **JSON inside `portfolio_history.holdings`** (a `TEXT` column on the `portfolio_history` table), **not a relational `holdings` table**.

This breaks the migration plan as written. Three design options surface.

## Decision

Adopt **Option A: holdings_metadata side table** keyed by `(account_id, holding_symbol, as_of_date)`.

### Schema sketch

```sql
CREATE TABLE holdings_metadata (
    -- Composite primary key matching how holdings appear in portfolio_history.holdings JSON
    account_id          TEXT NOT NULL,
    holding_symbol      TEXT NOT NULL,
    as_of_date          DATE NOT NULL,   -- matches portfolio_history.date

    -- Origin / provenance (extends sync_provider enum scope)
    origin              TEXT NOT NULL,   -- 'manual' | 'plaid' | 'snaptrade' | 'csv' | 'example' | 'setu' | 'sgfindex' | 'tink' | 'basiq' | 'lean' | 'ccxt' | 'chain_reader'

    -- Sharia screening (Track E)
    sharia_status       TEXT,            -- 'compliant' | 'non_compliant' | 'mixed' | 'unrated'
    last_screened_at    DATETIME,

    -- AI estimation (Track B real estate + collectibles)
    ai_estimated        INTEGER NOT NULL DEFAULT 0,  -- bool
    ai_confidence       TEXT,            -- 'low' | 'mid' | 'high'
    ai_value_range_low  NUMERIC,
    ai_value_range_high NUMERIC,

    -- User tags
    tags                TEXT,            -- JSON array

    -- Modifier states that are persisted (stale + pending-reconciliation
    -- are computed at read time, not stored)
    advisor_reviewed_by TEXT,            -- user_id of reviewing advisor (Track G)
    advisor_reviewed_at DATETIME,
    agent_modified_at   DATETIME,        -- last AI-agent mutation timestamp (Track C)

    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (account_id, holding_symbol, as_of_date),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX idx_holdings_metadata_origin ON holdings_metadata(origin);
CREATE INDEX idx_holdings_metadata_sharia ON holdings_metadata(sharia_status) WHERE sharia_status IS NOT NULL;
CREATE INDEX idx_holdings_metadata_screened_at ON holdings_metadata(last_screened_at) WHERE last_screened_at IS NOT NULL;
```

### Read path

The frontend's "Holdings list" rendering already decodes `portfolio_history.holdings` JSON to produce per-holding rows. The new metadata lookup is a left-join against `holdings_metadata` on `(account_id, symbol, latest_date)`. Missing metadata rows are treated as "no metadata yet" — the badge defaults to `origin: 'manual'` and no modifiers.

### Write path

- **Sync run completes** → writes `holdings_metadata` rows with `origin` set from the provider
- **Manual entry** → writes with `origin = 'manual'`
- **AAOIFI screening worker** (Track E PR-E4) → writes `sharia_status` + `last_screened_at`
- **AI estimation pipeline** (Track B PR-B12/B15) → writes `ai_estimated = 1` + range
- **Agent mutation** (Track C) → writes `agent_modified_at = now()`
- **Advisor sign-off** (Track G) → writes `advisor_reviewed_by` + `advisor_reviewed_at`

### Modifier states NOT stored

`'stale'`, `'pending-reconciliation'`, `'audit-trail'`, `'agent-modified'` (the BADGE — not the timestamp), `'mixed-compliance'` are **computed at read time** from other state:

- `'stale'`: `now() - quotes.as_of > class-specific TTL` from `cache_policy::policy_for(...)`
- `'pending-reconciliation'`: row exists in `reconciliation_queue` for this `(account_id, holding_symbol)`
- `'audit-trail'`: derived from `truth_ledger` head hash, always renderable
- `'agent-modified'`: derived from `agent_modified_at` (within last 24h)
- `'mixed-compliance'`: derived from `sharia_status = 'mixed'`

This keeps the schema lean — modifier badges are functions of state, not stored state.

## Rationale

**Why a side table (Option A) over JSON extension (Option B):**

- Indexable. `WHERE sharia_status = 'compliant'` returns rows in `O(log n)` via the partial index; same query against a JSON field requires `json_extract` + scan.
- Migration-friendly. Adding more metadata columns later is a normal `ALTER TABLE`; adding to a JSON blob requires every row to be touched.
- Schema-discoverable. Diesel schema.rs reflects reality; JSON fields hide their shape.
- Foreign-key integrity. `account_id` cascades from `accounts.id`.

**Why side table over relational holdings (Option C):**

- Option C is a multi-month migration with downstream consumer changes across `crates/core/portfolio`, `crates/core/synthesis`, the frontend holdings rendering pipeline, the AI agent's tools, every report template, and the test fixtures. **Net negative for Track E scope.**
- Option A delivers the badge functionality without touching the existing portfolio_history pipeline. The two coexist: portfolio_history retains its JSON holdings (snapshot of "what was held at date D"); holdings_metadata stores the slowly-changing per-holding provenance.
- If a future track needs the relational holdings model anyway, Option A doesn't prevent it — the migration absorbs holdings_metadata as a natural source of column data.

**Why composite key `(account_id, holding_symbol, as_of_date)` rather than a UUID:**

- The JSON-in-portfolio-history model doesn't generate a stable UUID per holding (the JSON blob is rebuilt each sync). The natural identity of a holding row is its account + symbol + date.
- Composite key matches the read-path join.
- A UUID per holding would require a major refactor to portfolio_history JSON producers; out of scope here.

## Consequences

**Positive:**
- Track E PR-E1 migration can proceed against a real schema design
- Future Tracks (Sharia worker, AI estimation, agent timestamps, advisor sign-off) write to the same table — single coherent surface for badges
- Indexes on `origin` and `sharia_status` make the Mizan Badge render-time lookups O(log n)
- Composite-key approach avoids touching the existing portfolio_history producers

**Negative:**
- Two tables (`portfolio_history` + `holdings_metadata`) to keep in sync — the sync worker must write both, the reconciliation path must compare both
- Older portfolio_history rows have no metadata; the read path must default gracefully (treated as "no metadata yet")
- Backfill needed for existing user data — a migration that creates `'manual'` metadata rows for every distinct (account, symbol) currently in portfolio_history would over-claim manual origin. **Decision: do not backfill.** New rows get metadata; old rows render with badge defaults until next sync rewrites them.

**Follow-ups (tracked):**
- Track E PR-E1.a: migration creating `holdings_metadata` table per the schema sketch above
- Track E PR-E1.b: read-path left join in the holdings render pipeline
- Track E PR-E1.c: write-path wiring for Plaid + SnapTrade + manual + CSV
- Track E PR-E1.d: Diesel schema.rs regeneration

## Alternatives Considered

**Option B: Extend `portfolio_history.holdings` JSON.** Rejected — index unfriendly, schema-undiscoverable, migration unfriendly.

**Option C: Migrate to relational `holdings` table.** Rejected for Track E scope — multi-month effort touching every consumer. May happen later as its own Track; this ADR doesn't preclude it.

**Option D: Skip per-holding metadata entirely; store only account-level badges.** Rejected — defeats the working agreement §12 ("every number on screen carries its source") and Spec §6 (universal pattern badge rendering on every holding row).

## References

- `crates/storage-sqlite/migrations/2024-09-16-023604_portfolio_history/up.sql` — current `portfolio_history` schema with JSON holdings
- `docs/working-agreement.md` §12 — Mizan Badge product surface rules
- `docs/plans/00-master-plan.md` Track E
- `docs/plans/SESSION-LOG-2026-06-02.md` — the finding that prompted this ADR
