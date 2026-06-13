#!/usr/bin/env bash
#
# Pre-canary verification — the final gate before flipping a release to
# any user traffic. Distinct from `release-gate.sh` (which verifies the
# code itself); this one verifies the *environment* a release is about
# to ship into.
#
#     ./scripts/pre-canary-check.sh
#     ./scripts/pre-canary-check.sh --env=staging   # default
#     ./scripts/pre-canary-check.sh --env=production
#
# Exits 0 if every check passes. Exits 1 if any fails — the release
# engineer must resolve each FAIL before flipping the canary toggle.
#
# Designed for human eyes: every check prints a clear PASS / FAIL /
# SKIP line with one-sentence rationale. SKIPs do not count against
# pass/fail; they are environment-specific items the operator may
# legitimately omit (e.g. Stripe live-mode check on a staging run).
#
# Companion to `PRODUCTION_HANDOFF.md` "10-item pre-canary checklist".

set -u

ENV="${1#--env=}"
if [[ -z "$ENV" || "$ENV" == "--env" ]]; then
  ENV="staging"
fi

# ─── pretty output ────────────────────────────────────────────────────
RED=$'\033[0;31m'
GREEN=$'\033[0;32m'
YELLOW=$'\033[1;33m'
BOLD=$'\033[1m'
DIM=$'\033[2m'
RESET=$'\033[0m'

PASSED=0
FAILED=0
SKIPPED=0

pass()  { echo "${GREEN}  PASS${RESET}  $1 ${DIM}— $2${RESET}"; PASSED=$((PASSED + 1)); }
fail()  { echo "${RED}  FAIL${RESET}  $1 ${DIM}— $2${RESET}"; FAILED=$((FAILED + 1)); }
skip()  { echo "${YELLOW}  SKIP${RESET}  $1 ${DIM}— $2${RESET}"; SKIPPED=$((SKIPPED + 1)); }

heading() {
  echo
  echo "${BOLD}$1${RESET}"
}

echo "${BOLD}Pre-canary check — environment: ${ENV}${RESET}"

# ─── 1. Encryption keys present in Fly secrets ────────────────────────
heading "1. Encryption keys present in Fly secrets (mizan-connect)"
if ! command -v fly &>/dev/null; then
  skip "fly CLI" "not installed locally; cannot enumerate Fly secrets"
else
  REQUIRED_SECRETS=(
    "MIZAN_PLAID_TOKEN_ENCRYPTION_KEY"
    "MIZAN_BROKER_SECRET_ENCRYPTION_KEY"
    "MIZAN_SNAPTRADE_STATE_SECRET"
    "STRIPE_SECRET_KEY"
    "STRIPE_WEBHOOK_SECRET"
    "SUPABASE_SERVICE_ROLE_KEY"
  )
  SECRETS_LIST=$(fly secrets list --app mizan-connect 2>/dev/null || echo "")
  if [[ -z "$SECRETS_LIST" ]]; then
    skip "fly secrets" "could not query — run \`fly auth login\` first"
  else
    for s in "${REQUIRED_SECRETS[@]}"; do
      if echo "$SECRETS_LIST" | grep -q "$s"; then
        pass "$s" "present"
      else
        fail "$s" "missing — set with \`fly secrets set $s=... --app mizan-connect\`"
      fi
    done
  fi
fi

# ─── 2. gitleaks scan ─────────────────────────────────────────────────
heading "2. gitleaks scan (no committed secrets)"
if ! command -v gitleaks &>/dev/null; then
  skip "gitleaks" "not installed (\`brew install gitleaks\`)"
else
  if gitleaks detect --no-banner --no-git --redact --exit-code=0 \
       --source=. --report-format=csv --report-path=/tmp/gitleaks.csv 2>/dev/null
  then
    if [[ -s /tmp/gitleaks.csv && $(wc -l < /tmp/gitleaks.csv) -gt 1 ]]; then
      fail "secret scan" "gitleaks found findings — review /tmp/gitleaks.csv"
    else
      pass "secret scan" "no findings"
    fi
  else
    fail "secret scan" "gitleaks exited non-zero — investigate"
  fi
fi

# ─── 3. Paid services on production tier ──────────────────────────────
heading "3. Paid services on production tier (manual confirmation)"
if [[ "$ENV" != "production" ]]; then
  skip "paid-tier check" "running against ${ENV}; production-tier verification only applies to production"
else
  echo "  ${DIM}Confirm in the relevant dashboards:${RESET}"
  echo "    • Plaid: Production environment selected (not Sandbox)"
  echo "    • SnapTrade: Production environment (api.snaptrade.com)"
  echo "    • Twelve Data: paid plan with required throughput"
  echo "    • MetalpriceAPI: paid plan if expecting > 50 req/month"
  echo "    • Resend: domain verified, on a paid plan"
  echo "  ${DIM}Enter Y when verified, anything else to mark FAIL:${RESET}"
  read -r -p "  > " ANS
  if [[ "$ANS" =~ ^[Yy]$ ]]; then
    pass "paid-tier" "operator confirmed"
  else
    fail "paid-tier" "operator did not confirm — block the canary"
  fi
fi

# ─── 4. Stripe live mode ──────────────────────────────────────────────
heading "4. Stripe live mode (production only)"
if [[ "$ENV" != "production" ]]; then
  skip "stripe live mode" "running against ${ENV}; check only applies to production"
elif ! command -v fly &>/dev/null; then
  skip "stripe live mode" "fly CLI not installed locally"
else
  # Check first 4 chars of the Stripe secret key surfaced via the
  # admin debug endpoint (only when --env=production AND
  # MIZAN_ADMIN_TOKEN is exported in the operator's shell).
  if [[ -z "${MIZAN_ADMIN_TOKEN:-}" ]]; then
    skip "stripe live mode" "MIZAN_ADMIN_TOKEN not exported; cannot inspect remote config"
  else
    KEY_PREFIX=$(curl -sS \
      -H "Authorization: Bearer $MIZAN_ADMIN_TOKEN" \
      https://mizan-connect.fly.dev/v1/admin/config/stripe-key-prefix 2>/dev/null \
      | grep -oE '"prefix":"[^"]+"' | cut -d'"' -f4)
    if [[ "$KEY_PREFIX" == "sk_live_" ]]; then
      pass "stripe live mode" "STRIPE_SECRET_KEY is sk_live_…"
    elif [[ "$KEY_PREFIX" == "sk_test_" ]]; then
      fail "stripe live mode" "STRIPE_SECRET_KEY is still sk_test_… — rotate to live before canary"
    else
      fail "stripe live mode" "could not verify Stripe key prefix (response: ${KEY_PREFIX:-empty})"
    fi
  fi
fi

# ─── 5. Stripe webhook signature ──────────────────────────────────────
heading "5. Stripe webhook signature verification"
if [[ "$ENV" == "production" ]]; then
  # Fire a synthetic webhook with a bogus signature and confirm the
  # production server returns 401 — this proves signature verification
  # is wired and not bypassed.
  STATUS=$(curl -sS -o /dev/null -w "%{http_code}" \
    -X POST https://mizan-connect.fly.dev/api/v1/stripe/webhook \
    -H "Stripe-Signature: t=0,v1=000000000000000000000000000000000000000000000000000000000000dead" \
    -H "Content-Type: application/json" \
    --data '{"id":"evt_test","type":"invoice.paid"}' 2>/dev/null)
  if [[ "$STATUS" == "401" ]]; then
    pass "webhook sig" "bogus signature rejected with 401"
  else
    fail "webhook sig" "expected 401, got ${STATUS} — verify webhook secret is configured"
  fi
else
  skip "webhook sig" "running against ${ENV}; check only applies to production"
fi

# ─── 6. Supabase service-role key not in desktop ─────────────────────
heading "6. Supabase service-role key not bundled into desktop"
DESKTOP_BUNDLES=(
  "mizan-4/apps/frontend/dist"
  "mizan-4/dist"
  "mizan-4/target/release"
)
FOUND_LEAK=0
for dir in "${DESKTOP_BUNDLES[@]}"; do
  if [[ -d "$dir" ]]; then
    if grep -rIlE "service_role|SUPABASE_SERVICE_ROLE_KEY" "$dir" >/dev/null 2>&1; then
      fail "service_role leak ($dir)" "found service_role reference in built desktop artifact"
      FOUND_LEAK=1
    fi
  fi
done
if [[ $FOUND_LEAK -eq 0 ]]; then
  pass "service_role" "no service_role reference in built desktop artifacts (or no built artifacts present)"
fi

# ─── 7. Anthropic spending cap configured ─────────────────────────────
heading "7. Anthropic monthly spending cap"
if ! command -v fly &>/dev/null; then
  skip "anthropic cap" "fly CLI not installed locally"
elif [[ -z "${MIZAN_ADMIN_TOKEN:-}" ]]; then
  skip "anthropic cap" "MIZAN_ADMIN_TOKEN not exported; cannot inspect remote config"
else
  # Hits an admin endpoint that surfaces the configured cap (in USD)
  # without revealing the API key itself.
  CAP=$(curl -sS \
    -H "Authorization: Bearer $MIZAN_ADMIN_TOKEN" \
    https://mizan-connect.fly.dev/v1/admin/config/anthropic-cap 2>/dev/null \
    | grep -oE '"monthly_usd":[0-9]+' | cut -d':' -f2)
  if [[ -n "$CAP" && "$CAP" -gt 0 ]]; then
    pass "anthropic cap" "monthly cap = \$${CAP}"
  else
    fail "anthropic cap" "no cap configured — set ANTHROPIC_MONTHLY_USD_CAP before canary"
  fi
fi

# ─── 8. Encryption key lengths ────────────────────────────────────────
heading "8. Encryption key lengths (32-byte / 64-hex)"
if [[ -z "${MIZAN_ADMIN_TOKEN:-}" ]]; then
  skip "key lengths" "MIZAN_ADMIN_TOKEN not exported; cannot inspect remote config"
else
  REPORT=$(curl -sS \
    -H "Authorization: Bearer $MIZAN_ADMIN_TOKEN" \
    https://mizan-connect.fly.dev/v1/admin/config/key-lengths 2>/dev/null)
  if [[ -z "$REPORT" ]]; then
    skip "key lengths" "admin endpoint did not respond (may not be deployed)"
  else
    if echo "$REPORT" | grep -q '"ok":true'; then
      pass "key lengths" "all encryption keys are 32 bytes (64 hex chars)"
    else
      fail "key lengths" "one or more keys are not 32 bytes — see $REPORT"
    fi
  fi
fi

# ─── 9. Code-signing certificate expiry > 90 days ─────────────────────
heading "9. Code-signing certificate expiry (> 90 days)"
APPLE_CERT="${APPLE_DEVELOPER_ID_CERT_PATH:-}"
WIN_CERT="${WINDOWS_CODE_SIGN_CERT_PATH:-}"
if [[ -z "$APPLE_CERT" && -z "$WIN_CERT" ]]; then
  skip "cert expiry" "neither APPLE_DEVELOPER_ID_CERT_PATH nor WINDOWS_CODE_SIGN_CERT_PATH is set"
else
  for label in "apple:$APPLE_CERT" "windows:$WIN_CERT"; do
    plat=${label%%:*}
    path=${label#*:}
    [[ -z "$path" ]] && continue
    if [[ ! -f "$path" ]]; then
      fail "cert ($plat)" "file not found at $path"
      continue
    fi
    EXPIRY=$(openssl x509 -noout -enddate -in "$path" 2>/dev/null | cut -d'=' -f2)
    if [[ -z "$EXPIRY" ]]; then
      fail "cert ($plat)" "could not parse cert at $path"
      continue
    fi
    EXPIRY_EPOCH=$(date -j -f "%b %d %H:%M:%S %Y %Z" "$EXPIRY" "+%s" 2>/dev/null \
      || date -d "$EXPIRY" "+%s" 2>/dev/null \
      || echo "0")
    NOW_EPOCH=$(date "+%s")
    DAYS_LEFT=$(( (EXPIRY_EPOCH - NOW_EPOCH) / 86400 ))
    if [[ "$DAYS_LEFT" -gt 90 ]]; then
      pass "cert ($plat)" "expires in $DAYS_LEFT days"
    else
      fail "cert ($plat)" "expires in $DAYS_LEFT days — rotate before canary"
    fi
  done
fi

# ─── 10. Sentry alerts configured ────────────────────────────────────
heading "10. Sentry alerts configured (manual confirmation)"
if [[ "$ENV" != "production" ]]; then
  skip "sentry alerts" "running against ${ENV}; check only applies to production"
else
  echo "  ${DIM}Confirm in Sentry dashboard:${RESET}"
  echo "    • Project: mizan-connect — alert rules exist for error spike + p95 latency"
  echo "    • Project: mizan-desktop — alert rules exist for crash-free sessions < 99.5%"
  echo "    • PagerDuty / Slack integration is wired and last-tested in the past 30 days"
  echo "  ${DIM}Enter Y when verified, anything else to mark FAIL:${RESET}"
  read -r -p "  > " ANS
  if [[ "$ANS" =~ ^[Yy]$ ]]; then
    pass "sentry alerts" "operator confirmed"
  else
    fail "sentry alerts" "operator did not confirm — block the canary"
  fi
fi

# ─── summary ──────────────────────────────────────────────────────────
echo
echo "${BOLD}Summary${RESET}"
echo "  ${GREEN}PASS${RESET}: $PASSED"
echo "  ${RED}FAIL${RESET}: $FAILED"
echo "  ${YELLOW}SKIP${RESET}: $SKIPPED"
echo

if [[ "$FAILED" -gt 0 ]]; then
  echo "${RED}Canary BLOCKED.${RESET} Resolve every FAIL above before flipping the toggle."
  exit 1
elif [[ "$SKIPPED" -gt 0 ]]; then
  echo "${YELLOW}Canary GATED.${RESET} The PASS checks succeeded but $SKIPPED check(s) were skipped. Re-run with the missing tooling/credentials before production canary."
  exit 0
else
  echo "${GREEN}Canary CLEARED.${RESET} All ${PASSED} checks passed. Proceed with flip."
  exit 0
fi
