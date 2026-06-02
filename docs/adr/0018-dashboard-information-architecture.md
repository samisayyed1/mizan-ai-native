# ADR 0018 — Dashboard Information Architecture

| Status | ✅ Accepted (autonomous-execution authority — Track A foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [Working-agreement §A14 (dashboard UX rules)](../working-agreement.md), [docs/plans/01-track-a.md](../plans/01-track-a.md), [ADR 0019 — Charting Vocabulary](0019-charting-vocabulary.md) (planned, lands with PR-A3) |

## Context

Mizan's existing dashboard surface (pre-Track A) is a Wealthfolio-inherited layout: top hero balance, multiple sibling chart cards, sidebar navigation between Portfolio / Net Worth / Activities. The Mizan Evolution Spec §3 prescribes a different shape — AI-native, single-surface, pinned command bar, fixed-order asset class panels.

This ADR locks the information-architecture decision so PR-A4 through PR-A14 can land against a stable plan. The visual implementation (chart primitives, panel components, etc.) lives in subsequent ADRs + PRs.

## Decision

The dashboard is **one surface, top-to-bottom scrollable**, replacing the separate Portfolio surface entirely (PR-A13 deletes the route).

### Section order (top → bottom, fixed)

1. **AI Command Bar** — Pinned to viewport top; never collapses on scroll. Connects to the local AI dispatcher with optional escalation to `/v1/ai/agent` for Gold+ users.
2. **Net Worth Strip** — Toggleable 24h/7d/30d/YTD/All deltas + sparkline; tap → Net Worth page.
3. **Asset-class Heatmap** — One tile per asset class, sized by % of portfolio, coloured by 24h move; tap → asset detail.
4. **News Strip Placeholder** — In Track A v1, a static "Wire me up" placeholder; Track D fills with real personalised news.
5. **Asset Class Panels** — 12 panels in spec §3(e) fixed order: Equities / Brokerage Accounts / Bank+Cash / Bonds+Sukuks / Provident Funds / Insurance / Private Equity / Real Estate / Crypto / Commodities / Collectibles / Forex.
6. **Today's Signal** — One AI-narrated insight per day, sourced from `crates/insights` rules + deduplicated against the last 7d's signals.
7. **Quick Action Pull-up Sheet** — Modal triggered by a floating action button: Add asset / Run Zakat / Generate report / Talk to Mizan.

### What's removed

- The **Portfolio surface** is deleted (PR-A13). The `/portfolio` route 302-redirects to `/` with a one-time toast: "Portfolio merged into Dashboard."
- The **per-page sidebar nav** between Portfolio / Net Worth / Activities is replaced by the AI Command Bar + the heatmap tile taps as the primary navigation paths.

### What's preserved

- The **Net Worth page** as a deep-dive route (`/net-worth`) — separate from the dashboard heatmap, accessible via Net Worth Strip tap. Its skeleton was audited in PR-A2; gaps land as PR-A2.a/b/c per docs/plans/01-track-a.md.
- The **Activities page** for transaction history + CSV imports.
- All **Asset-detail pages** (one per holding type) reachable via heatmap-tile taps.

## Rationale

**Why pinned AI Command Bar (not collapsible)?**
Working-agreement §A14: "AI is the primary interaction; chrome that hides it on scroll trains users that AI is secondary." The bar's height (44px on desktop, 56px on mobile) is small enough to coexist with the heatmap on first paint.

**Why fixed-order asset class panels (not user-reorderable in v1)?**
Two reasons:
1. **Cognitive load** — users in user-research feedback (cited in spec §3) reported they "scan top-to-bottom" rather than seeking; fixed order makes the surface predictable across sessions.
2. **Scope** — drag-to-reorder + persistence + cross-device sync is a 3-PR detour. Slipped to PR-A14.1 post-rollout.

The fixed order is spec-defined (§3(e)) and matches the conventional wealth-management report ordering (Equities and Brokerage first because they're the largest category for the majority of users; Bonds + Sukuks before alternatives; collectibles and forex last because they're niche). Documented in PR-A9.

**Why slide-up sheet for quick actions instead of always-visible buttons?**
The spec lists 4 quick actions but the dashboard already has the AI Command Bar covering "Talk to Mizan" + a heatmap. Inline buttons would compete for top-of-fold attention; the slide-up sheet is the conventional mobile-first pattern.

**Why delete the Portfolio surface (not deprecate over 6 months)?**
The Portfolio surface and the new Dashboard cover the same data. Keeping both would force every Track A panel + chart primitive change to touch two surfaces — doubling review surface for the same outcome. The 302 redirect + one-time toast covers users with bookmarks; documented in PR-A13.

## Consequences

**Positive:**
- One scroll surface to learn instead of navigating between Portfolio / Net Worth / Activities for top-level views
- AI Command Bar always reachable → encourages discovery of agent-driven workflows
- Fixed panel order = predictable surface across sessions + simpler caching (panel order is part of the cache key)

**Negative / accepted:**
- Users who customised Portfolio's old layout lose that customisation. Mitigation: one-time toast explains the move + Quick Action sheet maintains action-shortcut familiarity.
- Total scrollable height is large (12 panels × ~400px avg = ~5000px). Mitigation: each panel lazy-renders below the fold per PR-A9; the heatmap remains the primary navigation device for users who want to jump.

**Risks:**
- Performance budget §A19 (cold start < 1.2s, chart paint < 200ms cached) requires every panel above the fold to render synchronously. PR-A9 measures + slips below-fold panels to lazy-render if budget breaches.

## Alternatives considered

- **Keep both Portfolio and Dashboard** — rejected per §"Why delete the Portfolio surface" above.
- **User-reorderable panels in v1** — rejected per §"Why fixed-order" above. Tracked as PR-A14.1.
- **Sidebar-driven nav instead of heatmap taps** — rejected because the spec's user research showed heatmap taps had higher tap-through rate than sidebar in usability tests.

## Implementation map

| PR | What lands |
|---|---|
| **PR-A2 (this ADR's anchor)** | Audit + ADR; no code change |
| PR-A3 | Shared chart primitives (donut/bar/sparkline/heatmap) per ADR 0019 |
| PR-A4 | Remove pie/radar/polar/3D chart usages (audit + replace with donut/bar) |
| PR-A5 | AI Command Bar component + pinning |
| PR-A6 | Net worth strip + toggleable deltas + sparkline |
| PR-A7 | Heatmap tile-tap → asset detail |
| PR-A8 | News strip placeholder (Track D fills real wiring) |
| PR-A9 | Asset class panel skeleton + 12 panels in fixed order |
| PR-A10 | Today's Signal card |
| PR-A11 | Quick action pull-up sheet |
| PR-A12 | Notification panel polish |
| PR-A13 | Remove Portfolio surface + 302 redirect |
| PR-A14 | Feature flag rollout (`dashboard_v2`) |

Each PR ≤ 500 lines per working-agreement §A21.
