# Track C — AI-Native Depth

**Status:** Pending. Foundation work (tool registry + memory) unblocks Tracks D, F, J, K.
**Estimated sprints:** 6.
**Source:** `docs/plans/00-master-plan.md` → "Track C — AI-Native Depth".

## Scope

**In (foundation):** tool registry expansion (15+ new tools per spec §7.1), `user_memory` table + vector store + memory writer subroutine, conversational mutation depth, the "App without AI" offline banner.

**In (later):** multi-modal input (voice / image / document / screenshot), predictive layer (Monte Carlo, cash flow forecast, retirement projection, goal tracking), offline robustness with embedded local model, cost discipline instrumentation.

**Out:** Sharia screening (Track E); Zakat extensions (Track F); news synthesis (Track D); MCP (Track K).

## PRs (foundation — ships first)

| # | Status | Title |
|---|---|---|
| C1.a | ✅ Done | `user_memory` migration | `2026-06-02-000002_user_memory` with 10 columns + 4 indexes + soft-delete for GDPR |
| C1.b | ⏸️ Pending | Diesel schema regen + repository scaffolding (sqlite-vec embeddings on desktop, pgvector mirror on cloud for Gold+) |
| C2 | ⏸️ Pending | Memory editor UI (settings panel) — view, edit, delete every fact |
| C3 | ⏸️ Pending | Cloud mirror of memory for Gold+ — pgvector schema + sync worker |
| C3.b | ✅ Done (2026-06-04) | `register_tool!` macro + `ToolRegistration` struct + `AuditScope` / `NumericBounds` enums per ADR 0020 §"Compile-time enforcement". Closes audit Finding 11.1. Cap-weight `const`-validated against ADR 0020's 1..=8 range (compile-time panic on out-of-range literal). `debug_assert_truth_ledger_contract` runtime guard catches declared-vs-actual mismatch at registry boot. 12 unit tests + smoke test instantiating all 21 ADR 0020 inventory rows + ledger-emitter set assertion. |
| C4.d | ✅ Done (2026-06-04) | Safety descriptors for `get_user_memory`/`update_user_memory` (ADR 0020 rows 20-21). cap=1/3; audit `Custom("fact_keys_list")` (keys not values) + `Diff`; bounds `None` / `PerFactType`; both ledger=false (memory edits aren't Truth Ledger material per ADR 0020 §"Memory + tooling boundaries"). 6 unit tests including write-cap-greater-than-read invariant + neither-emits-truth-ledger guard. |
| C4.a | ✅ Done (2026-06-04) | Safety descriptors for `create_holding`/`update_holding`/`delete_holding` (ADR 0020 rows 1-3). First consumer of the `register_tool!` macro from C3.b — locks the AI Safety Runtime contract for the three holdings-mutation tools before handler impls land. 7 unit tests assert each row matches ADR 0020 (cap=5/5/3; audit=FullInput/Diff/IdOnly; bounds=Holding/Holding/None; all three emit Truth Ledger). |
| C4.a.1 | ⏸️ Pending | `create_holding` handler — draft-preview per the `create_account` pattern (consumes C4.a descriptor) |
| C4.a.2 | ⏸️ Pending | `update_holding` handler — diff-based update draft |
| C4.a.3 | ⏸️ Pending | `delete_holding` handler — confirmation prompt + ledger-deletion entry |
| C4.e | ✅ Done (2026-06-04) | Safety descriptors for `add_activity`/`list_activities`/`set_reminder`/`set_alert`/`get_news`/`run_scenario`/`compare_scenarios` (ADR 0020 rows 4, 5, 12, 13, 14, 18, 19). cap=5/1/2/2/1/8/5; `add_activity` is the **fifth Truth-Ledger-emitting tool** in the registry (only one in this catalog); `set_reminder` enforces 2-year future horizon (63072000s); `run_scenario` is heaviest tier (cap=8 for Monte Carlo). 11 unit tests including only-add-activity-emits-ledger invariant + 2-year horizon literal check + heaviest-cap-in-catalog sanity. **Completes the ADR 0020 inventory** — 22/22 rows now have safety descriptors registered through `register_tool!`. |
| C5 | ⏸️ Pending | Handlers (C4.e.1): `add_activity`, `list_activities` |
| C5.a | ✅ Done (2026-06-04) | Insights rule extensions: `BondMaturityApproaching` (90/30/7/1 day thresholds, severity Info/Info/Warning/Critical), `FxMovedMaterially` (3% threshold + $5K min exposure), `ShariaStatusChanged` (Warning, fires on verdict flip). Three new `NotificationKind` variants added forward-only per CLAUDE.md §0 rule 5 (with `from_str_lenient` + `as_str` round-trip). Three new `InsightsInput` fields. 14 unit tests including the §23 47-day Emaar Sukuk fixture, the stable-rule-order regression guard, and dedupe-key direction-includes pin. mizan-insights 30/30; mizan-core 1018/1018. |
| C5.b | ✅ Done (2026-06-04) | Insights rule extensions batch 2: `ZakatHawlApproaching` (30/7/1 day from `hawl_anchors` PR-F1, severity Info/Warning/Critical), `ConcentrationRisk` (25% net-worth threshold + $10K min exposure, Warning), `CashDragOpportunity` (1.5% yield gap + $5K min cash, Info — distinct from CashDrag duration rule), `TaxOptimizationWindow` (90/30/7/1 day per-jurisdiction window — CPF SA top-up, IRA, capital gains harvest). Four new `NotificationKind` variants forward-only. Four new `InsightsInput` fields. 18 new unit tests including the full-rule-set stable-order regression pin. mizan-insights 48/48; mizan-core 1018/1018. **Completes the ADR 0020 / Goal v3 §V Phase 4 insights rule inventory** — 7 of 7 new rules now in production. |
| C5.c | ✅ Done (2026-06-04) | `todays_signal.rs` — pure selection-algorithm scorer for the Today's Signal card surface (PR-A7 #97). `score_signal(&Notification) → u32` combines severity-tier (1000/100/10/5 for Critical/Warning/Info/Success) with per-kind bonus (action-shaped financial events +50, portfolio events +20, advisory +15, celebratory +10, operational +0). `pick_todays_signal(candidates, recent) → Option<Notification>` deduplicates against the last 7 days of bell-panel rows and picks the highest-scoring fresh candidate. Deterministic via lexicographic dedupe_key tiebreak. 11 unit tests including the §23 47-day Emaar Sukuk fixture, the bond-Warning-beats-FX-Warning kind-bonus pin, and the success-severity-underweights-info regression guard. Scheduler hydration (the wiring from real data sources) lands in PR-C5.d. |
| C5.d.1 | ✅ Done (2026-06-04) | Hawl-anchor scheduler hydration — first slice of the C5.d scheduler-wiring series. Adds `mizan_storage_sqlite::hawl::HawlAnchorRepository` reading `hawl_anchors` (PR-F1 migration) via diesel, with lunar-year arithmetic (`anchor_date + 354d`) + caller-tunable grace window. Exposes via `ServiceContext.hawl_anchor_repository`. `scheduler.rs::hydrate_hawl_anchors(context, today)` converts rows → `HawlAnchorCandidate` and replaces the `Vec::new()` stub in the `InsightsInput` constructor. Cohort labels derived via `pretty_cohort_label` ("cash-2026" → "Cash (2026)"). 4 unit tests on the label helper. mizan-storage-sqlite 174/174; mizan-insights 59/59. PR-C5.d.2+ will hydrate the remaining three rule families. |
| C5.d.2 | ✅ Done (2026-06-04) | Concentration-risk scheduler hydration — second slice of C5.d. `hydrate_concentration_findings(context, base_currency)` reads holdings via the existing `holdings_service.get_holdings(PORTFOLIO_TOTAL_ACCOUNT_ID, base_currency)`, computes per-issuer (keyed on `instrument.symbol`, name fallback to symbol) + per-currency rollup, emits `ConcentrationRiskFinding[]`. Engine filters against the 25% threshold + $10K floor from PR-C5.b. Pure-math helper `rollup_concentration_findings(&[Holding], base_currency)` extracted for testability. **7 unit tests** including §23-flavored "Emaar at 30% of 1M" issuer-pin, duplicate-symbol aggregation, name-empty-falls-to-symbol, multi-currency rollup, no-instrument-cash-bucket. mizan-app 74/74 unit tests; tsc / clippy clean. Sector + geography dimensions deferred to a follow-up (weighted multi-category distributions need a more nuanced rollup than the symbol/currency keys). PR-C5.d.3+ will hydrate cash-drag-opportunity + tax-window. |
| C5.d.3 | ✅ Done (2026-06-04) | Bond-maturity scheduler hydration — third slice of C5.d. `hydrate_bond_maturity_candidates(context, base_currency, today)` reads holdings via `holdings_service`, filters to bond-classified instruments via `is_bond_holding` (matches `BOND_*` / `DEBT_SECURITY` / `MONEY_MARKET_DEBT` / `SUKUK` `assetType.key` values — same predicate `eval_bond_maturity` walks), batch-fetches the underlying `Asset` rows via `asset_service.get_assets_by_asset_ids`, reads `Asset.bond_spec().maturity_date` from the existing JSON metadata, computes `days_to_maturity` against today, emits `BondMaturityCandidate[]`. Pure-math helper `bond_maturity_candidates_from(&[Holding], &HashMap<asset_id, Asset>, today)` extracted for testability. **10 unit tests** including the §23-flavored "Emaar 47-day" fixture, missing-maturity-skip, zero-principal-skip, asset-not-in-map-safe, past-date negative-days. Engine filters against 1/7/30/90 day thresholds from PR-C5.a. mizan-app 84/84 (was 74; +10); clippy clean. PR-C5.d.4+ will hydrate FX moves + Sharia status flips. |
| C5.d.4 | ✅ Done (2026-06-04) | FX-move scheduler hydration — fourth slice of C5.d. `hydrate_fx_pair_moves(context, base_currency)` reads holdings via `holdings_service`, rolls up exposure per non-base currency, calls `fx_service.get_historical_rates(currency, base, 30d)` per pair, computes the older→newer `change_pct`, emits `FxPairMove[]`. Two pure-math helpers extracted for testability: `exposure_by_currency(&[Holding], base)` (sums per-currency base-currency value, excludes base-currency holdings + zero/negative + case-insensitive base match) and `fx_change_pct(&[ExchangeRate])` (sorts by timestamp, computes `(newest - oldest) / oldest`, returns `None` on <2 samples or zero oldest). **9 unit tests** including reverse-input-order sort, negative-change weakening, zero-oldest divide-by-zero guard, case-insensitive USD/usd base match. Engine filters against `FX_MATERIAL_MOVE_PCT` (3%) + `FX_MIN_EXPOSURE_BASE` ($5K). mizan-app 93/93 (was 84; +9); clippy clean. PR-C5.d.5 will hydrate Sharia status changes. |
| C4.b | ✅ Done (2026-06-04) | Safety descriptors for `compute_net_worth`/`get_holding_history`/`get_market_data`/`get_fx_rate`/`bond_analytics` (ADR 0020 rows 6-9 + 16). All five are read-only (no Truth Ledger emission); cap weights 2/1/1/1/3; bond_analytics has `BondAsOf` numeric bounds; `get_fx_rate` audit_scope=`Pair` with handler invariant inline: returns `None` on missing rate per CLAUDE.md §0 rule 2 (PR-C7 handler will enforce). 8 unit tests. |
| C6 | ⏸️ Pending | Handlers (C4.b.1): `compute_net_worth`, `get_holding_history` |
| C7 | ⏸️ Pending | Handlers (C4.b.2): `get_market_data`, `get_fx_rate` (refuses to invent rates per QA Pass 8 + working agreement §0 rule 2) |
| C4.c | ✅ Done (2026-06-04) | Safety descriptors for `sync_account`/`generate_report`/`summarize_document`/`estimate_price`/`find_sharia_status` (ADR 0020 rows 10/11/15/17 + Goal v3 add). cap=5/3/8/5/1; sync_account redacts plaid_access_token + snaptrade_user_secret per CLAUDE.md §0 rule 4; summarize_document caps at 50K words across pdf/csv/xlsx/txt; estimate_price ledger=false (surfaces draft; update_holding path writes); find_sharia_status bridges to PR-E4 cloud worker. 9 unit tests including no_lifecycle_tool_emits_truth_ledger invariant + redact-list overzealousness guard. |
| C8 | ⏸️ Pending | Handlers (C4.c.1): `sync_account`, `generate_report` |
| C9 | ⏸️ Pending | Tools: `set_reminder`, `set_alert` |
| C10 | ⏸️ Pending | Tools: `get_news` (stub — Track D fills table) |
| C11 | ⏸️ Pending | Tools: `summarize_document` (PDF + CSV layout-aware parsing) |
| C12 | ⏸️ Pending | Tools: `bond_analytics`, `estimate_price` |
| C13 | ⏸️ Pending | Tools: `run_scenario`, `compare_scenarios` |
| C14 | ⏸️ Pending | System prompt update + Anthropic prompt cache invalidation hook + `crates/ai/prompts/CHANGELOG.md` entry |

## PRs (later)

| # | Status | Title |
|---|---|---|
| C14.aux | ✅ Done | Foundation migrations | `2026-06-02-000006_agent_audit_log` (12mo retention → archive) + `2026-06-02-000007_reconciliation_queue` (durable) |
| C15.a | ✅ Done | `projection_snapshots` foundation | `2026-06-02-000005_projection_snapshots` with 3 indexes + rollup-candidate index + fingerprint short-circuit index |
| C15 | ⏸️ Pending | Predictive layer — Monte Carlo NW trajectory wiring (consumes `projection_snapshots`) + `Rollup` impl |
| C16 | ⏸️ Pending | Predictive layer — cash flow forecast |
| C17 | ⏸️ Pending | Predictive layer — retirement projection + goal tracking dashboard chips |
| C18 | ⏸️ Pending | Multi-modal — voice input (Whisper local + cloud STT for Gold+) |
| C19 | ⏸️ Pending | Multi-modal — image OCR (bank statement photo) |
| C20 | ⏸️ Pending | Multi-modal — document upload (PDF / XLSX) routed via `summarize_document` |
| C21 | ⏸️ Pending | Multi-modal — screenshot paste |
| C22 | ⏸️ Pending | Offline-robustness — embedded local model integration in `crates/ai` |
| C23 | ⏸️ Pending | Cost discipline — per-action credit metering UI + cost-per-user dashboard hooks |

## ADRs (planned)

- 0015 — AI tool registry expansion
- 0016 — User memory layer
- 0017 — Predictive layer Monte Carlo
- 0018 — Multi-modal input
- 0019 — Offline robustness embedded model
- 0020 — AI cost discipline

## Definition of Done

- All 15+ new tools registered with AI Safety Runtime properties at compile time (per-turn cap weight + audit scope + numeric bounds + Truth Ledger flag); missing any → compile error
- `user_memory` editor surface live; every fact example from spec §7.3 saveable
- Predictive layer runs in Tauri sidecar (no main process block); outputs cached in `projection_snapshots`
- 4 modalities usable end-to-end (voice / image / document / screenshot)
- Offline banner appears when agent service unreachable
- Credit meter visible in agent input bar
- Anthropic prompt cache hit rate measured ≥ 80%
- Truth Ledger entry emitted by every financial-mutating tool — chain integrity test passes after AI write paths (QA-P19)

## Open Questions

- Embedded model choice (Llama 3.1 8B quantized vs Qwen2.5 7B quantized) — measure cold-start impact + disk footprint before committing
- Memory expiry policy default: 12mo TTL on `'agent-inferred'` source, never-expire on `'user-stated'`
