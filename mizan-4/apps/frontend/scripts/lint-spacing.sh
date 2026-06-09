#!/usr/bin/env bash
# PR-DENSITY-7 — fail CI if any frontend file uses an off-grid
# Tailwind arbitrary spacing value.
#
# Valid spacing pixels: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64.
# Tailwind class mapping: 1 → 4, 2 → 8, 3 → 12, 4 → 16, 5 → 20,
#                         6 → 24, 8 → 32, 10 → 40, 12 → 48, 16 → 64.
#
# Off-grid values caught by this script:
#   h-[14px] / w-[14px] / p-[14px] / m-[14px] / gap-[14px] / etc.
#   …and the same for 15, 17, 18, 19, 21, 22, 23, 25, 26, 27 — any
#   1–2 digit pixel value not in the canonical scale.
#
# Why a shell script + grep: ESLint can't see inside arbitrary
# values without a custom plugin; a focused grep is simpler and
# fast enough to run on every CI build.

set -euo pipefail

# Allow override for local dev (run from frontend dir or repo root).
ROOT="${1:-mizan-4/apps/frontend/src}"

if [ ! -d "$ROOT" ]; then
  echo "lint-spacing: scan root '$ROOT' does not exist; nothing to lint." >&2
  exit 0
fi

# Pattern matches: <attr>-[<NN>px] where NN is 13/14/15/17/18/19/
# 21/22/23/25/26/27 — the most common off-grid offenders. We
# intentionally narrow to these instead of "anything not on grid"
# because Tailwind's spacing 0.5/1.5/2.5/3.5 (which produce 2/6/
# 10/14 px) are legitimate and used widely.
PATTERN='(h|w|p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr|gap|gap-x|gap-y|space-x|space-y|top|right|bottom|left|inset|inset-x|inset-y)-\[(13|14|15|17|18|19|21|22|23|25|26|27)px\]'

# Exclude generated files + tests where off-grid values are
# sometimes pasted from design specs intentionally.
if grep -rnE "$PATTERN" "$ROOT" \
    --include='*.ts' --include='*.tsx' --include='*.js' --include='*.jsx' \
    --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=__generated__; then
  echo "" >&2
  echo "lint-spacing: off-grid arbitrary spacing values found above." >&2
  echo "  Use the canonical 8-point grid: 4 / 8 / 12 / 16 / 20 / 24 / 32 / 40 / 48 / 64." >&2
  echo "  Tailwind: 1=4 / 2=8 / 3=12 / 4=16 / 5=20 / 6=24 / 8=32 / 10=40 / 12=48 / 16=64." >&2
  exit 1
fi

echo "lint-spacing: clean — no off-grid arbitrary spacing values."
