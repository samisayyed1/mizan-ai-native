# Architecture Decision Records (ADRs)

Numbered, chronological record of every significant architectural decision in Mizan. Per the working agreement (`docs/working-agreement.md` §16), ADRs are the source of truth for **why** the system looks the way it does.

## Format

Every ADR follows the standard four-section structure:

1. **Context** — what's the situation, what forces are in play
2. **Decision** — what we chose to do
3. **Consequences** — what changes downstream, positive and negative
4. **Alternatives Considered** — what else was on the table, why it was rejected

## Conventions

- Numbered `NNNN-kebab-case-title.md` starting at `0001`
- Status field: `Proposed` / `Accepted` / `Superseded by NNNN` / `Deprecated`
- Date in `YYYY-MM-DD`
- Deciders listed by name
- Track letter (A–K) per the Evolution Plan when applicable
- Update the Status field — never delete an ADR, only mark it superseded

## Lifecycle

ADRs reviewed annually for staleness per `docs/working-agreement.md` §16. A stale ADR either gets a new ADR superseding it or has its current relevance confirmed in a comment-only revision.

## Active ADRs

| # | Title | Track | Status |
|---|---|---|---|
| 0001 | [Adopt the v1.0 Working Agreement as `docs/working-agreement.md`](0001-adopt-working-agreement-v1.md) | H | Accepted |
| 0006 | [CI Hygiene Scans (informational)](0006-ci-hygiene-scans.md) | H | Accepted |
| 0008 | [Cache Policy Single Source of Truth](0008-cache-policy-single-source-of-truth.md) | I | Accepted |
| 0009 | [Updater Snapshot & Rollback Design](0009-updater-snapshot-and-rollback-design.md) | I | Accepted |
| 0010 | [IPC Schema Versioning](0010-ipc-schema-versioning.md) | I | Accepted |
| 0011 | [Holdings Metadata Design (pre-Track E)](0011-holdings-metadata-design.md) | E | Accepted |
| 0012 | [AAOIFI Sharia Screening Criteria](0012-aaoifi-screening-criteria.md) | E | Accepted (annual review) |

## Pending (planned, not yet written)

Per `docs/plans/00-master-plan.md`. Numbering is illustrative — ADRs are written in chronological order as work lands.

| # | Title | Track |
|---|---|---|
| 0002 | Extract `crates/financial-truth` | H |
| 0003 | Extract `crates/zakat` | H |
| 0004 | Extract `crates/insights` | H |
| 0005 | Extract `crates/synthesis` | H |
| 0007 | Repo rename `mizan-4/` → `mizan-desktop/` (conditional) | H |
| 0049 | Extract `crates/csv-import` | H |
| 0011 | Badge modifier severity ordering | E |
| 0013 | Dashboard information architecture | A |
| 0014 | Charting vocabulary (donut/bar/heatmap/sparkline/Sankey) | A |
| 0015 | AI tool registry expansion | C |
| 0016 | User memory layer | C |
| 0017 | Predictive layer (Monte Carlo) | C |
| 0018 | Multi-modal input | C |
| 0019 | Offline robustness — embedded model | C |
| 0020 | AI cost discipline | C |
| 0021–0029 | Per-provider integration ADRs | B |
| 0030 | News providers and personalization model | D |
| 0031 | Personal materiality scoring | D |
| 0032 | Maliki school rules (requires scholarly sign-off) | F |
| 0033 | Hanbali school rules (requires scholarly sign-off) | F |
| 0034 | PE Zakatability | F |
| 0035 | Locked retirement two-views | F |
| 0036 | Crypto Zakatability toggleable | F |
| 0037 | Debt deduction by school | F |
| 0038 | Zakat payment flow via Stripe | F |
| 0039 | SSO SAML/OIDC Enterprise | G |
| 0040 | Advisor-Client linking model | G |
| 0041 | Enterprise multi-seat billing | G |
| 0042 | OAuth connector framework | J |
| 0043 | Initial OAuth provider selection | J |
| 0044 | MCP capability architecture | K |
| 0045 | MCP sandbox gate (absolute) | K |
| 0046 | MCP egress DLP rules | K |
| 0047 | MCP public catalog review process | K |
| 0048 | MCP `trust_level` schema prep | K |
