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
| C4.a | ✅ Done (2026-06-04) | Safety descriptors for `create_holding`/`update_holding`/`delete_holding` (ADR 0020 rows 1-3). First consumer of the `register_tool!` macro from C3.b — locks the AI Safety Runtime contract for the three holdings-mutation tools before handler impls land. 7 unit tests assert each row matches ADR 0020 (cap=5/5/3; audit=FullInput/Diff/IdOnly; bounds=Holding/Holding/None; all three emit Truth Ledger). |
| C4.a.1 | ⏸️ Pending | `create_holding` handler — draft-preview per the `create_account` pattern (consumes C4.a descriptor) |
| C4.a.2 | ⏸️ Pending | `update_holding` handler — diff-based update draft |
| C4.a.3 | ⏸️ Pending | `delete_holding` handler — confirmation prompt + ledger-deletion entry |
| C5 | ⏸️ Pending | Tools: `add_activity`, `list_activities` |
| C6 | ⏸️ Pending | Tools: `compute_net_worth`, `get_holding_history` |
| C7 | ⏸️ Pending | Tools: `get_market_data`, `get_fx_rate` (refuses to invent rates per QA Pass 8 + working agreement §0 rule 2) |
| C8 | ⏸️ Pending | Tools: `sync_account`, `generate_report` |
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
