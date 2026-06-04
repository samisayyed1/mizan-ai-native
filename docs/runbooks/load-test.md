# Load test plan — Mizan production

**Track PROD PR-PROD-3 / Goal v3 §V Phase 11 step 3.**

Authoritative plan for the production-scale load test that gates
Gate 3 canary approval. The test verifies p99 latencies against
the §A19 budgets under realistic §23-flavoured traffic.

---

## Goals

The load test must answer:

1. **Do the §A19 latency budgets hold at production scale?**
   - Cold start < 1.2s
   - Chart paint < 200ms cached
   - Endpoint read p99 < 300ms
   - Endpoint sync p99 < 800ms
   - Agent intent p99 < 500ms / read tool p99 < 2s / write tool p99 < 5s
2. **Does the Fly.io autoscaler hold at canary scale?**
   - 5% canary (~50 users) → 25% (~250) → 100% (~1000) cohort sizes
3. **Does Supabase Postgres absorb the read-volume without
   degradation?**
   - News-feed personalization queries (pgvector) are the hottest path
4. **Do any of the Tier-1 Sentry alerts (per `sentry-alerts.md`)
   fire during the test?**
   - If yes: the threshold needs adjustment OR the system has a real
     bottleneck.

---

## Fixture portfolio (the §23 reference user, ×N)

The load test seeds N synthetic users whose portfolios are
randomized variations on the §23 reference user (per
`Mizan_Continue_Autonomous_v3.md` lines 36-44):

- Sukuks at 3 issuers via 3 custodians (10-30K USD each)
- US + SG stocks (5-15 positions, total 50-200K)
- CPF cash + UT via Phillip/DBS SRS (10-50K)
- 1-2 ULIPs (5-30K surrender value)
- Sarwa ETFs incl. SPUS (5-20K)
- Physical cash across 2-4 jurisdictions (1-20K each)
- 1-5 real estate units with intent variation (200K-2M each)
- 0-5 PE positions (Hasan VC + HAPL + Wholesum + Stake fixture
  shapes, 25-500K each)

Total user wealth ranges from "above Nisab + a few hundred K" to
"~$10M" so the load test covers both the median user + the heavy-
tail user. **Crucially**: the fixture randomizer is **deterministic
on a seed** so reruns produce the same traffic pattern.

---

## Cohort plan

Three concurrent cohorts hitting the same staging cluster:

### Cohort A — Hawl-active cohort (10% of synthetic users)

- One Zakat computation per user per ramp
- Pay-Zakat flow initiated for 30% of these
- Hits the Truth Ledger append path on every calc
- Verifies the school-aware routing (PR-F2.b.1, PR-F2.c.1) under
  load

### Cohort B — Dashboard-browse cohort (60% of synthetic users)

- Open dashboard (cold start measured)
- Tap into 3-5 panels at random
- Trigger heatmap interactions, Today's Signal expansion, news
  page open
- Verifies §A19 chart-paint + endpoint-read budgets

### Cohort C — Sync-intensive cohort (30% of synthetic users)

- Plaid sync every 5 minutes
- SnapTrade sync every 10 minutes
- Webhook bursts from staging providers
- Verifies sync-success-rate Tier-1 alert thresholds

---

## Tooling

Test harness lives under `mizan-connect/tests/load/`:

- `synthetic_users.rs` — fixture-portfolio generator (deterministic
  on `SEED` env)
- `cohort_a.rs`, `cohort_b.rs`, `cohort_c.rs` — per-cohort traffic
  drivers using `tokio` + `reqwest`
- `metrics.rs` — per-cohort p50 / p90 / p99 latency recorder, writes
  to a CSV for post-run analysis
- `run.sh` — orchestrator that ramps cohorts on a schedule:
  - 0-15 min: 5% cohort (50 synthetic users)
  - 15-30 min: 25% (250 users)
  - 30-60 min: 100% (1000 users)
  - 60-90 min: 200% burst test (2000 users) to find the
    failure mode

External dependencies:
- A staging Fly.io app (`mizan-connect-staging`) running the same
  code as production
- A staging Supabase project with `news_items` / `holdings` /
  `truth_ledger` seeded for the synthetic users
- A staging Stripe project in test-mode
- Anthropic test-mode budget (the AI tools cap per the cost-
  discipline budget)

---

## Pass/fail criteria

**The load test PASSES when:**

- p99 latencies for each of the §A19 budgets stay within budget
  during the 100% cohort window
- Zero Tier-1 Sentry alerts fire during the 100% window
- Tier-2 alerts that fire are documented as "expected under
  burst" or are tuned in a follow-up PR
- Fly.io autoscaler completes each ramp without dropping
  connections
- Supabase p99 query latency stays under the §A19 endpoint
  budget (300ms read, 800ms sync)

**The load test FAILS when:**

- Any §A19 budget breached at 100% scale (not just burst)
- Any Tier-1 Sentry alert fires
- Truth Ledger chain-integrity verifier reports any error
  (zero-tolerance per CLAUDE.md §0 rule 1)
- Any token-encryption decryption failure (would indicate the
  rotation discipline has drifted)

A failed load test blocks Gate 3 canary approval. The operator
files an incident, fixes the bottleneck or threshold, and re-runs
the load test before the canary opens.

---

## Schedule

Pre-launch: run the full plan once, hand-validate the metrics
CSV against §A19 budgets, file the run report under
`docs/audit/<YYYY-MM-DD>-load-test-report.md`.

Recurring: re-run the load test before every major version bump
that touches:
- Truth Ledger / financial-truth code
- Sync provider integrations (any provider in
  `secrets-inventory.md`)
- AI tool registry (per CLAUDE.md §15 cost discipline)
- The dashboard cold-start path

---

## Out of scope (deferred)

- **PR-PROD-3.b** — Implementation of the `synthetic_users.rs`
  generator + cohort drivers. This runbook is the *plan*; the
  implementation is a follow-up PR with its own review surface.
- **PR-PROD-3.c** — CI integration: nightly load test against
  staging with auto-rollback if budgets breach.
