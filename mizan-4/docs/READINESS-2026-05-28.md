# Mizan readiness report — 2026-05-28

End-to-end QA + production-grade hardening pass spanning a continuous
session of physical-app testing on the actual Tauri desktop + cloud
deployments. This report documents what's green, what's blocked on
external action, and what would still be required to call the system
"enterprise-grade" by an auditor's bar.

---

## TL;DR

✅ **Ready for sandbox/test-mode end-to-end use today.**
🟡 **Two things blocked on operator action** outside the code (Stripe
Dashboard / SnapTrade Developer Dashboard).
✅ **Production DMG builds clean** at `target/release/bundle/dmg/Mizan
AI_3.4.1_aarch64.dmg` (164 MB, fresh as of this session).

---

## Test posture

| Suite | Count | Status |
| --- | --- | --- |
| Desktop workspace tests | **1 912 passed / 0 failed** | 🟢 |
| Cloud unit tests | **72 passed / 0 failed** | 🟢 |
| Cloud integration tests (Postgres via testcontainers) | **6+ passed / 0 failed** in this session (full suite green in earlier sessions, 19 total) | 🟢 |
| Stripe webhook signature tests (new) | **11 passed / 0 failed** | 🟢 |
| Playwright E2E route coverage | **14 routes / 14 passed** in last full run (Gap #2) | 🟢 |
| Workspace clippy `--all-targets -- -D warnings` | **clean** | 🟢 |
| `cargo audit` desktop | **0 vulnerabilities** (20 allowed warnings on GTK transitives) | 🟢 |
| `cargo audit` cloud | **0 vulnerabilities** (1 allowed warning on dev-only testcontainers dep) | 🟢 |
| Frontend `tsc --noEmit` (strict) | **clean** | 🟢 |

---

## Cloud deployment trail

`mizan-connect` on Fly went v32 → v38 across this session:

| Version | Change |
| --- | --- |
| v33 | Auto-create solo team on first checkout (`checkout-session` no longer 500s on Supabase-fresh users) |
| v34 | Stripe webhook also writes `team_id` (out-of-order Stripe delivery handled) |
| v35 | `--no-cache` rebuild to flush stale binary layers |
| v37 | QA admin endpoint (opt-in via `MIZAN_ADMIN_TOKEN`) with audit log |
| v38 | Multi-secret webhook (`STRIPE_WEBHOOK_SECRET` = `OLD,NEW`) for zero-downtime rotation + duplicate-endpoint tolerance |

`/health` smokes green at 110–180 ms cold-cache from a residential
connection.

---

## Real bugs uncovered + fixed

1. **`PLAID_REDIRECT_URI` treated as boot-required** even though Plaid
   only needs it for OAuth-required institutions. Unsetting it
   crash-looped both Fly machines. Now `Option<String>` end-to-end;
   sandbox non-OAuth banks (First Platypus etc.) work without any
   redirect configured. *Commit `2635cfe`.*

2. **Stripe Checkout 500ed for users registered after migration 0005**
   because the `subscriptions` row INSERT lacked `team_id`. Added
   `ensure_solo_team()` invoked at checkout-creation; lazy-creates the
   solo team using migration 0005's `team_id == user_id` invariant.
   *Commits `429a0de`, `a3ef626`.*

3. **Stripe webhook for `customer.subscription.{created,updated}`
   hit the same NOT NULL violation** even when checkout succeeded
   (out-of-order Stripe delivery). Webhook upsert now writes
   `team_id` and re-asserts the solo team in the same tx. *Commit
   `a3ef626`.*

4. **`hasSubscription = hasBrokerSync(userInfo)` conflation in the
   Connect page** meant Silver users saw the IDENTICAL UI to Free
   users — same upgrade grid, no badge, no acknowledgement of payment.
   Replaced with a real `CurrentPlanCard` that distinguishes "no
   subscription" / "Silver (Plus Gold CTA)" / "Gold". *Commit
   `16e4cbd`.*

5. **`SubscriptionPlans` component was unmounted** behind a "Plans
   & billing — COMING SOON" placeholder, so the only path to upgrade
   in-app was unreachable. Re-mounted with `onRefresh={refetchUserInfo}`
   so the post-checkout window-focus listener refreshes `/v1/me`.
   *Commit `0b503c2`.*

6. **`text_to_datetime` in `crates/storage-sqlite/src/taxonomies/model.rs`
   only tried RFC3339** even though the taxonomies table also stores
   `CURRENT_TIMESTAMP` strings (`2026-05-25 15:16:19`). Result: 200+
   fatal-looking ERROR lines in the dev log per page open. Now tries
   five formats. *Commit `16e4cbd`.*

7. **SnapTrade portal failures dumped raw `API error 500 …` toasts**
   on the user. Mapped the three failure modes (account-config 405,
   signature 401, deployment 503) to one-sentence actionable hints
   that tell the user where to look (their SnapTrade Dashboard, our
   Fly secrets, or the deployment posture). *Commit `16e4cbd`.*

8. **Stripe Customer Portal failed with `404 not found`** on
   admin-granted users (no `stripe_customer_id`). Wrapped in a
   "No Stripe billing record yet" toast that points the user at the
   real fix — re-run a fresh checkout. *Commit `16e4cbd`.*

9. **Web build broke** on import of `streamAgentChat` /
   `@/adapters/shared/*` paths after the per-target alias was
   introduced. Added a `streamAgentChat` stub to the web adapter
   that yields a clean `unsupported_platform` error event, and
   switched type-only imports to relative paths that bypass the
   build-target alias. *Commit `70e317d`.*

10. **`check_for_updates` Tauri command timed out in dev** because
    the updater endpoint isn't reachable for unreleased versions.
    Short-circuits to `Ok(None)` under `cfg(debug_assertions)` so
    the menu item resolves instantly. Production DMG path
    unchanged. *Commit `e02c7b7`.*

---

## Enterprise-grade hardening

* **Admin / break-glass surface (`/v1/admin/*`)** — opt-in via
  `MIZAN_ADMIN_TOKEN`, constant-time bearer compare, audit-logged on
  every grant via `tracing::info!` with structured `{user_id, tier,
  status}` fields. Two endpoints: read the user's full subscription
  + team state, and force-grant a tier/status. Used during this
  session to unstick a tester whose Stripe checkout succeeded but
  webhook crashed before persistence. *Commit `e8dc154`.*

* **Stripe webhook multi-secret support** — `STRIPE_WEBHOOK_SECRET`
  now accepts a comma-separated list. Enables:
  - **Rotation without downtime** (deploy with `OLD,NEW`, rotate
    Dashboard to `NEW`, deploy with `NEW`).
  - **Duplicate-endpoint tolerance** (a long-lived `stripe listen`
    CLI session sharing the cloud URL caused ~75 % of webhook events
    to 401 in this session — multi-secret would have absorbed both
    without any code change).
  Five new tests pin the contract. *Commit `5014c71`.*

* **Browser E2E coverage for every major route** (Gap #2 from prior
  readiness report) — 14 routes (dashboard, holdings, activities,
  import, connect, settings/*, assistant, zakat, reports,
  reports/monthly, advisor) all walked with anchor-element + URL
  + error-boundary assertions. *Commit `70e317d`.*

* **Cross-consistency invariants** with golden tests (already shipped):
  - Allocation % sums to exactly 100.00 (Largest-Remainder rounding).
  - FIFO realized P&L golden vectors.
  - TWR + Modified Dietz MWR known-answer math.
  - Truth-ledger hash chain integrity.
  - Zakat correctness across Cash / Investment / PrivateEquity /
    Other (no silent skips).
  - Entitlements parity (cloud ↔ desktop) frozen snapshot.

---

## NEEDS MANUAL VERIFICATION (operator action, out of repo)

| Item | Why | Concrete step |
| --- | --- | --- |
| **SnapTrade `/snapTrade/login` returns 405** for this client_id | Account-level endpoint allowlist on SnapTrade's side, not a Mizan code issue. Probed their API directly — endpoint accepts POST with `allow: POST, OPTIONS` for un-authed requests. | dashboard.snaptrade.com → your application → ensure the Connection Portal endpoint is enabled for the current environment. |
| **Stripe webhook ~75 % 401 rate** observed during checkout | Multi-secret fix shipped (v38), but the root cause appears to be a duplicate webhook endpoint registered against the same URL with a different signing secret. | Stripe Dashboard → Developers → Webhooks → check for duplicate endpoints; either delete the duplicate or set both secrets in `STRIPE_WEBHOOK_SECRET=OLD,NEW`. |
| **Production Stripe keys** | Test mode only this session per scope. | Live-mode keys + price IDs need to be set on Fly secrets prior to real billing. |
| **Apple Developer Program cert + notarization** | Out of scope per existing rules. | Provisioning, signing, notarization needed for distribution outside `pnpm tauri dev`. |

---

## Open + intentional gaps

* **Stripe Customer Portal on admin-granted subscriptions** returns
  "No Stripe billing record yet" — graceful by design, since
  out-of-band grants don't create a Stripe customer. Recovery path is
  one fresh checkout. *Intentional, documented in the toast.*

* **AI Assistant tags** (`add_ai_thread_tag`, `remove_ai_thread_tag`,
  `get_ai_thread_tags`) are no-op stubs on the Rust side; no UI
  currently triggers them so they're effectively dead. Worth either
  implementing or removing the hooks in a follow-up.

---

## Session commits (newest first)

```
e02c7b7  Updater — skip the network check in dev builds
7812ebf  Test-fixture — wrap PlaidConfig.redirect_uri in Some(…) after Option change
5014c71  Stripe-webhook — multi-secret support for rotation + duplicate-endpoint tolerance
e8dc154  QA-admin — opt-in /v1/admin surface for QA + ops break-glass
16e4cbd  QA Pass 5 — Current-plan card + graceful errors + log spam fix
a3ef626  Webhook-fix — write team_id from Stripe subscription upsert
429a0de  Checkout-fix — auto-create solo team on first checkout
0b503c2  Connect-upgrade — mount real SubscriptionPlans in place of Chunk-2 placeholder
2635cfe  Plaid-fix — PLAID_REDIRECT_URI is truly optional now
70e317d  QA Pass 4 — Browser E2E route coverage + web-build fixes
```

Plus QA Pass 1-3 from the prior session, totalling 13 substantive
commits in the production-readiness sweep.

---

## Recommended next moves (if continuing)

1. Resolve the SnapTrade Dashboard config so the broker portal works
   end-to-end.
2. De-duplicate the Stripe webhook endpoints in Stripe Dashboard
   (the multi-secret fix is a workaround, not a root-cause cure).
3. Wire the `add_ai_thread_tag` UI or remove the dead adapter hooks.
4. Decide on a Plaid OAuth redirect URI for production-tier banks
   (sandbox doesn't need one; live OAuth-required institutions do).
5. Schedule the manual Apple cert + notarization work so a DMG
   built off `main` can ship outside `tauri dev`.
