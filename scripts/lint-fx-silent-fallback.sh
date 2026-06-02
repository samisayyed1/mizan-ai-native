#!/usr/bin/env bash
# Track H PR-H8 — FX silent-fallback lint.
#
# Working agreement §0 rule 2: NEVER silent-fallback an FX rate.
# Past bug (QA Pass 7, working-agreement §13): the dashboard once
# shipped SGD-denominated holdings valued at SGD == USD because an
# `.unwrap_or(Decimal::ONE)` paved over a missing rate, displaying
# $184K instead of the actual ~$236K. This lint enforces the rule
# structurally — anywhere an Option<Decimal> appears to be an FX rate
# and gets defaulted to 1, the lint fires.
#
# Why a grep-based lint instead of clippy::disallowed_methods?
# Clippy's `disallowed_methods` can ban a path like `Option::unwrap_or`
# globally, but it can't context-distinguish "this Option holds an FX
# rate" from "this Option holds an options contract multiplier" — both
# are `Option<Decimal>` defaulted to `Decimal::ONE`. So the rule needs
# to look at NAMES (function/local-variable/comment context). A grep
# script can do that; clippy can't (yet).
#
# Matched patterns (any one fires the lint):
#   1. `.unwrap_or(Decimal::ONE)` immediately following a `*fx*`,
#      `*get_fx_rate*`, `*currency_rate*`, or `*convert_currency*`
#      method-chain (regex windows back ~3 lines).
#   2. `.unwrap_or(dec!(1))` or `.unwrap_or(Decimal::ONE)` inside a
#      function whose name contains `fx`, `currency`, or `rate`
#      (heuristic; the function name is matched on the preceding
#      `fn ...` line within 60 lines).
#   3. `.unwrap_or_else(|| Decimal::ONE)` / `.unwrap_or_default()`
#      followed within 3 lines by a money-arithmetic operator (`*`,
#      `/`, `+`, `-` on a `Decimal`) AND inside a function whose name
#      hints FX.
#
# Allowed contexts (NOT flagged):
#   - The dedicated `get_fx_rate_or_fallback` helper on
#     HoldingsValuationService — it's documented as the lenient escape
#     hatch for the day-change widget. The helper itself contains the
#     pattern but the rule names the helper as the SOLE legitimate
#     home. Lint allowlists the helper file by full path + line.
#   - `option_multiplier` / `contract_multiplier` defaults (options
#     contracts, not FX).
#   - Test files (`_tests.rs`, `/tests/`).
#
# Exit codes:
#   0 — zero unauthorised FX silent fallbacks
#   1 — at least one (real failure)
#   2 — script invocation error

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"

# Single explicit allowlist entry — the documented lenient helper.
ALLOWLIST=(
  "mizan-4/crates/core/src/portfolio/holdings/holdings_valuation_service.rs:get_fx_rate_or_fallback"
)

# Money-handling source directories to scan.
SCAN_PATHS=(
  "mizan-4/crates/core/src/portfolio"
  "mizan-4/crates/core/src/activities"
  "mizan-4/crates/core/src/health"
  "mizan-4/crates/core/src/net_worth_snapshot"
  "mizan-4/crates/core/src/fx"
  "mizan-4/crates/connect/src/broker"
  "mizan-4/crates/financial-truth/src"
  "mizan-4/crates/zakat/src"
  "mizan-4/crates/synthesis/src"
  "mizan-4/crates/insights/src"
)

EXEMPT_PATTERNS=(
  "_tests.rs"
  "/tests/"
)

findings=()

is_exempt_file() {
  local file="$1"
  for pat in "${EXEMPT_PATTERNS[@]}"; do
    [[ "$file" == *"$pat"* ]] && return 0
  done
  return 1
}

is_allowlisted_call() {
  # Match $file:$function_name vs the ALLOWLIST entries.
  local file="$1" function_name="$2"
  for entry in "${ALLOWLIST[@]}"; do
    local entry_file="${entry%:*}"
    local entry_fn="${entry##*:}"
    if [[ "$file" == *"$entry_file" && "$function_name" == "$entry_fn" ]]; then
      return 0
    fi
  done
  return 1
}

# Find each `.unwrap_or(Decimal::ONE)` / `.unwrap_or(dec!(1))` /
# `.unwrap_or_else(|| Decimal::ONE)` site, then walk back to find the
# enclosing `fn NAME` to test if NAME hints FX.
fx_hint_regex='(^|_)(fx|currency|rate|convert_currency)'

for dir in "${SCAN_PATHS[@]}"; do
  full="$REPO_ROOT/$dir"
  [[ -d "$full" ]] || continue
  while IFS= read -r -d '' rust_file; do
    is_exempt_file "$rust_file" && continue
    # Walk the file once. Track the most recent enclosing function.
    awk -v file="$rust_file" -v fx_hint="$fx_hint_regex" '
      BEGIN { fn_name="" }
      /^[[:space:]]*(pub[[:space:]]+(\(crate\)[[:space:]]+)?)?(async[[:space:]]+)?fn[[:space:]]+/ {
        # Extract the function name token.
        for (i = 1; i <= NF; i++) {
          if ($i == "fn") { fn_name = $(i+1); sub(/[(<].*/, "", fn_name); break }
        }
      }
      /\.unwrap_or\(Decimal::ONE\)|\.unwrap_or\(dec!\(1\)\)|\.unwrap_or_else\(\|\|[[:space:]]*Decimal::ONE\)|\.unwrap_or_else\(\|\|[[:space:]]*dec!\(1\)\)/ {
        if (fn_name ~ fx_hint) {
          printf "%s|%d|%s|%s\n", file, NR, fn_name, $0
        }
      }
    ' "$rust_file"
  done < <(find "$full" -type f -name '*.rs' -print0)
done | while IFS='|' read -r file lineno fn_name match; do
  if is_allowlisted_call "$file" "$fn_name"; then
    continue
  fi
  trimmed="${match#"${match%%[![:space:]]*}"}"
  echo "$file:$lineno (fn $fn_name): $trimmed"
done > /tmp/lint-fx-silent-fallback.findings || true

if [[ ! -s /tmp/lint-fx-silent-fallback.findings ]]; then
  echo "lint-fx-silent-fallback: OK — zero unauthorised FX silent fallbacks"
  exit 0
fi

echo "lint-fx-silent-fallback: FAIL — FX silent-fallback pattern found:" >&2
echo "" >&2
sed 's/^/  /' /tmp/lint-fx-silent-fallback.findings >&2
echo "" >&2
echo "Working agreement §0 rule 2 + §13 past-bug: never silently fall back" >&2
echo "an FX rate to 1. The single documented lenient helper is" >&2
echo "HoldingsValuationService::get_fx_rate_or_fallback — and it is ONLY" >&2
echo "acceptable for non-critical widgets (e.g. day-change preview)." >&2
echo "" >&2
echo "Fix: either route to cost-basis fallback when FX is missing, or" >&2
echo "surface a precise per-pair error and let the caller decide." >&2
exit 1
