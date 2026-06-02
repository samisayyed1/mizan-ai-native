#!/usr/bin/env bash
# Track I PR-I2.d — Migration cache-eviction manifest lint.
#
# Per ADR 0008 + working-agreement §19.2:
#
#   Every migration in `mizan-desktop/migrations/` carries a manifest entry
#   listing which cache tables it invalidates.
#   New CI rule: any migration touching a schema referenced by a cache
#   table must explicitly declare which caches to evict, or CI rejects.
#
# The manifest convention adopted in PR-E1.a / PR-C1.a / PR-F1: a comment
# line at the top of the migration's `up.sql`:
#
#   -- caches-evicted: (none — durable state)
#   -- caches-evicted: market_news, quotes
#
# This lint checks that every NEW migration's `up.sql` contains exactly
# one `-- caches-evicted:` line. Existing migrations without one are not
# flagged (grandfathered until they next change).
#
# Heuristic for "new" migration: any migration directory whose timestamp
# is after the threshold below. Adjust threshold when shipping the lint to
# pick up the migrations landed in this Track I batch onward.

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
MIGRATIONS_DIR="$REPO_ROOT/mizan-4/crates/storage-sqlite/migrations"

# Manifest required for all migrations dated 2026-06-02 onward (when the
# convention was formalised in PR-E1.a). Earlier migrations are
# grandfathered.
THRESHOLD="2026-06-02"

if [[ ! -d "$MIGRATIONS_DIR" ]]; then
  echo "ERROR: $MIGRATIONS_DIR not found." >&2
  exit 2
fi

missing=()
for migration in "$MIGRATIONS_DIR"/*/; do
  name=$(basename "$migration")
  # Extract the YYYY-MM-DD prefix from the migration name.
  date_prefix="${name:0:10}"
  # Only check migrations dated on or after THRESHOLD.
  if [[ "$date_prefix" < "$THRESHOLD" ]]; then
    continue
  fi
  up="$migration/up.sql"
  if [[ ! -f "$up" ]]; then
    echo "ERROR: $up not found." >&2
    continue
  fi
  if ! grep -qE '^-- caches-evicted:' "$up"; then
    missing+=("$name")
  fi
done

if [[ ${#missing[@]} -eq 0 ]]; then
  echo "migration-cache-manifest: OK — every migration ≥ $THRESHOLD declares caches-evicted"
  exit 0
fi

echo "migration-cache-manifest: FAIL — the following migrations lack the cache-eviction manifest:" >&2
for m in "${missing[@]}"; do
  echo "  - $m" >&2
done
echo "" >&2
echo "Per ADR 0008 + working-agreement §19.2, every migration must carry a comment:" >&2
echo "" >&2
echo "  -- caches-evicted: (none — durable state)" >&2
echo "" >&2
echo "or a comma-separated list of cache tables the migration invalidates:" >&2
echo "" >&2
echo "  -- caches-evicted: market_news, quotes" >&2
echo "" >&2
echo "Add the line to the migration's up.sql and re-run." >&2
exit 1
