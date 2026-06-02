#!/usr/bin/env bash
# Track H PR-H8.b — f64-in-money-paths lint.
#
# The QA Pass 4 lesson (working agreement §13): an `f64` slipped into a
# P&L calculation and produced a $1.32 rounding drift on a $1.7M portfolio.
# `rust_decimal::Decimal` is mandatory in money paths.
#
# This lint enforces the rule structurally by grepping for `f64` usage
# in directories that handle money:
#   - crates/core/src/portfolio/*
#   - crates/core/src/activities/*
#   - crates/core/src/zakat/*    (once extracted)
#   - crates/core/src/synthesis/* (once extracted)
#   - crates/core/src/insights/*  (once extracted)
#   - crates/core/src/health/*
#   - crates/financial-truth/*    (once extracted in Track H PR-H3.a)
#
# Allowed contexts (grep is heuristic — these patterns are NOT flagged):
#   - `// f64` in comments
#   - `f64 (parsed)` style explanation comments
#   - `as_f64()` method calls for tests that compare to known answers
#     (test files are excluded entirely)
#   - String literals containing the substring (rare; allowed)
#
# Exit codes:
#   0 — zero f64 usage in money paths
#   1 — at least one f64 in a money path (real failure)
#   2 — script invocation error

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show2-toplevel 2>/dev/null || pwd)}"
if [[ ! -d "$REPO_ROOT/mizan-4" ]]; then
  REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

# Directories where f64 is forbidden in production code paths.
MONEY_PATHS=(
  "mizan-4/crates/core/src/portfolio"
  "mizan-4/crates/core/src/activities"
  "mizan-4/crates/core/src/zakat"
  "mizan-4/crates/core/src/synthesis"
  "mizan-4/crates/core/src/insights"
  "mizan-4/crates/core/src/health"
  "mizan-4/crates/core/src/net_worth_snapshot"
  "mizan-4/crates/core/src/financial_truth"
  "mizan-4/crates/financial-truth/src"
  "mizan-4/crates/zakat/src"
  "mizan-4/crates/synthesis/src"
  "mizan-4/crates/insights/src"
)

# Files exempt from the check. Tests can use f64 to compare against known
# Decimal-arithmetic outputs.
EXEMPT_PATTERNS=(
  "_tests.rs"
  "/tests/"
  "/test_"
  "/golden/"
)

findings=()

for dir in "${MONEY_PATHS[@]}"; do
  full="$REPO_ROOT/$dir"
  [[ -d "$full" ]] || continue
  # Match `f64` as a whole word (Rust identifier boundary). Exclude
  # exempt patterns from the grep results.
  while IFS=: read -r file lineno match; do
    skip=0
    for pat in "${EXEMPT_PATTERNS[@]}"; do
      if [[ "$file" == *"$pat"* ]]; then
        skip=1
        break
      fi
    done
    if [[ $skip -eq 1 ]]; then
      continue
    fi
    # Skip comments (cheap heuristic: line starts with whitespace then `//` or `/*`)
    trimmed="${match#"${match%%[![:space:]]*}"}"
    case "$trimmed" in
      "//"*|"/*"*|"*"*) continue ;;
    esac
    findings+=("$file:$lineno:$match")
  done < <(grep -rn --include='*.rs' -wE 'f64' "$full" 2>/dev/null || true)
done

if [[ ${#findings[@]} -eq 0 ]]; then
  echo "no-f64-in-money-paths: OK — zero f64 usage in money-path directories"
  exit 0
fi

echo "no-f64-in-money-paths: FAIL — f64 found in money-path directories:" >&2
echo "" >&2
for f in "${findings[@]}"; do
  echo "  $f" >&2
done
echo "" >&2
echo "Working agreement §0 rule (Decimal-only in money paths) + §13 past-bug" >&2
echo "(QA Pass 4: \$1.32 drift on \$1.7M portfolio from f64 in P&L)." >&2
echo "" >&2
echo "Replace f64 with rust_decimal::Decimal. If the use is genuinely outside" >&2
echo "the money path (e.g. a UI animation easing curve), move it OUT of these" >&2
echo "directories or move the file's tests into the exempt pattern list." >&2
exit 1
