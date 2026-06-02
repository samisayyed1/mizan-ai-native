# ADR 0022 — News module + personalization

| Status | ✅ Accepted (autonomous-execution authority — Track D foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/04-track-d.md](../plans/04-track-d.md), [ADR 0018 — Dashboard IA](0018-dashboard-information-architecture.md), [ADR 0020 — AI Tool Registry](0020-ai-tool-registry-expansion.md) (the `get_news` tool consumes this surface) |

## Context

The Mizan Evolution Spec §10 prescribes a personalised news module: a two-tab UI (Relevant / Global), provider-aggregated content from multiple sources, relevance scoring tied to the user's holdings + memory, "Why this matters to you" reasoning per item, in-app reader, and an "Discuss with Mizan" link out to the AI dispatcher.

This ADR locks the provider set, the personalisation model, and the cross-tier data shape so PR-D1..D10+ land against a stable plan.

## Decision

### News providers (initial set)

| Provider | Coverage | Tier |
|---|---|---|
| **NewsAPI** | Global aggregator | All — included in Silver+ |
| **NewsCatcher** | Global aggregator (different source mix) | Gold+ (second opinion) |
| **Benzinga** | US financial news + Squawk | Gold+ |
| **Polygon** | Market data + news | Gold+ (already a quote-provider option) |
| **Refinitiv** | Institutional-grade | Enterprise (paid line item — see Track G) |
| **Bondevalue** | Asia + emerging-market bond headlines | All (Sukuk-relevant) |
| **CNA / Mint / Khaleej Times** | Regional (SG / IN / UAE) | All |
| **IFN / Salaam Gateway** | Islamic finance | All (Sharia-screening relevant) |

Per-provider clients live in `mizan-connect/src/news/providers/`. The aggregator (`news_aggregator.rs`) fans out to all enabled-per-tier providers, dedupes by article URL hash, then writes to the `news_items` table (Track D migration already shipped).

### Personalisation worker

The personalisation worker lives on Mizan Connect (`mizan-connect/src/news/personalization.rs`):

1. **Input:** the user's holdings + the user's memory (cloud-mirrored from desktop per Track C PR-C3)
2. **Embedding lookup:** each news item gets a `news_embeddings` row (sentence-transformer 384-d) when the aggregator writes it. The user's "topical interest vector" is the mean of their holdings' embeddings + their memory fact embeddings, capped to the same dimension.
3. **Ranking:** cosine similarity between the user vector and each item's embedding produces a per-item relevance score in [0, 1]. Items > 0.4 are eligible for the "Relevant" tab.
4. **Diversity:** the top-20 by relevance go through a Maximal Marginal Relevance (MMR) pass to prevent topical clustering (no 20 articles all about the same Fed-rate-cut news). MMR's λ = 0.7 (the conventional default for news ranking).
5. **Recency boost:** scores decay over time per `exp(-0.05 * age_in_days)`. A 2-week-old article competes with a fresh one only if it's >2× more relevant.
6. **Output:** the top-10 per user gets persisted into `news_user_feed_${user_id}` (materialized per-user feed, cache-invalidated on holdings change).

### "Why this matters to you" reasoning

The AI dispatcher's `get_news` tool (per ADR 0020 entry #14) consumes the personalised feed. For each article surfaced, the tool composes a one-sentence "Why this matters" caption sourced from:
- The article's relevant entities (tickers / countries / sectors)
- The user's holding that overlaps with those entities
- A short factual claim (no recommendation; no buy/sell language per working-agreement §16 "no financial advice")

Example: "You hold AAPL (12% of portfolio); this Apple-supplier export filing affects companies in your holding's supply chain."

### Two-tab UI

Per Spec §10:

- **Relevant tab** (default): the personalised feed
- **Global tab**: chronological, all-providers, no personalisation filter — for users who want raw feed access (Gold+ default; Silver+ optional)

Both tabs share the same item-card component; the difference is the data source (`news_user_feed_*` vs `news_items` ordered by `published_at DESC`).

### Reading state + saved articles + share

Per Spec §10:
- **Reading state** stored in `news_read_state` table per (user_id, article_id) — sync'd to cloud via Mizan Connect
- **Saved articles** stored in `news_saved_articles` — appears in a dedicated "Saved" sub-tab
- **Share** uses the OS native share sheet (Tauri's `tauri-plugin-shell::open` + a deep link)

## Rationale

**Why a cloud-side personalisation worker (not desktop-side)?**
- The cost of computing embeddings is amortizable across users — one embedding per article serves every user who sees it
- The user's holdings + memory are already mirrored to cloud (Track C); running personalisation cloud-side reuses that
- Offline-mode users see the last-synced feed; not a blocker
- Cloud-side processing keeps the desktop's cold-start budget intact (Spec §17 / working-agreement §A19)

**Why MMR for diversity?**
Without MMR, the personalisation tends to collapse on a single hot topic (all top-20 articles about the same earnings release). MMR with λ=0.7 is the standard ML information-retrieval choice for "balance relevance with novelty"; the value 0.7 weights relevance moderately above diversity. Tunable per user-research feedback.

**Why the dedicated "Why this matters" reasoning (not just headline)?**
The Mizan agent's value-add over a generic news reader is the **personal context**. A user with no exposure to Boeing doesn't need to read another Boeing headline. The reasoning forces the agent to surface ONLY items that genuinely affect the user.

**Why no "buy this" / "sell this" recommendations?**
Working-agreement §16: Mizan never gives financial advice. The agent surfaces facts + the user's context; the user decides actions. This is enforced as an adversarial-prompt test in the dispatcher (see ADR 0020).

**Why two tabs (not one feed)?**
Users self-select. Some want the curated feed (Relevant); some prefer to browse everything (Global). Forcing one removes user agency.

## Consequences

**Positive:**
- Personalisation cost amortizes across users (cloud-side embeddings)
- News surface composes naturally with Track C's `get_news` AI tool
- Per-provider client structure lets us add/remove providers without touching the aggregator (each provider PR is small + focused)
- Cache-invalidated materialised feeds keep desktop reads fast (Spec §17 budget)

**Negative / accepted:**
- Cloud dependency for personalisation: offline users see stale feed. Mitigation: feed sync is opportunistic; the user sees what was last synced + a "Sync" pull-to-refresh.
- Per-provider API costs vary (NewsAPI free tier; Refinitiv paid). Mitigation: per-tier provider gating (Refinitiv = Enterprise only) and cost dashboard (working-agreement §15).
- Embedding model swap (e.g. upgrading from MiniLM to a larger model) requires re-embedding the entire `news_embeddings` table. Mitigation: embedding model version in the row + a backfill job runs in the eviction worker schedule per ADR 0008.

**Risks:**
- Provider outage degrades feed (NewsAPI quota hit). Mitigation: per-provider circuit-breaker; the aggregator continues with whatever providers responded.
- Personalisation could surface privacy-relevant edge cases (e.g. an article about the user's neighbourhood). Mitigation: the personalisation worker operates on holdings + memory facts only; location-data is NOT used as a personalisation signal (working-agreement §9.2 privacy contract).

## Alternatives considered

- **Single provider (NewsAPI only)** — rejected; spec §10 requires multi-source for resilience + Sharia-specific outlets (IFN / Salaam Gateway) that NewsAPI doesn't aggregate.
- **Desktop-side personalisation** — rejected per "Why cloud-side" above (cost amortization + offline contract).
- **Single-tab feed (no Global option)** — rejected because power users (Gold) want raw feed access; spec §10 explicitly mentions both tabs.
- **Recommendation feed (with buy/sell signals)** — REJECTED. Working-agreement §16 hard rule.

## Implementation map

| PR | What lands |
|---|---|
| PR-D1 | `news_items` migration + dedup-by-URL hash logic (foundation already shipped in PR-C/D foundation batch — see task tracker item #34) |
| PR-D2 | First provider integration (NewsAPI) end-to-end as a template |
| PR-D3 | Personalisation worker on Mizan Connect (embeddings, ranking, MMR, recency boost, materialised feed) |
| PR-D4 | News feed endpoint `GET /v1/news/feed?tab=relevant|global&cursor=...` |
| PR-D5 | Desktop sync from cloud on app open + periodic refresh |
| PR-D6 | News page — Relevant tab UI |
| PR-D7 | News page — Global tab UI |
| PR-D8 | News card with "Why this matters to you" reasoning |
| PR-D9 | In-app reader + related-holdings side panel + "Discuss with Mizan" entry point |
| PR-D10 | Read state + saved articles + share |
| PR-D11+ | Additional providers (Benzinga / Polygon / Refinitiv / Bondevalue / CNA / Mint / Khaleej Times / IFN / Salaam Gateway) — one PR per provider |

Each PR ≤ 500 lines per working-agreement §A21.
