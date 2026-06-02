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
| D2 | ⏸️ Pending | First provider integration (NewsAPI) end-to-end as a template |
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
