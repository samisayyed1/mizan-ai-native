#!/usr/bin/env bash
#
# Release-gate verification (§A25, first cut).
#
# Runs the seven checks every Phase PR's verification table documents,
# in the same order, with timing and a final pass/fail summary.
# Designed to be run from the monorepo root.
#
#     ./scripts/release-gate.sh
#
# Exit status: 0 if every check passed, 1 if any failed. CI-friendly.
# Set GATE_FAST=1 to skip the long ones (test + build) for a quick
# pre-commit sanity pass.
#
# This does NOT cover the manual "No Silent Failure" walks Feroz called
# out (disconnect internet, break Plaid token, etc.) — those live in
# the §A25 spec and need a release-engineer sitting in front of the
# unsigned DMG.

set -u

# ─── pretty output ────────────────────────────────────────────────────
RED=$'\033[0;31m'
GREEN=$'\033[0;32m'
YELLOW=$'\033[1;33m'
DIM=$'\033[2m'
RESET=$'\033[0m'

PASSED=0
FAILED=0
SKIPPED=0
FAILURES=()
TOTAL_START=$SECONDS

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIZAN4_DIR="$ROOT_DIR/mizan-4"
CONNECT_DIR="$ROOT_DIR/mizan-connect"

print_header() {
  echo
  echo "${YELLOW}── $1 ──${RESET}"
}

run_check() {
  local label="$1"
  local working_dir="$2"
  shift 2

  local start
  start=$SECONDS

  echo "${DIM}▸ ${label}${RESET}"
  if [ ! -d "$working_dir" ]; then
    echo "  ${YELLOW}↷ skipped — directory not present: $working_dir${RESET}"
    SKIPPED=$((SKIPPED + 1))
    return
  fi

  if (cd "$working_dir" && "$@") >/tmp/release-gate.log 2>&1; then
    local elapsed=$((SECONDS - start))
    echo "  ${GREEN}✓ pass${RESET} ${DIM}(${elapsed}s)${RESET}"
    PASSED=$((PASSED + 1))
  else
    local elapsed=$((SECONDS - start))
    echo "  ${RED}✗ fail${RESET} ${DIM}(${elapsed}s)${RESET}"
    echo "    last 20 lines of output:"
    tail -n 20 /tmp/release-gate.log | sed "s/^/    ${DIM}|${RESET} /"
    FAILED=$((FAILED + 1))
    FAILURES+=("$label")
  fi
}

# ─── Rust workspace ──────────────────────────────────────────────────
print_header "Rust: mizan-4 workspace"
run_check "cargo check --workspace" "$MIZAN4_DIR" cargo check --workspace --quiet
run_check "cargo clippy (no -D warnings yet — surface only)" "$MIZAN4_DIR" cargo clippy --workspace --quiet
if [ "${GATE_FAST:-}" != "1" ]; then
  run_check "cargo test --lib (workspace)" "$MIZAN4_DIR" cargo test --lib --workspace --quiet
else
  echo "${DIM}↷ skipping cargo test — GATE_FAST=1${RESET}"
  SKIPPED=$((SKIPPED + 1))
fi

# ─── Rust: mizan-connect (separate workspace) ────────────────────────
print_header "Rust: mizan-connect"
run_check "cargo check" "$CONNECT_DIR" cargo check --quiet

# ─── Frontend ────────────────────────────────────────────────────────
print_header "Frontend: pnpm verification matrix"
run_check "pnpm install (frozen lockfile)" "$MIZAN4_DIR" pnpm install --frozen-lockfile --silent
run_check "pnpm type-check" "$MIZAN4_DIR" pnpm --filter frontend type-check
run_check "pnpm lint:quiet" "$MIZAN4_DIR" pnpm --filter frontend lint:quiet
if [ "${GATE_FAST:-}" != "1" ]; then
  run_check "pnpm test -- --run" "$MIZAN4_DIR" pnpm --filter frontend test -- --run --reporter=dot
  run_check "pnpm build" "$MIZAN4_DIR" pnpm --filter frontend build
else
  echo "${DIM}↷ skipping pnpm test + build — GATE_FAST=1${RESET}"
  SKIPPED=$((SKIPPED + 2))
fi

# ─── Summary ─────────────────────────────────────────────────────────
TOTAL_ELAPSED=$((SECONDS - TOTAL_START))
echo
echo "${YELLOW}── Summary ──${RESET}"
echo "  ${GREEN}passed ${PASSED}${RESET}, ${RED}failed ${FAILED}${RESET}, ${DIM}skipped ${SKIPPED}${RESET}"
echo "  ${DIM}total ${TOTAL_ELAPSED}s${RESET}"
if [ ${FAILED} -gt 0 ]; then
  echo "  ${RED}failures:${RESET}"
  for f in "${FAILURES[@]}"; do
    echo "    ${RED}·${RESET} $f"
  done
  echo
  echo "${RED}Release gate: NOT PASSED${RESET}"
  exit 1
fi
echo
echo "${GREEN}Release gate: PASSED${RESET}"
echo "${DIM}This script does not cover §A25's manual 'No Silent Failure' walks${RESET}"
echo "${DIM}(broken Plaid / SnapTrade / market data tokens, offline, etc).${RESET}"
echo "${DIM}Run those against the unsigned DMG before publishing.${RESET}"
exit 0
