#!/usr/bin/env bash
# Track I PR-I1.b — Cache policy registration lint.
#
# Walks `crates/storage-sqlite/src/cache_policy.rs::CACHE_POLICIES` and
# reports any cache-shaped table in the schema that's missing from the
# registry. Per ADR 0008 + docs/working-agreement.md §19.7.
#
# Cache-shaped tables for this lint's purposes are tables whose names
# end in one of the conventional cache suffixes:
#   * _cache
#   * _quotes
#   * _runs       (e.g. daily_brief_runs)
#   * news_items
#   * projection_snapshots
#   * agent_audit_log
#   * sync_run_ledger
#
# Adjust this list as the cache landscape evolves — the registry remains
# the authoritative source; this script only enforces "if you added one,
# you registered it."
#
# Exit codes:
#   0 — every cache-shaped table is registered (or there are none)
#   1 — at least one cache-shaped table is missing from CACHE_POLICIES
#   2 — script invocation error (file missing, etc.)

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
POLICY_FILE="$REPO_ROOT/mizan-4/crates/storage-sqlite/src/cache_policy.rs"
SCHEMA_FILE="$REPO_ROOT/mizan-4/crates/storage-sqlite/src/schema.rs"

if [[ ! -f "$POLICY_FILE" ]]; then
  echo "ERROR: $POLICY_FILE not found." >&2
  echo "Track I PR-I1 must land before this lint runs." >&2
  exit 2
fi

if [[ ! -f "$SCHEMA_FILE" ]]; then
  echo "ERROR: $SCHEMA_FILE not found." >&2
  exit 2
fi

# Extract table names from the Diesel schema. Schema entries look like:
#   diesel::table! {
#       table_name (pk) {
#           ...
# We pick out the `table_name (pk) {` line. AWK is more reliable than
# multi-stage sed here.
schema_tables=$(awk '/^[[:space:]]+[a-z_][a-z0-9_]*[[:space:]]*\(.*\)[[:space:]]*\{$/ {
                       gsub(/^[[:space:]]+/, "", $0)
                       sub(/[[:space:]]*\(.*$/, "", $0)
                       print
                     }' "$SCHEMA_FILE" 2>/dev/null | sort -u || true)

# Also harvest CREATE TABLE definitions in migrations as a secondary source
# (catches tables added but not yet propagated to schema.rs).
migration_tables=$(grep -rhE 'CREATE TABLE[[:space:]]+"?[a-z_][a-z0-9_]*"?' \
                   "$REPO_ROOT/mizan-4/crates/storage-sqlite/migrations" 2>/dev/null \
                   | sed -E 's/.*CREATE TABLE[[:space:]]+"?([a-z_][a-z0-9_]*)"?.*/\1/' \
                   | sort -u || true)

all_tables=$(printf "%s\n%s\n" "$schema_tables" "$migration_tables" | sort -u | grep -v '^$' || true)

# Tables registered in CACHE_POLICIES (extract `table: "name",`).
registered=$(grep -oE 'table:\s*"([a-z_][a-z0-9_]*)"' "$POLICY_FILE" \
             | sed -E 's/.*"([a-z_][a-z0-9_]*)".*/\1/' \
             | sort -u || true)

# Cache-shaped table predicate.
#
# Cache tables get TTL + eviction policy in CACHE_POLICIES. The list below
# is the EXPLICIT allowlist of names recognised as caches — heuristic
# suffix matching turned out wrong (`_runs` matched `import_runs` which is
# durable state, not a cache). Add new entries here when a new cache table
# joins the schema; the lint then enforces a matching CACHE_POLICIES entry.
#
# Tables NOT in this list are treated as durable state by the lint —
# adding one to the schema does NOT require a CACHE_POLICIES entry.
KNOWN_CACHE_TABLES=(
  "quotes"
  "fx_rates"
  "daily_brief_runs"
  "sync_run_ledger"
  "market_news"
  # Tracks C/D — activated 2026-06-02:
  "user_memory"
  "news_items"
  "projection_snapshots"
  "agent_audit_log"
)

cache_shaped() {
  local name="$1"
  for t in "${KNOWN_CACHE_TABLES[@]}"; do
    if [[ "$name" == "$t" ]]; then
      return 0
    fi
  done
  return 1
}

missing=()
while IFS= read -r t; do
  [[ -z "$t" ]] && continue
  if cache_shaped "$t"; then
    if ! grep -qx "$t" <<<"$registered"; then
      missing+=("$t")
    fi
  fi
done <<<"$all_tables"

if [[ ${#missing[@]} -eq 0 ]]; then
  echo "cache-policy: OK — every cache-shaped table is registered in CACHE_POLICIES"
  exit 0
fi

echo "cache-policy: FAIL — the following cache-shaped tables are missing from CACHE_POLICIES:" >&2
for t in "${missing[@]}"; do
  echo "  - $t" >&2
done
echo "" >&2
echo "Per ADR 0008 + working-agreement §19.7, every cache table must declare its TTL + eviction policy." >&2
echo "Add an entry to mizan-4/crates/storage-sqlite/src/cache_policy.rs::CACHE_POLICIES." >&2
exit 1
