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
    # Track H Gate 2 — Finding 3.1.1 (Informational) exemptions per 2026-Q3
    # audit report. These are RATIO / THRESHOLD / DISPLAY-PERCENTAGE f64s that
    # the user accepted as scoped Informational. They are NOT accumulating
    # money sums (those are Finding 3.1.2 Major, resolved in PR-H10).
    #
    # The exemption list is intentionally narrow — every entry has a
    # documented reason in the audit report's §3.5 sub-classification.
    # New f64 outside this allowlist will still fail the lint.
    case "$trimmed" in
      # compute_data_hash takes mv_pct as a ratio for change-detection.
      # The hash output is opaque; f64 is acceptable here.
      *"fn compute_data_hash("*|*"mv_pct: f64"*) continue ;;
      # HealthContext + HealthIssue carry ratio-percentages for display
      # and threshold comparison. Documented Informational class.
      *"total_portfolio_value: f64"*) continue ;;
      *"affected_mv_pct"*"f64"*|*"affected_mv_pct: f64"*) continue ;;
      *"mv_escalation_threshold"*"f64"*) continue ;;
      *"classification_warn_threshold"*"f64"*) continue ;;
      # Insights engine threshold constants (BIG_MOVE / NW_DIP / CASH_DRAG /
      # GOAL_MILESTONES) — ratios used in comparisons, not money sums.
      *"BIG_MOVE_THRESHOLD_PCT"*|*"BIG_MOVE_MIN_VALUE_BASE"*) continue ;;
      *"GOAL_MILESTONES"*|*"NW_DIP_THRESHOLD_PCT"*) continue ;;
      *"CASH_DRAG_PCT_THRESHOLD"*) continue ;;
      # The dec_to_f64 conversion helper itself — declaring the boundary.
      *"fn dec_to_f64("*) continue ;;
      # FIRE projection rates (Finding 3.1.3 Minor — Monte-Carlo dominated
      # by simulation variance; precision drift negligible).
      *"bond_return_rate: f64"*) continue ;;
      *"bond_allocation_at_fire: f64"*|*"bond_allocation_at_horizon: f64"*) continue ;;
      # Classification migration weights (taxonomy weights are decimal
      # fractions used in comparison; not money sums).
      *"pub weight: f64"*) continue ;;
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
echo "" >&2
echo "If this f64 is a documented ratio/threshold/display-percentage (Finding" >&2
echo "3.1.1 Informational from the 2026-Q3 baseline audit), add a new entry to" >&2
echo "the EXEMPT case statement above with a reference to the audit section." >&2
echo "Do not add general allowlists — every exemption requires per-pattern" >&2
echo "justification matching what's documented in the audit report." >&2
exit 1
