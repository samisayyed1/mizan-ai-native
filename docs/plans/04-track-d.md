# Track D — News Module

**Status:** Pending. Depends on Track C (memory + tool registry for personalization).
**Estimated sprints:** 2.
**Source:** `docs/plans/00-master-plan.md` → "Track D — News Module".

## Scope

**In:** Mizan Connect provider integrations (NewsAPI / NewsCatcher / Benzinga / Polygon / Refinitiv / Bondevalue / regional feeds), personalization worker (relevance scoring against `user_memory` + holdings via vector similarity), `news_items` table on desktop, two-tab UI (Relevant / Global), reading state, saved articles, share.

**Out:** Dashboard news strip placeholder (Track A); `get_news` agent tool stub (Track C — table here populates it).

## PRs

| # | Status | Title |
|---|---|---|
| D1.a | ✅ Done | `news_items` desktop migration | `2026-06-02-000004_news_items` with 4 indexes including partial saved-list + personalized + read-state indexes |
| D1.b | ⏸️ Pending | `news_items_per_user` materialized view on Mizan Connect |
| D2 | ✅ Done (2026-06-04 as PR-D2 per Goal v3 §V Phase 6 — Mizan Connect news module foundation; PR-D2.b adds the `/v1/news/feed` handler over this provider stack; per-region providers ship as PR-D2.c..g) | NewsAPI provider + news module foundation in `mizan-connect/src/news/`. New module structure: `mod.rs` (router stub with `/v1/news/health`), `types.rs` (`RawArticle`, `NewsCategory` with lexical classifier covering Sukuks-beats-Bonds + Crypto + Commodities + Forex + RealEstate + Equities + Regulatory + Macro + Other, `NewsTab` parser), `personalization.rs` (`rank_articles` pure function with 3 deterministic signals: ticker overlap +0.6, category-of-holding +0.3, memory-keyword +0.1, capped at 1.0; ties break by published_at desc; PR-D3 will graft pgvector similarity over this baseline), `providers/newsapi.rs` (NewsAPI.org REST client with `parse_response` pure parser + `fetch_with_base` for wiremock-backed tests, FetchQuery normalises pageSize to [1,100], `NewsApiError::{MissingApiKey, Transport, HttpError, BadBody}` enumerated). Wired into `lib.rs` + `server.rs` (merged after sharia router). **35 unit tests**: 13 types tests (every NewsCategory branch + RawArticle::classify constructor), 9 personalization tests (empty input, ticker case+whitespace, category alignment, memory keyword, all-three-signals cap, tiebreaker, empty-string filtering, §23 sukuk-outranks-macro fixture), 13 newsapi tests (clamping, canonical payload parse including AAPL+sukuk, missing-fields skip, missing-description default, 800-char summary truncation, non-ok status reject, garbage-json reject, unknown-source default, 404 HttpError, wiremock happy-path, missing-api-key error). `cargo check` clean; `cargo clippy --lib --tests -- -D warnings` clean (test modules `#[allow(expect_used, unwrap_used, panic)]` per existing crate convention); 35/35 tests pass. |
| D3 | ⏸️ Pending | Personalization worker on Mizan Connect (vector similarity against `user_memory` + holdings) |
| D4 | ⏸️ Pending | News feed endpoint `GET /v1/news/feed?tab=relevant\|global&cursor=...` |
| D5 | ⏸️ Pending | Desktop sync from cloud on app open + periodic |
| D6 | ⏸️ Pending | News page — Relevant tab UI |
| D7 | ⏸️ Pending | News page — Global tab UI |
| D8 | ⏸️ Pending | News card with "Why this matters to you" reasoning rendered by agent |
| D9 | ⏸️ Pending | In-app reader + related-holdings side panel + "Discuss with Mizan" |
| D10 | ⏸️ Pending | Read state, saved articles, share |
| D11..N | ⏸️ Pending | Per additional provider: Benzinga, Polygon, Refinitiv (paid), Bondevalue, CNA, Mint, Khaleej Times, IFN, Salaam Gateway |

## ADRs (planned)

- 0030 — News providers and personalization model
- 0031 — Personal materiality scoring

## Definition of Done

- Two-tab News page live (Relevant + Global)
- 5+ providers integrated
- Personalization runs cloud-side, syncs to desktop
- Reference user sees Sukuk issuer headlines (Emaar, Dar al Arkan, Sobha) ranked above generic Fed news per spec §10
